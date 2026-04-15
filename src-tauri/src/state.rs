use serde::{Deserialize, Serialize};
use std::sync::Mutex;

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

    pub fn is_recording(&self) -> bool {
        matches!(self.get_status(), AppStatus::Recording { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.get_status(), AppStatus::Idle)
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
}
