#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

    let transcriber = Arc::new(GroqTranscription::new(key.clone(), url.clone()));
    let processor = Arc::new(GroqPostProcessing::new(key, url));
    let recorder = Arc::new(CpalRecorder::new());
    let clipboard_svc = Arc::new(WindowsClipboard::new());

    let new_pipeline = Arc::new(DictationPipeline::new(
        recorder, transcriber, processor, clipboard_svc,
    ));

    *state.pipeline.lock().await = Some(new_pipeline);
    Ok(())
}

#[tauri::command]
fn get_settings() -> Result<FrontendSettings, String> {
    let s = settings::load().map_err(|e| e.to_string())?;
    Ok(FrontendSettings::from(&s))
}

fn show_overlay(app_handle: &tauri::AppHandle) {
    // If overlay already exists, just show it
    if let Some(win) = app_handle.get_webview_window("overlay") {
        let _ = win.show();
        return;
    }

    // Create overlay window
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
        // Position at top center of screen
        if let Ok(monitor) = win.primary_monitor() {
            if let Some(monitor) = monitor {
                let screen_width = monitor.size().width as f64 / monitor.scale_factor();
                let x = (screen_width / 2.0) - 100.0;
                let _ = win.set_position(tauri::LogicalPosition::new(x, 10.0));
            }
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

    // Guard: don't start if already recording or processing
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
        ])
        .setup(|app| {
            tray::create_tray(app)?;

            if let Ok(stored) = settings::load() {
                if stored.has_completed_setup {
                    if let Ok(api_key) = settings::decrypt_api_key(&stored.api_key_encrypted) {
                        let transcriber = Arc::new(GroqTranscription::new(
                            api_key.clone(),
                            stored.api_base_url.clone(),
                        ));
                        let processor = Arc::new(GroqPostProcessing::new(
                            api_key,
                            stored.api_base_url,
                        ));
                        let recorder = Arc::new(CpalRecorder::new());
                        let clipboard_svc = Arc::new(WindowsClipboard::new());
                        let new_pipeline = Arc::new(DictationPipeline::new(
                            recorder, transcriber, processor, clipboard_svc,
                        ));

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
            } else {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.center();
                    let _ = window.set_focus();
                }
            }

            // Register global shortcut — TOGGLE MODE
            // Tap once to start recording, tap again to stop.
            let app_handle = app.handle().clone();
            app.global_shortcut().on_shortcut(
                hotkey::SHORTCUT,
                move |_app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;

                    // Only act on key press, not release
                    if !matches!(event.state, ShortcutState::Pressed) {
                        return;
                    }

                    let handle = app_handle.clone();
                    let state = handle.state::<AppState>();

                    if state.is_processing.load(Ordering::SeqCst) {
                        // Pipeline is processing, ignore
                        return;
                    }

                    if state.is_recording.load(Ordering::SeqCst) {
                        // Currently recording — stop
                        stop_recording(handle);
                    } else {
                        // Not recording — start
                        start_recording(handle);
                    }
                },
            )?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
