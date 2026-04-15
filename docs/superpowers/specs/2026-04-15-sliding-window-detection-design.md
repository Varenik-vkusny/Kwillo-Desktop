# Sliding Window Meeting Detection

**Date:** 2026-04-15  
**Status:** Approved

## Problem

The app occasionally prompts for recording permission when the user is not in a meeting. The root cause: `AudioSessionStateActive` in WASAPI fires whenever a browser tab is actively reading from the microphone — not just during calls. A tab with mic permissions can become Active due to ambient noise, a pre-join lobby, or other non-call audio usage. With `ENTRY_DEBOUNCE = 1`, a single 2-second Active tick is enough to trigger the prompt.

## Solution

Replace the `consecutive_detections` counter with a sliding window of the last N ticks. Prompt only when the ratio of Active ticks exceeds a threshold. This handles two problems simultaneously:

1. **False positives**: brief/sporadic mic activity (1–2 ticks) does not reach the threshold.
2. **Real call fluctuation**: mic goes Inactive during silences — a hard consecutive counter would reset; a window tolerates gaps.

## Design

### Constants (in `lib.rs`)

```rust
/// Number of ticks in the sliding detection window (6 × 2s = 12 seconds).
const WINDOW_SIZE: usize = 6;

/// Minimum Active ticks within the window required to prompt (~66%).
const ENTRY_THRESHOLD: usize = 4;
```

`ENTRY_DEBOUNCE` is removed — replaced by the window logic.

### State change in `start_poll_loop`

Replace:
```rust
let mut consecutive_detections: u32 = 0;
```

With:
```rust
use std::collections::VecDeque;
let mut detection_window: VecDeque<bool> = VecDeque::with_capacity(WINDOW_SIZE);
```

### Idle branch logic

Replace the `consecutive_detections` increment/check with:

```rust
AppStatus::Idle => {
    let detected = meeting_poller::detect_active_meeting();

    // Push current tick into the sliding window
    detection_window.push_back(detected.is_some());
    if detection_window.len() > WINDOW_SIZE {
        detection_window.pop_front();
    }

    let active_count = detection_window.iter().filter(|&&v| v).count();

    if let Some(platform) = detected {
        if active_count >= ENTRY_THRESHOLD && app_state.should_prompt(&platform, DISMISS_COOLDOWN_SECS) {
            detection_window.clear();
            pending_since = Some(std::time::Instant::now());
            app_state.set_status(AppStatus::PendingPermission { platform: platform.clone() });
            app.emit("meeting-detected", platform).ok();
            if let Some(window) = app.get_webview_window("popup") {
                window.show().ok();
                window.set_focus().ok();
            }
        }
    } else {
        // Meeting gone — clear cooldown so next session re-prompts
        app_state.clear_dismissed();
    }
}
```

The window is also cleared when transitioning out of Idle (on prompt trigger) to avoid stale state bleeding into the next detection cycle.

## Trade-offs

| | Old (ENTRY_DEBOUNCE=1) | New (sliding window 4/6) |
|---|---|---|
| False positive resistance | Low (1 tick = 2s) | High (needs 8s+ of sustained activity) |
| Handles mic fluctuation | No (resets on silence) | Yes (tolerates 2 Inactive ticks per window) |
| Detection latency for real calls | ~2s | ~8–12s first time |
| Code complexity | Minimal | Low (VecDeque + count) |

Detection latency for real calls increases to ~8–12 seconds on first join, which is acceptable — the user is still in the process of joining when the prompt appears.

## Files Changed

- `src-tauri/src/lib.rs` — sliding window logic in `start_poll_loop`, remove `ENTRY_DEBOUNCE`, add `WINDOW_SIZE`/`ENTRY_THRESHOLD`
