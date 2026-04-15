use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppStatus {
    Idle,
    PendingPermission { platform: String },
    Recording { platform: String, started_at_secs: u64 },
    Uploading,
}

#[derive(Debug)]
pub struct AppState {
    pub status: Mutex<AppStatus>,
    pub recording_path: Mutex<Option<std::path::PathBuf>>,
    /// When the user dismisses the permission dialog, we store (platform, instant)
    /// so we can avoid re-prompting for the same meeting session.
    /// Cleared automatically when the meeting process goes away entirely.
    pub dismissed: Mutex<Option<(String, Instant)>>,
}

impl Default for AppStatus {
    fn default() -> Self {
        AppStatus::Idle
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(AppStatus::Idle),
            recording_path: Mutex::new(None),
            dismissed: Mutex::new(None),
        }
    }
}

impl AppState {
    pub fn set_status(&self, s: AppStatus) {
        *self.status.lock().unwrap() = s;
    }

    pub fn get_status(&self) -> AppStatus {
        self.status.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        matches!(self.get_status(), AppStatus::Recording { .. })
    }

    #[allow(dead_code)]
    pub fn is_idle(&self) -> bool {
        matches!(self.get_status(), AppStatus::Idle)
    }

    /// Records that the user dismissed the permission prompt for `platform`.
    /// Subsequent calls to `should_prompt` will return false until the cooldown
    /// expires or `clear_dismissed` is called.
    pub fn record_dismiss(&self, platform: String) {
        *self.dismissed.lock().unwrap() = Some((platform, Instant::now()));
    }

    /// Returns true if we should show the recording prompt for this platform.
    ///
    /// Returns false when the user recently dismissed for the same platform and
    /// the `cooldown_secs` window has not yet elapsed.
    pub fn should_prompt(&self, platform: &str, cooldown_secs: u64) -> bool {
        let guard = self.dismissed.lock().unwrap();
        match &*guard {
            Some((dismissed_platform, dismissed_at)) if dismissed_platform == platform => {
                dismissed_at.elapsed().as_secs() >= cooldown_secs
            }
            _ => true,
        }
    }

    /// Clears the dismissed state so the next meeting session starts fresh.
    /// Should be called when the meeting process is no longer detected.
    pub fn clear_dismissed(&self) {
        let mut guard = self.dismissed.lock().unwrap();
        if guard.is_some() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_status_is_idle() {
        let state = AppState::default();
        assert_eq!(state.get_status(), AppStatus::Idle);
    }

    #[test]
    fn test_set_and_get_status() {
        let state = AppState::default();
        state.set_status(AppStatus::PendingPermission {
            platform: "Zoom".to_string(),
        });
        assert_eq!(
            state.get_status(),
            AppStatus::PendingPermission { platform: "Zoom".to_string() }
        );
    }

    #[test]
    fn test_is_recording_false_when_idle() {
        let state = AppState::default();
        assert!(!state.is_recording());
    }

    #[test]
    fn test_is_recording_true_when_recording() {
        let state = AppState::default();
        state.set_status(AppStatus::Recording {
            platform: "Teams".to_string(),
            started_at_secs: 0,
        });
        assert!(state.is_recording());
    }

    #[test]
    fn test_is_idle_false_after_status_change() {
        let state = AppState::default();
        state.set_status(AppStatus::Uploading);
        assert!(!state.is_idle());
    }

    #[test]
    fn test_should_prompt_true_when_no_dismiss() {
        let state = AppState::default();
        assert!(state.should_prompt("Zoom", 300));
    }

    #[test]
    fn test_should_prompt_false_immediately_after_dismiss() {
        let state = AppState::default();
        state.record_dismiss("Zoom".to_string());
        // Cooldown of 300s — immediately after dismiss, should NOT prompt
        assert!(!state.should_prompt("Zoom", 300));
    }

    #[test]
    fn test_should_prompt_true_for_different_platform_after_dismiss() {
        let state = AppState::default();
        state.record_dismiss("Zoom".to_string());
        // Different platform should still get prompted
        assert!(state.should_prompt("Google Meet", 300));
    }

    #[test]
    fn test_clear_dismissed_restores_prompt() {
        let state = AppState::default();
        state.record_dismiss("Zoom".to_string());
        assert!(!state.should_prompt("Zoom", 300));
        state.clear_dismissed();
        assert!(state.should_prompt("Zoom", 300));
    }
}
