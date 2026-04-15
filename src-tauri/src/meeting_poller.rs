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
