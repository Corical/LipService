#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod audio;
mod clipboard;
mod hotkey;
mod pipeline;
mod settings;
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
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;

struct AppState {
    pipeline: Arc<Mutex<Option<Arc<DictationPipeline>>>>,
    is_running: AtomicBool,
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            pipeline: Arc::new(Mutex::new(None)),
            is_running: AtomicBool::new(false),
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

            // Register global shortcut
            let app_handle = app.handle().clone();
            app.global_shortcut().on_shortcut(
                hotkey::SHORTCUT,
                move |_app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    let handle = app_handle.clone();

                    match event.state {
                        ShortcutState::Pressed => {
                            let state = handle.state::<AppState>();
                            if state.is_running.swap(true, Ordering::SeqCst) {
                                return;
                            }

                            let _ = handle.emit("pipeline:state", PipelineState::Recording);

                            tauri::async_runtime::spawn({
                                let handle = handle.clone();
                                async move {
                                    let state = handle.state::<AppState>();
                                    let guard = state.pipeline.lock().await;
                                    if let Some(ref pipeline) = *guard {
                                        if let Err(e) = pipeline.start_recording() {
                                            let _ = handle.emit("pipeline:error", e.to_string());
                                            let _ = handle.emit("pipeline:state", PipelineState::Idle);
                                            state.is_running.store(false, Ordering::SeqCst);
                                        }
                                    }
                                }
                            });
                        }
                        ShortcutState::Released => {
                            tauri::async_runtime::spawn({
                                let handle = handle.clone();
                                async move {
                                    let state = handle.state::<AppState>();
                                    if !state.is_running.load(Ordering::SeqCst) {
                                        return;
                                    }

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
                                    state.is_running.store(false, Ordering::SeqCst);
                                }
                            });
                        }
                    }
                },
            )?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
