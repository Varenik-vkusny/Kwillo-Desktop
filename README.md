# Audio Analyzer

Desktop app (Windows) that detects active meetings and records system audio for transcription.

## How it works

1. App runs in the system tray, polling for active meetings every 2 seconds
2. When a meeting is detected (Zoom, Teams, Google Meet, Slack, etc.), a permission dialog appears
3. Click "Record" to start capturing system audio (WASAPI loopback — all meeting participants)
4. When the meeting ends, audio is automatically uploaded to the server and the local file is deleted

## Prerequisites

- Rust (via [rustup.rs](https://rustup.rs))
- Node.js 18+ (via [nodejs.org](https://nodejs.org))
- Microsoft C++ Build Tools (Visual Studio 2022, "Desktop development with C++")
- Python 3.9+ (for test server)

## Development Setup

```bash
npm install
```

## Running

Start the test server (separate terminal):
```bash
cd test-server
pip install -r requirements.txt
python main.py
```

Start the app:
```bash
cargo tauri dev
```

## Supported Meeting Platforms

**Native apps:** Zoom, Microsoft Teams, Slack, Cisco Webex, Skype, Discord

**Browser (Chrome/Edge/Firefox):** Google Meet, Microsoft Teams Web, Zoom Web, Webex Web, Whereby

## Configuration

Upload URL can be configured in `%APPDATA%\audio_analyzer\config.json`:
```json
{ "upload_url": "http://your-server/upload" }
```

Default: `http://localhost:8080/upload`
