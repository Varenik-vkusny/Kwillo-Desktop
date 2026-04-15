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

    /// Looks up the exe name for a given PID using a process snapshot.
    fn get_process_name_by_pid(target_pid: u32) -> Option<String> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == target_pid {
                        let end = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_string();
                        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
                        return Some(name);
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snapshot);
            None
        }
    }

    /// Returns true if any browser process currently holds an open WASAPI
    /// microphone capture session — regardless of Active/Inactive state.
    ///
    /// A browser opens the capture endpoint when the user joins a call and
    /// holds it for the entire session. Checking for session existence (not
    /// Active state) is more reliable than checking Active state, which only
    /// fires when audio is literally flowing (i.e. during speech, not silence).
    pub fn browser_has_mic_session() -> bool {
        let result = browser_has_mic_session_inner();
        eprintln!("[MicDetect] browser_has_mic_session = {result}");
        result
    }

    fn browser_has_mic_session_inner() -> bool {
        use windows::Win32::Media::Audio::{
            DEVICE_STATE_ACTIVE, IAudioSessionControl, IAudioSessionControl2,
            IAudioSessionManager2, IMMDeviceCollection, IMMDeviceEnumerator,
            MMDeviceEnumerator, eCapture,
        };
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
        };
        #[allow(unused_imports)]
        use windows::core::Interface;

        const BROWSERS: &[&str] =
            &["chrome.exe", "brave.exe", "msedge.exe", "firefox.exe", "opera.exe"];

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let Ok(enumerator): Result<IMMDeviceEnumerator, _> =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                eprintln!("[MicDetect] FAIL: CoCreateInstance IMMDeviceEnumerator");
                return false;
            };

            // Enumerate ALL active capture endpoints, not just the default.
            // Browsers often capture from a non-default device (headset, USB mic,
            // virtual mic) — the default endpoint lookup would miss those sessions.
            let Ok(collection): Result<IMMDeviceCollection, _> =
                enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            else {
                eprintln!("[MicDetect] FAIL: EnumAudioEndpoints");
                return false;
            };

            let Ok(device_count) = collection.GetCount() else {
                eprintln!("[MicDetect] FAIL: collection GetCount");
                return false;
            };

            eprintln!("[MicDetect] capture endpoint count = {device_count}");

            for d in 0..device_count {
                let Ok(device) = collection.Item(d) else { continue; };

                let Ok(manager): Result<IAudioSessionManager2, _> =
                    device.Activate(CLSCTX_ALL, None)
                else {
                    continue;
                };

                let Ok(sessions) = manager.GetSessionEnumerator() else { continue; };
                let Ok(count) = sessions.GetCount() else { continue; };

                eprintln!("[MicDetect] endpoint {d}: session count = {count}");

                for i in 0..count {
                    let Ok(session): Result<IAudioSessionControl, _> = sessions.GetSession(i)
                    else {
                        continue;
                    };

                    let Ok(s2): Result<IAudioSessionControl2, _> = session.cast() else {
                        continue;
                    };

                    let Ok(pid) = s2.GetProcessId() else { continue; };

                    let name =
                        get_process_name_by_pid(pid).unwrap_or_else(|| format!("pid:{pid}"));
                    let state = session.GetState().ok();
                    eprintln!(
                        "[MicDetect] endpoint {d} session {i}: pid={pid} name={name} state={state:?}"
                    );

                    if BROWSERS.contains(&name.to_ascii_lowercase().as_str()) {
                        return true;
                    }
                }
            }

            false
        }
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
/// Detection is purely signal-based — no window title reading:
/// 1. Native apps (Zoom, Teams, etc.) → detected by process name
/// 2. Browser calls (Google Meet, etc.) → detected by WASAPI mic ownership.
///    The browser opens the capture device only when the user clicks "Join",
///    so this fires at exactly the right moment regardless of tab focus or URL.
#[cfg(target_os = "windows")]
pub fn detect_active_meeting() -> Option<String> {
    let processes = windows_scan::list_process_names();

    // 1. Native meeting apps — process name is unambiguous
    if let Some(platform) = find_meeting_in_processes(&processes) {
        return Some(platform);
    }

    // 2. Browser holds an open mic capture session = call in progress.
    //    We check for session existence (not Active state) because browsers
    //    keep the mic endpoint open for the entire call — Active/Inactive only
    //    reflects whether audio is literally flowing at this instant.
    if windows_scan::browser_has_mic_session() {
        return Some("Meeting".to_string());
    }

    None
}
