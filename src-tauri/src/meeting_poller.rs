// Meeting detection — implemented in Tasks 6 & 7

/// Known meeting app process names (lowercase) mapped to display name.
pub const MEETING_PROCESSES: &[(&str, &str)] = &[
    ("zoom.exe", "Zoom"),
    ("ms-teams.exe", "Microsoft Teams"),
    ("teams.exe", "Microsoft Teams"),
    ("slack.exe", "Slack"),
    ("webex.exe", "Cisco Webex"),
    ("ciscowebexmeetings.exe", "Cisco Webex Meetings"),
    ("skype.exe", "Skype"),
    ("discord.exe", "Discord"),
];

/// Browser process names that we inspect window titles of.
pub const BROWSER_PROCESSES: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe"];

/// Window title substrings that indicate a browser meeting, mapped to display name.
pub const BROWSER_TITLE_PATTERNS: &[(&str, &str)] = &[
    (" - Google Meet", "Google Meet"),
    ("Meet - ", "Google Meet"),
    ("Microsoft Teams", "Microsoft Teams"),
    ("Zoom Meeting", "Zoom"),
    ("Webex", "Cisco Webex"),
    ("Whereby", "Whereby"),
];

/// Returns Some(platform_name) if the process name matches a known meeting app.
/// Comparison is case-insensitive.
pub fn match_process_name(process_name: &str) -> Option<&'static str> {
    let lower = process_name.to_ascii_lowercase();
    MEETING_PROCESSES
        .iter()
        .find(|(p, _)| *p == lower.as_str())
        .map(|(_, name)| *name)
}

/// Returns Some(platform_name) if the window title contains a meeting platform pattern.
pub fn match_browser_title(title: &str) -> Option<&'static str> {
    BROWSER_TITLE_PATTERNS
        .iter()
        .find(|(pattern, _)| title.contains(pattern))
        .map(|(_, name)| *name)
}

/// Returns Some(platform_name) if any process in the list is a known meeting app.
pub fn find_meeting_in_processes(process_names: &[String]) -> Option<String> {
    for name in process_names {
        if let Some(platform) = match_process_name(name) {
            return Some(platform.to_string());
        }
    }
    None
}

/// Returns Some(platform_name) if any title belongs to a browser meeting.
pub fn find_meeting_in_titles(titles: &[String]) -> Option<String> {
    for title in titles {
        if let Some(platform) = match_browser_title(title) {
            return Some(platform.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_zoom_lowercase() {
        assert_eq!(match_process_name("zoom.exe"), Some("Zoom"));
    }

    #[test]
    fn test_match_zoom_uppercase() {
        assert_eq!(match_process_name("ZOOM.EXE"), Some("Zoom"));
    }

    #[test]
    fn test_match_ms_teams() {
        assert_eq!(match_process_name("ms-teams.exe"), Some("Microsoft Teams"));
    }

    #[test]
    fn test_match_teams_legacy() {
        assert_eq!(match_process_name("teams.exe"), Some("Microsoft Teams"));
    }

    #[test]
    fn test_match_discord() {
        assert_eq!(match_process_name("discord.exe"), Some("Discord"));
    }

    #[test]
    fn test_no_match_notepad() {
        assert_eq!(match_process_name("notepad.exe"), None);
    }

    #[test]
    fn test_no_match_spotify() {
        assert_eq!(match_process_name("spotify.exe"), None);
    }

    #[test]
    fn test_no_match_chrome_itself() {
        assert_eq!(match_process_name("chrome.exe"), None);
    }

    #[test]
    fn test_match_google_meet_suffix() {
        assert_eq!(
            match_browser_title("Daily Standup - Google Meet"),
            Some("Google Meet")
        );
    }

    #[test]
    fn test_match_google_meet_prefix() {
        assert_eq!(
            match_browser_title("Meet - Team Sync"),
            Some("Google Meet")
        );
    }

    #[test]
    fn test_match_webex_in_title() {
        assert_eq!(
            match_browser_title("Cisco Webex Meeting"),
            Some("Cisco Webex")
        );
    }

    #[test]
    fn test_no_match_youtube_title() {
        assert_eq!(match_browser_title("YouTube - Music"), None);
    }

    #[test]
    fn test_no_match_empty_title() {
        assert_eq!(match_browser_title(""), None);
    }

    #[test]
    fn test_find_meeting_in_process_list() {
        let processes = vec![
            "explorer.exe".to_string(),
            "chrome.exe".to_string(),
            "zoom.exe".to_string(),
            "notepad.exe".to_string(),
        ];
        assert_eq!(
            find_meeting_in_processes(&processes),
            Some("Zoom".to_string())
        );
    }

    #[test]
    fn test_find_no_meeting_in_normal_process_list() {
        let processes = vec!["explorer.exe".to_string(), "chrome.exe".to_string()];
        assert_eq!(find_meeting_in_processes(&processes), None);
    }

    #[test]
    fn test_find_meeting_in_titles() {
        let titles = vec![
            "New Tab - Chrome".to_string(),
            "Gmail".to_string(),
            "Daily Standup - Google Meet".to_string(),
        ];
        assert_eq!(
            find_meeting_in_titles(&titles),
            Some("Google Meet".to_string())
        );
    }
}

// ─── Windows API integration ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub mod windows_scan {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};

    /// Returns the exe names of all currently running processes.
    pub fn list_process_names() -> Vec<String> {
        let mut names = Vec::new();
        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return names,
            };

            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let end = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                    names.push(name);

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        }
        names
    }

    /// Returns titles of all visible top-level windows.
    pub fn list_visible_window_titles() -> Vec<String> {
        let mut titles: Vec<String> = Vec::new();
        let ptr = &mut titles as *mut Vec<String> as isize;

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let titles = &mut *(lparam.0 as *mut Vec<String>);
            if IsWindowVisible(hwnd).as_bool() {
                let mut buf = [0u16; 512];
                let len = GetWindowTextW(hwnd, &mut buf);
                if len > 0 {
                    titles.push(String::from_utf16_lossy(&buf[..len as usize]).to_string());
                }
            }
            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(ptr));
        }
        titles
    }
}

/// Detects whether any meeting is currently active.
/// Returns Some(platform_name) or None.
/// Only compiled on Windows.
#[cfg(target_os = "windows")]
pub fn detect_active_meeting() -> Option<String> {
    let processes = windows_scan::list_process_names();

    // Check native meeting apps first
    if let Some(platform) = find_meeting_in_processes(&processes) {
        return Some(platform);
    }

    // Only scan window titles if a browser is running
    let browser_running = processes
        .iter()
        .any(|p| BROWSER_PROCESSES.contains(&p.to_ascii_lowercase().as_str()));

    if browser_running {
        let titles = windows_scan::list_visible_window_titles();
        if let Some(platform) = find_meeting_in_titles(&titles) {
            return Some(platform);
        }
    }

    None
}
