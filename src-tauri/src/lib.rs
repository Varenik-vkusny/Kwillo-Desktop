#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_capture;
mod file_manager;
mod meeting_poller;
mod state;
mod uploader;

use audio_capture::AudioCapture;
use state::{AppState, AppStatus};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

// ─── Shared capture handle ───────────────────────────────────────────────────

type CaptureHandle = Arc<Mutex<Option<AudioCapture>>>;

// ─── Poll loop constants ─────────────────────────────────────────────────────

/// Consecutive detections required before showing the permission prompt.
/// Mic-based detection is a hard signal (no false positives), so 1 tick is enough.
const ENTRY_DEBOUNCE: u32 = 1;

/// Consecutive missed detections before auto-stopping a recording.
/// 3 ticks × 2s = 6 seconds grace for brief network/mic drops.
const EXIT_DEBOUNCE: u32 = 3;

/// Seconds before the unanswered permission dialog is silently dismissed.
const PENDING_TIMEOUT_SECS: u64 = 60;

/// Seconds after "Skip" before we re-prompt for the same platform.
/// The meeting must also disappear entirely before we reset this.
const DISMISS_COOLDOWN_SECS: u64 = 300;

/// Initial grace period before the first detection check.
/// Gives the frontend webview time to mount and register Tauri event listeners.
const STARTUP_GRACE_SECS: u64 = 2;

// ─── Window threshold logic ──────────────────────────────────────────────────

/// Returns true if at least `threshold` of the entries in `window` are `true`.
/// Pure function — no side effects, easy to unit test.
fn window_is_active(window: &std::collections::VecDeque<bool>, threshold: usize) -> bool {
    window.iter().filter(|&&v| v).count() >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn make_window(values: &[bool]) -> VecDeque<bool> {
        values.iter().copied().collect()
    }

    #[test]
    fn empty_window_is_not_active() {
        assert!(!window_is_active(&make_window(&[]), 4));
    }

    #[test]
    fn below_threshold_is_not_active() {
        // 3 out of 6 — below threshold of 4
        assert!(!window_is_active(
            &make_window(&[true, true, true, false, false, false]),
            4
        ));
    }

    #[test]
    fn at_threshold_is_active() {
        // exactly 4 out of 6
        assert!(window_is_active(
            &make_window(&[true, true, true, true, false, false]),
            4
        ));
    }

    #[test]
    fn above_threshold_is_active() {
        // 5 out of 6
        assert!(window_is_active(
            &make_window(&[true, true, true, true, true, false]),
            4
        ));
    }

    #[test]
    fn full_active_window_is_active() {
        assert!(window_is_active(
            &make_window(&[true, true, true, true, true, true]),
            4
        ));
    }

    #[test]
    fn all_false_is_not_active() {
        assert!(!window_is_active(
            &make_window(&[false, false, false, false, false, false]),
            4
        ));
    }

    #[test]
    fn partial_window_under_threshold() {
        // Window not yet full (only 2 ticks seen) — can't be active
        assert!(!window_is_active(&make_window(&[true, true]), 4));
    }

    #[test]
    fn partial_window_at_threshold() {
        // Window not yet full but already at threshold — should be active
        assert!(window_is_active(&make_window(&[true, true, true, true]), 4));
    }
}

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

/// Called when the user dismisses the permission dialog ("Skip").
/// Records a per-platform cooldown so we don't immediately re-prompt.
#[tauri::command]
fn dismiss_recording(app_state: State<AppState>) {
    if let AppStatus::PendingPermission { platform } = app_state.get_status() {
        app_state.record_dismiss(platform);
    }
    app_state.set_status(AppStatus::Idle);
}

/// Debug: returns current process names and visible window titles.
/// Call from browser devtools: await window.__TAURI__.core.invoke('debug_detection')
#[tauri::command]
fn debug_detection() -> serde_json::Value {
    #[cfg(target_os = "windows")]
    {
        use meeting_poller::windows_scan;
        let mic_active = windows_scan::browser_with_active_mic();
        let meeting = meeting_poller::detect_active_meeting();
        serde_json::json!({
            "browser_mic_active": mic_active,
            "detected_meeting": meeting,
        })
    }
    #[cfg(not(target_os = "windows"))]
    serde_json::json!({ "error": "Windows only" })
}

// ─── Meeting Poll Loop ────────────────────────────────────────────────────────

fn start_poll_loop(app: AppHandle, upload_url: String) {
    std::thread::spawn(move || {
        let app_state = app.state::<AppState>();

        // How many consecutive ticks we've seen a meeting while Idle.
        let mut consecutive_detections: u32 = 0;
        // How many consecutive ticks we've NOT seen a meeting while Recording.
        let mut missed_detections: u32 = 0;
        // When the PendingPermission state was entered (for timeout tracking).
        let mut pending_since: Option<std::time::Instant> = None;

        // Wait for the frontend webview to mount and register its event listeners
        // before we start emitting any events. Without this pause, a meeting that
        // is already running at app launch would fire `meeting-detected` before
        // React has called `listen()`, causing the event to be silently dropped.
        std::thread::sleep(std::time::Duration::from_secs(STARTUP_GRACE_SECS));

        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let status = app_state.get_status();

            match status {
                // ── Idle: watch for a new meeting to start ─────────────────
                AppStatus::Idle => {
                    missed_detections = 0;

                    match meeting_poller::detect_active_meeting() {
                        Some(platform) => {
                            if app_state.should_prompt(&platform, DISMISS_COOLDOWN_SECS) {
                                consecutive_detections += 1;

                                // Require N stable detections before prompting.
                                // This avoids false positives from transient process
                                // names or browser tab flickers during page loads.
                                if consecutive_detections >= ENTRY_DEBOUNCE {
                                    consecutive_detections = 0;
                                    pending_since = Some(std::time::Instant::now());

                                    app_state.set_status(AppStatus::PendingPermission {
                                        platform: platform.clone(),
                                    });
                                    app.emit("meeting-detected", platform).ok();

                                    if let Some(window) = app.get_webview_window("popup") {
                                        window.show().ok();
                                        window.set_focus().ok();
                                    }
                                }
                            } else {
                                // In cooldown after "Skip" — don't count toward debounce
                                consecutive_detections = 0;
                            }
                        }
                        None => {
                            consecutive_detections = 0;
                            // Meeting is gone → clear the dismiss cooldown so the
                            // next time this platform is detected (new session) we
                            // will prompt again.
                            app_state.clear_dismissed();
                        }
                    }
                }

                // ── PendingPermission: user hasn't responded yet ───────────
                AppStatus::PendingPermission { .. } => {
                    consecutive_detections = 0;
                    missed_detections = 0;

                    let meeting_gone = meeting_poller::detect_active_meeting().is_none();
                    let timed_out = pending_since
                        .map(|t| t.elapsed().as_secs() >= PENDING_TIMEOUT_SECS)
                        .unwrap_or(false);

                    if meeting_gone || timed_out {
                        // Silently dismiss: the meeting left or the user walked away
                        pending_since = None;
                        app_state.set_status(AppStatus::Idle);
                        // Notify the frontend so it can reset to its idle view
                        app.emit("permission-dismissed", ()).ok();
                        if let Some(window) = app.get_webview_window("popup") {
                            window.hide().ok();
                        }
                    }
                }

                // ── Recording: watch for the meeting to end ────────────────
                AppStatus::Recording { .. } => {
                    consecutive_detections = 0;

                    if meeting_poller::detect_active_meeting().is_none() {
                        missed_detections += 1;
                        if missed_detections >= EXIT_DEBOUNCE {
                            missed_detections = 0;
                            app.emit("meeting-ended", ()).ok();
                            let app_clone = app.clone();
                            let url_clone = upload_url.clone();
                            tauri::async_runtime::spawn(async move {
                                let state = app_clone.state::<AppState>();
                                let capture = app_clone.state::<CaptureHandle>();
                                let _ = stop_recording(
                                    app_clone.clone(),
                                    state,
                                    capture,
                                    url_clone,
                                )
                                .await;
                            });
                        }
                    } else {
                        missed_detections = 0;
                    }
                }

                // ── Uploading: let the upload finish, don't interfere ──────
                AppStatus::Uploading => {
                    consecutive_detections = 0;
                    missed_detections = 0;
                }
            }
        }
    });
}

// ─── Config ──────────────────────────────────────────────────────────────────

fn load_config() -> serde_json::Value {
    let config_path = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("audio_analyzer")
        .join("config.json");

    if let Ok(data) = std::fs::read_to_string(config_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            return json;
        }
    }

    serde_json::json!({})
}

fn load_upload_url() -> String {
    load_config()
        .get("upload_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8080/upload")
        .to_string()
}

/// URL to load in the main Kwillo WebView window.
/// Always opens at /dashboard so the site's auth guard redirects unauthenticated
/// users to /login and sends authenticated users straight to the app.
/// Override the base domain via %APPDATA%\audio_analyzer\config.json → "site_url".
fn load_site_url() -> String {
    let base = load_config()
        .get("site_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:5174")
        .to_string();

    if let Ok(mut url) = base.parse::<url::Url>() {
        url.set_path("/dashboard");
        url.to_string()
    } else {
        format!("{}/dashboard", base.trim_end_matches('/'))
    }
}

// ─── App Entry ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let upload_url = load_upload_url();

    tauri::Builder::default()
        .manage(AppState::default())
        .manage(CaptureHandle::default())
        .setup(move |app| {
            // ── Create the main Kwillo WebView window ─────────────────────
            let site_url = load_site_url();
            if let Ok(parsed_url) = site_url.parse::<url::Url>() {
                let kwillo_window = WebviewWindowBuilder::new(
                    app,
                    "kwillo",
                    WebviewUrl::External(parsed_url),
                )
                .title("Kwillo")
                .inner_size(1400.0, 860.0)
                .min_inner_size(900.0, 600.0)
                .resizable(true)
                .visible(true)
                .build();

                if let Ok(window) = kwillo_window {
                    if let Some(icon) = app.default_window_icon() {
                        let _ = window.set_icon(icon.clone());
                    }
                }
            }

            // ── Set icon for the config-defined 'popup' window ───────────
            if let Some(window) = app.get_webview_window("popup") {
                let _ = window.set_icon(app.default_window_icon().unwrap().clone());
            }

            start_poll_loop(app.handle().clone(), upload_url);

            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            TrayIconBuilder::new()
                .tooltip("Kwillo")
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        // Left click → show/focus the Kwillo site window
                        if let Some(window) = app.get_webview_window("kwillo") {
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
            dismiss_recording,
            debug_detection
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
