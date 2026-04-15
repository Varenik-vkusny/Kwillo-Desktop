# Sliding Window Meeting Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `consecutive_detections` counter in the meeting poll loop with a sliding window so that brief/spurious mic activity no longer triggers the permission prompt.

**Architecture:** Extract a pure `window_is_active` helper for testability, then update `start_poll_loop` to use a `VecDeque<bool>` instead of a bare counter. Constants `ENTRY_DEBOUNCE` is removed; `WINDOW_SIZE` and `ENTRY_THRESHOLD` take its place.

**Tech Stack:** Rust, Tauri 2.x, `std::collections::VecDeque`

---

### Task 1: Extract and test the window decision logic

**Files:**
- Modify: `src-tauri/src/lib.rs`

The window threshold check is a pure function — extract it so it can be unit-tested without spinning up a Tauri app.

- [ ] **Step 1: Add `window_is_active` helper and its tests to `lib.rs`**

Open `src-tauri/src/lib.rs`. After the constants block (after line 37, before the Tauri commands) add:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they all pass**

```bash
cd src-tauri && cargo test window_is_active -- --nocapture
```

Expected output: all 8 tests pass, no compile errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "test: add unit tests for window_is_active helper"
```

---

### Task 2: Replace constants

**Files:**
- Modify: `src-tauri/src/lib.rs` (constants block, lines 19–37)

- [ ] **Step 1: Remove `ENTRY_DEBOUNCE`, add `WINDOW_SIZE` and `ENTRY_THRESHOLD`**

Find this block in `lib.rs`:

```rust
/// Consecutive detections required before showing the permission prompt.
/// Mic-based detection is a hard signal (no false positives), so 1 tick is enough.
const ENTRY_DEBOUNCE: u32 = 1;
```

Replace it with:

```rust
/// Number of ticks in the sliding entry-detection window.
/// 6 ticks × 2 s = 12-second observation window.
const WINDOW_SIZE: usize = 6;

/// Minimum number of Active ticks within the window required to show the prompt.
/// 4 of 6 (~66%) tolerates natural mic silence during a real call while
/// filtering out brief ambient activations that only last 1–2 ticks.
const ENTRY_THRESHOLD: usize = 4;
```

- [ ] **Step 2: Verify compile**

```bash
cd src-tauri && cargo check
```

Expected: compile error because `ENTRY_DEBOUNCE` is still referenced in `start_poll_loop` — that's fine, we fix it in Task 3.

---

### Task 3: Update the poll loop

**Files:**
- Modify: `src-tauri/src/lib.rs` (function `start_poll_loop`)

- [ ] **Step 1: Replace `consecutive_detections` declaration with `detection_window`**

In `start_poll_loop`, find:

```rust
        // How many consecutive ticks we've seen a meeting while Idle.
        let mut consecutive_detections: u32 = 0;
```

Replace with:

```rust
        // Sliding window of the last WINDOW_SIZE detection ticks (true = meeting detected).
        // We prompt only when enough ticks are Active — see window_is_active().
        let mut detection_window: std::collections::VecDeque<bool> =
            std::collections::VecDeque::with_capacity(WINDOW_SIZE);
```

- [ ] **Step 2: Replace the Idle branch**

Find the entire `AppStatus::Idle` arm (from `AppStatus::Idle => {` through its closing `}`):

```rust
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
```

Replace the entire arm with:

```rust
                // ── Idle: watch for a new meeting to start ─────────────────
                AppStatus::Idle => {
                    missed_detections = 0;

                    let detected = meeting_poller::detect_active_meeting();

                    // Slide the window: record whether this tick detected a meeting.
                    detection_window.push_back(detected.is_some());
                    if detection_window.len() > WINDOW_SIZE {
                        detection_window.pop_front();
                    }

                    match detected {
                        Some(platform) => {
                            // Only prompt when the window has enough Active ticks AND
                            // we're not in a dismiss cooldown for this platform.
                            if window_is_active(&detection_window, ENTRY_THRESHOLD)
                                && app_state.should_prompt(&platform, DISMISS_COOLDOWN_SECS)
                            {
                                // Clear window so stale ticks don't carry over into
                                // the next detection cycle after this one ends.
                                detection_window.clear();
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
                        }
                        None => {
                            // Meeting is gone → clear the dismiss cooldown so the
                            // next time this platform is detected (new session) we
                            // will prompt again.
                            app_state.clear_dismissed();
                        }
                    }
                }
```

- [ ] **Step 3: Verify compile and tests pass**

```bash
cd src-tauri && cargo test -- --nocapture
```

Expected: all tests pass, no compile errors or warnings about unused variables.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: replace consecutive_detections with sliding window (4/6 ticks)"
```

---

### Task 4: Manual smoke test

No automated test can cover the live WASAPI loop — verify the behaviour manually.

- [ ] **Step 1: Start the app in dev mode**

```bash
cargo tauri dev
```

- [ ] **Step 2: Verify real-call detection still works**

Open Google Meet (or any browser-based call) and actually join a call. The permission prompt should appear within ~12 seconds of joining.

- [ ] **Step 3: Verify false-positive suppression**

Open a browser tab with mic access (e.g. navigate to a site that asks for mic, grant permission, but don't start a call). Make some noise near the laptop. The prompt should NOT appear within 30 seconds.

- [ ] **Step 4: Check `debug_detection` output if needed**

In browser devtools console while the app is running:

```js
await window.__TAURI__.core.invoke('debug_detection')
```

This returns `{ browser_mic_active: bool, detected_meeting: string|null }` — useful to confirm the underlying WASAPI signal when debugging.
