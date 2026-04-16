## Build
- Run `cargo check` / `cargo tauri dev` from `src-tauri/` dir, or use `cargo tauri dev` from project root
- Linker error `link.exe not found`: VS 2022 Community is installed but C++ workload may be missing — open VS Installer → Modify → add "Desktop development with C++"
- `cargo tauri dev` compiles Rust + starts Vite dev server on port 1420 simultaneously

## Windows API gotchas
- `IAudioSessionControl2::IsSystemSoundsSession()` in windows-rs: both S_OK and S_FALSE return `Ok(())` — do NOT use `.is_ok()` to filter system sessions, it filters everything. Rely on process name instead.
- `CoInitializeEx` must be called on each thread that uses COM; poll loop thread needs its own call
- WASAPI loopback: call `build_input_stream` on the OUTPUT device (not input) to capture system audio
- `cpal::Stream` is `!Send` due to `PhantomData<*mut ()>` — requires `unsafe impl Send/Sync` on wrapper structs

## Tauri 2.x specifics
- `.emit()` on `AppHandle` requires `use tauri::Emitter` in scope (not in Tauri 1.x)
- `withGlobalTauri: true` in `tauri.conf.json` required for `window.__TAURI__` in devtools
- Capabilities file needs explicit `"core:window:allow-hide"` for `win.hide()` to work
- Vite 7 rejects inline `<style>` in `index.html` — import CSS from `main.tsx` instead

## Meeting detection
- Primary signal: WASAPI mic session owned by browser process (brave.exe/chrome.exe) = call in progress
- Brave browser process name: `brave.exe` (add to BROWSER_PROCESSES list)
- `AudioSessionState(0)` = Inactive, `(1)` = Active — mic state fluctuates during call (silence = inactive)
- ENTRY_DEBOUNCE=1 is fine for mic-based detection (hard signal, no false positives)

## mp3lame-encoder 0.1 API
- `encode()` takes pre-allocated `&mut [MaybeUninit<u8>]`, returns `Result<usize>` (bytes written)
- Use `max_required_buffer_size(num_samples)` to size the buffer
- Flush needs minimum 7200 byte buffer
