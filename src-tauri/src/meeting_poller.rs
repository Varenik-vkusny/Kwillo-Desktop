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
pub const BROWSER_PROCESSES: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe"];

/// Window title substrings that indicate a browser meeting, mapped to display name.
/// These are only checked AFTER mic activity is confirmed, so "Meet - " matching
/// "Google Meet - Brave" (pre-join) is not a problem — no mic = no call.
pub const BROWSER_TITLE_PATTERNS: &[(&str, &str)] = &[
    ("Meet - ", "Google Meet"),
    (" - Google Meet", "Google Meet"),
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
    fn test_match_google_meet_in_call() {
        // When inside a call the title is "Meet - [name] - Brave"
        assert_eq!(
            match_browser_title("Meet - Team Sync - Brave"),
            Some("Google Meet")
        );
    }

    #[test]
    fn test_match_google_meet_prejoin_title() {
        // "Google Meet - Brave" (pre-join) also matches "Meet - " in the title —
        // but in practice is filtered out by browser_with_active_mic() returning false.
        assert_eq!(
            match_browser_title("Google Meet - Brave"),
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

    /// Returns lowercase exe names of all processes currently holding the
    /// microphone open, as reported by the Windows Privacy Consent Store.
    ///
    /// Registry path (per-user, desktop apps):
    ///   HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\
    ///   CapabilityAccessManager\ConsentStore\microphone\NonPackaged\<exe_path>
    ///
    /// Windows sets LastUsedTimeStop = 0 when a process opens the mic and
    /// records the stop FILETIME when it closes it. All-zero stop = active now.
    /// This is an OS-level signal: device-agnostic, process-precise, no audio
    /// stream needed.
    pub fn active_mic_processes_registry() -> Vec<String> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(mic_key) = hkcu.open_subkey(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged",
        ) else {
            eprintln!("[MicDetect] registry: mic consent key not found");
            return Vec::new();
        };

        let mut result = Vec::new();

        for subkey_name in mic_key.enum_keys().filter_map(|k| k.ok()) {
            let Ok(subkey) = mic_key.open_subkey(&subkey_name) else {
                continue;
            };

            // LastUsedTimeStart must be non-zero — proves the process has
            // actually opened the mic at least once in this session.
            let start: Vec<u8> = subkey
                .get_raw_value("LastUsedTimeStart")
                .map(|v| v.bytes)
                .unwrap_or_default();

            if start.iter().all(|&b| b == 0) {
                continue;
            }

            // LastUsedTimeStop == all-zeros (or missing) → mic still open.
            // A non-zero value is a FILETIME recording when the mic was closed.
            let stop: Vec<u8> = subkey
                .get_raw_value("LastUsedTimeStop")
                .map(|v| v.bytes)
                .unwrap_or_default();

            if !stop.iter().all(|&b| b == 0) {
                continue;
            }

            // Key name is the exe path with backslashes replaced by '#'.
            // e.g. "C:#Program Files#...#brave.exe" or "C:#...#zoom.exe#Session:1"
            // We want the last segment ending in ".exe".
            if let Some(exe) = subkey_name
                .split('#')
                .rev()
                .find(|s| s.to_ascii_lowercase().ends_with(".exe"))
            {
                let name = exe.to_ascii_lowercase();
                eprintln!("[MicDetect] registry active: {name}");
                result.push(name);
            }
        }

        result
    }

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
///
/// Detection strategy (in priority order):
/// 1. OS registry mic consent store → which process is actively holding the mic.
///    Device-agnostic: works regardless of which mic the browser/app selected.
///    Process-precise: identifies the exact exe, no audio stream analysis needed.
/// 2. Process name scan → fallback for native apps that may not have touched
///    the mic yet (e.g. joining a Zoom call before unmuting).
#[cfg(target_os = "windows")]
pub fn detect_active_meeting() -> Option<String> {
    // 1. Registry: who is holding the mic open right now?
    let mic_procs = windows_scan::active_mic_processes_registry();

    // Known meeting app is actively using the mic — strongest signal.
    if let Some(platform) = find_meeting_in_processes(&mic_procs) {
        return Some(platform);
    }

    // Browser is using the mic — likely a web call (Meet, Teams web, etc.).
    for name in &mic_procs {
        if BROWSER_PROCESSES.contains(&name.as_str()) {
            return Some("Meeting".to_string());
        }
    }

    // 2. Fallback: native meeting app is running (may be muted / in lobby).
    //    Less precise than mic-usage, but catches the join→unmute gap.
    let all_procs = windows_scan::list_process_names();
    if let Some(platform) = find_meeting_in_processes(&all_procs) {
        return Some(platform);
    }

    None
}
