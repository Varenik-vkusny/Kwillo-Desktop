#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_capture;
mod file_manager;
mod meeting_poller;
mod state;
mod uploader;

use audio_capture::AudioCapture;
use state::{AppState, AppStatus};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Shared capture handle ───────────────────────────────────────────────────

type CaptureHandle = Arc<Mutex<Option<AudioCapture>>>;

// ─── Tauri Commands ──────────────────────────────────────────────────────────

/// Called by the frontend when the user approves recording.
#[tauri::command]
fn start_recording(
    app: AppHandle,
    app_state: State<AppState>,
    capture: State<CaptureHandle>,
) -> Result<(), String> {
    let platform = match app_state.get_status() {
        AppStatus::PendingPermission { platform } => platform,
        _ => return Err("Not in PendingPermission state".to_string()),
    };

    file_manager::ensure_temp_dir().map_err(|e| e.to_string())?;
    let path = file_manager::new_recording_path();

    let audio = AudioCapture::start(path)?;
    *capture.lock().unwrap() = Some(audio);

    let started_at_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    app_state.set_status(AppStatus::Recording {
        platform: platform.clone(),
        started_at_secs,
    });

    app.emit("recording-started", platform).ok();
    Ok(())
}

/// Called by the frontend when the user stops recording, or automatically when meeting ends.
#[tauri::command]
async fn stop_recording(
    app: AppHandle,
    app_state: State<'_, AppState>,
    capture: State<'_, CaptureHandle>,
    upload_url: String,
) -> Result<(), String> {
    app_state.set_status(AppStatus::Uploading);
    app.emit("upload-started", ()).ok();

    let audio = capture.lock().unwrap().take();
    if let Some(audio) = audio {
        let path = audio.stop()?;

        match uploader::upload_and_delete(path, &upload_url).await {
            Ok(_) => {
                app.emit("upload-success", ()).ok();
            }
            Err(e) => {
                app.emit("upload-failed", e).ok();
            }
        }
    }

    app_state.set_status(AppStatus::Idle);
    Ok(())
}

/// Called when the user dismisses the permission dialog.
#[tauri::command]
fn dismiss_recording(app_state: State<AppState>) {
    app_state.set_status(AppStatus::Idle);
}

// ─── Meeting Poll Loop ────────────────────────────────────────────────────────

fn start_poll_loop(app: AppHandle, upload_url: String) {
    std::thread::spawn(move || {
        let app_state = app.state::<AppState>();

        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let status = app_state.get_status();

            match status {
                AppStatus::Idle => {
                    if let Some(platform) = meeting_poller::detect_active_meeting() {
                        app_state.set_status(AppStatus::PendingPermission {
                            platform: platform.clone(),
                        });
                        app.emit("meeting-detected", platform).ok();

                        if let Some(window) = app.get_webview_window("main") {
                            window.show().ok();
                            window.set_focus().ok();
                        }
                    }
                }
                AppStatus::Recording { .. } => {
                    if meeting_poller::detect_active_meeting().is_none() {
                        app.emit("meeting-ended", ()).ok();
                        let app_clone = app.clone();
                        let url_clone = upload_url.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_clone.state::<AppState>();
                            let capture = app_clone.state::<CaptureHandle>();
                            let _ =
                                stop_recording(app_clone.clone(), state, capture, url_clone).await;
                        });
                    }
                }
                _ => {}
            }
        }
    });
}

// ─── Config ──────────────────────────────────────────────────────────────────

fn load_upload_url() -> String {
    let config_path = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("audio_analyzer")
        .join("config.json");

    if let Ok(data) = std::fs::read_to_string(config_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(url) = json.get("upload_url").and_then(|v| v.as_str()) {
                return url.to_string();
            }
        }
    }

    "http://localhost:8080/upload".to_string()
}

// ─── App Entry ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let upload_url = load_upload_url();

    tauri::Builder::default()
        .manage(AppState::default())
        .manage(CaptureHandle::default())
        .setup(move |app| {
            start_poll_loop(app.handle().clone(), upload_url);

            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            TrayIconBuilder::new()
                .tooltip("Audio Analyzer")
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            dismiss_recording
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
