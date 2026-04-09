#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

mod api;
mod audio;
mod clipboard;
mod hotkey;
mod pipeline;
mod settings;
mod sounds;
mod tray;

use api::transcription::GroqTranscription;
use api::post_process::GroqPostProcessing;
use audio::recorder::CpalRecorder;
use clipboard::WindowsClipboard;
use pipeline::DictationPipeline;
use pipeline::types::PipelineState;
use settings::types::FrontendSettings;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;

struct AppState {
    pipeline: Arc<Mutex<Option<Arc<DictationPipeline>>>>,
    is_recording: AtomicBool,
    is_processing: AtomicBool,
}

fn build_pipeline(api_key: &str, base_url: &str, settings: &settings::types::AppSettings) -> Arc<DictationPipeline> {
    let transcriber = Arc::new(GroqTranscription::new(
        api_key.to_string(),
        base_url.to_string(),
        settings.transcription_model.clone(),
    ));
    let processor = Arc::new(GroqPostProcessing::new(
        api_key.to_string(),
        base_url.to_string(),
        settings.post_processing_model.clone(),
    ));
    let recorder = Arc::new(CpalRecorder::new());
    let clipboard_svc = Arc::new(WindowsClipboard::new(settings.preserve_clipboard));

    Arc::new(DictationPipeline::new(recorder, transcriber, processor, clipboard_svc))
}

#[tauri::command]
async fn validate_api_key(key: String, base_url: String) -> Result<bool, String> {
    let url = if base_url.trim().is_empty() {
        settings::types::DEFAULT_API_BASE_URL.to_string()
    } else {
        base_url
    };
    Ok(api::validate_api_key(&key, &url).await)
}

#[tauri::command]
async fn save_settings(
    key: String,
    base_url: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let url = if base_url.trim().is_empty() {
        settings::types::DEFAULT_API_BASE_URL.to_string()
    } else {
        base_url
    };
    settings::save_with_api_key(&key, &url).map_err(|e| e.to_string())?;

    let stored = settings::load().map_err(|e| e.to_string())?;
    let new_pipeline = build_pipeline(&key, &url, &stored);
    *state.pipeline.lock().await = Some(new_pipeline);
    Ok(())
}

#[tauri::command]
fn get_settings() -> Result<FrontendSettings, String> {
    let s = settings::load().map_err(|e| e.to_string())?;
    Ok(FrontendSettings::from(&s))
}

#[tauri::command(rename_all = "camelCase")]
async fn update_settings(
    shortcut: String,
    transcription_model: String,
    post_processing_model: String,
    preserve_clipboard: bool,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut stored = settings::load().map_err(|e| e.to_string())?;

    let shortcut_changed = stored.shortcut != shortcut;
    stored.shortcut = shortcut.clone();
    stored.transcription_model = transcription_model;
    stored.post_processing_model = post_processing_model;
    stored.preserve_clipboard = preserve_clipboard;
    settings::save(&stored).map_err(|e| e.to_string())?;

    // Rebuild pipeline with new settings
    if let Ok(api_key) = settings::decrypt_api_key(&stored.api_key_encrypted) {
        let new_pipeline = build_pipeline(&api_key, &stored.api_base_url, &stored);
        *state.pipeline.lock().await = Some(new_pipeline);
    }

    // Re-register shortcut if changed
    if shortcut_changed {
        let gs = app_handle.global_shortcut();
        if let Err(e) = gs.unregister_all() {
            return Err(format!("Failed to unregister old shortcut: {}", e));
        }
        if let Err(e) = try_register_shortcut(&app_handle, &shortcut) {
            return Err(format!("Failed to register shortcut '{}': {}", shortcut, e));
        }
    }

    Ok(())
}

fn show_overlay(app_handle: &tauri::AppHandle) {
    if let Some(win) = app_handle.get_webview_window("overlay") {
        let _ = win.show();
        return;
    }

    let builder = WebviewWindowBuilder::new(
        app_handle,
        "overlay",
        WebviewUrl::App("overlay.html".into()),
    )
    .title("")
    .inner_size(200.0, 50.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .visible(true);

    if let Ok(win) = builder.build() {
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let screen_width = monitor.size().width as f64 / monitor.scale_factor();
            let x = (screen_width / 2.0) - 100.0;
            let _ = win.set_position(tauri::LogicalPosition::new(x, 10.0));
        }
        let _ = win.set_ignore_cursor_events(true);
    }
}

fn hide_overlay(app_handle: &tauri::AppHandle) {
    if let Some(win) = app_handle.get_webview_window("overlay") {
        let _ = win.hide();
    }
}

fn start_recording(handle: tauri::AppHandle) {
    let state = handle.state::<AppState>();

    if state.is_recording.load(Ordering::SeqCst) || state.is_processing.load(Ordering::SeqCst) {
        return;
    }

    state.is_recording.store(true, Ordering::SeqCst);

    sounds::play_start_sound();
    show_overlay(&handle);

    let _ = handle.emit("pipeline:state", PipelineState::Recording);
    let _ = handle.emit("overlay:state", "recording");

    tauri::async_runtime::spawn({
        let handle = handle.clone();
        async move {
            let state = handle.state::<AppState>();
            let guard = state.pipeline.lock().await;
            if let Some(ref pipeline) = *guard {
                if let Err(e) = pipeline.start_recording() {
                    let _ = handle.emit("pipeline:error", e.to_string());
                    let _ = handle.emit("pipeline:state", PipelineState::Idle);
                    state.is_recording.store(false, Ordering::SeqCst);
                    hide_overlay(&handle);
                }
            }
        }
    });
}

fn stop_recording(handle: tauri::AppHandle) {
    let state = handle.state::<AppState>();

    if !state.is_recording.load(Ordering::SeqCst) {
        return;
    }

    state.is_recording.store(false, Ordering::SeqCst);
    state.is_processing.store(true, Ordering::SeqCst);

    sounds::play_stop_sound();

    let _ = handle.emit("overlay:state", "transcribing");

    tauri::async_runtime::spawn({
        let handle = handle.clone();
        async move {
            let state = handle.state::<AppState>();

            let pipeline_guard = state.pipeline.lock().await;
            if let Some(ref pipeline) = *pipeline_guard {
                let _ = handle.emit("pipeline:state", PipelineState::Transcribing);

                match pipeline.execute().await {
                    Ok(result) => {
                        let _ = handle.emit("pipeline:result", &result);
                    }
                    Err(e) => {
                        let _ = handle.emit("pipeline:error", e.to_string());
                    }
                }
            }

            let _ = handle.emit("pipeline:state", PipelineState::Idle);
            state.is_processing.store(false, Ordering::SeqCst);
            hide_overlay(&handle);
        }
    });
}

fn try_register_shortcut(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    let handle = app_handle.clone();
    let shortcut_str = shortcut.to_string();

    app_handle.global_shortcut().on_shortcut(
        shortcut_str.as_str(),
        move |_app, _shortcut, event| {
            use tauri_plugin_global_shortcut::ShortcutState;

            if !matches!(event.state, ShortcutState::Pressed) {
                return;
            }

            let handle = handle.clone();
            let state = handle.state::<AppState>();

            if state.is_processing.load(Ordering::SeqCst) {
                return;
            }

            if state.is_recording.load(Ordering::SeqCst) {
                stop_recording(handle);
            } else {
                start_recording(handle);
            }
        },
    ).map_err(|e| e.to_string())
}

fn register_shortcut(app_handle: &tauri::AppHandle, shortcut: &str) {
    if let Err(e) = try_register_shortcut(app_handle, shortcut) {
        eprintln!("Failed to register shortcut '{}': {}", shortcut, e);
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            pipeline: Arc::new(Mutex::new(None)),
            is_recording: AtomicBool::new(false),
            is_processing: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            validate_api_key,
            save_settings,
            get_settings,
            update_settings,
        ])
        .on_window_event(|window, event| {
            // Hide window on close instead of destroying it
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            tray::create_tray(app)?;

            let stored = settings::load().unwrap_or_default();

            if stored.has_completed_setup {
                if let Ok(api_key) = settings::decrypt_api_key(&stored.api_key_encrypted) {
                    let new_pipeline = build_pipeline(&api_key, &stored.api_base_url, &stored);
                    let state = app.state::<AppState>();
                    let state_pipeline = state.pipeline.clone();
                    tauri::async_runtime::spawn(async move {
                        *state_pipeline.lock().await = Some(new_pipeline);
                    });
                }
            } else {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.center();
                    let _ = window.set_focus();
                }
            }

            // Register the configured shortcut
            let shortcut = stored.shortcut.clone();
            register_shortcut(app.handle(), &shortcut);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
