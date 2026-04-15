# Audio Analyzer — полная логика приложения

## Что это такое

Tauri 2.0 десктоп-приложение для Windows. Rust-бэкенд делает всю системную работу, React-фронтенд — UI. Общение через Tauri IPC (`invoke` = вызов команды, `listen` = подписка на событие).

Приложение живёт в системном трее и молчит, пока не обнаружит активный звонок.

---

## Жизненный цикл — главный поток

```
Запуск
  → приложение в трее, окно скрыто (visible: false)
  → фоновый поток каждые 2 секунды вызывает detect_active_meeting()

Обнаружен звонок (mic активен в браузере)
  → статус: Idle → PendingPermission { platform }
  → событие "meeting-detected" → окно показывается
  → UI: экран разрешения

Пользователь нажал "Record"
  → invoke("start_recording")
  → статус: PendingPermission → Recording { platform, started_at_secs }
  → событие "recording-started" → UI: экран записи с таймером

Пользователь нажал "Skip"
  → invoke("dismiss_recording")
  → статус: PendingPermission → Idle
  → cooldown 5 минут — не предлагать снова для этой платформы
  → окно скрывается

Звонок закончился (mic неактивен 3 тика подряд = ~6 сек) ИЛИ пользователь нажал Stop
  → событие "meeting-ended"
  → invoke("stop_recording", { uploadUrl })
  → статус: Recording → Uploading
  → событие "upload-started" → UI: "Uploading..."

Загрузка на сервер (до 3 попыток, 30 сек между)
  → успех: событие "upload-success", локальный файл удалён
    → UI: "✓ Sent" → через 2 секунды окно скрывается, статус → Idle
  → ошибка: событие "upload-failed" с сообщением
    → UI: "✗ Error: ..."

Диалог открыт, но звонок исчез / 60 секунд без ответа
  → событие "permission-dismissed" → UI сбрасывается, окно скрывается
```

---

## Состояния бэкенда (AppStatus)

```rust
Idle                                          // ничего не происходит
PendingPermission { platform: String }        // показан диалог, ждём юзера
Recording { platform: String, started_at_secs: u64 }  // идёт запись
Uploading                                     // файл загружается
```

Только один статус одновременно. Переходы строго по схеме выше.

---

## Tauri команды (invoke из фронтенда)

| Команда | Когда вызывать | Что делает |
|---|---|---|
| `start_recording` | Пользователь нажал "Record" | Открывает mic + loopback потоки, начинает писать MP3, статус → Recording |
| `stop_recording({ uploadUrl })` | Пользователь нажал "Stop" | Останавливает потоки, флашит MP3, загружает файл, удаляет его |
| `dismiss_recording` | Пользователь нажал "Skip" | Записывает cooldown для платформы, статус → Idle |
| `debug_detection` | Только для отладки | Возвращает `{ browser_mic_active, detected_meeting }` |

---

## Tauri события (listen на фронтенде)

| Событие | Payload | Когда |
|---|---|---|
| `meeting-detected` | `string` (название платформы) | Обнаружен активный звонок, надо показать диалог |
| `recording-started` | `string` (название платформы) | Запись началась, переход к таймеру |
| `meeting-ended` | — | Звонок закончился, начинается загрузка |
| `upload-started` | — | Загрузка пошла |
| `upload-success` | — | Загружено успешно, файл удалён |
| `upload-failed` | `string` (сообщение ошибки) | Все 3 попытки провалились |
| `permission-dismissed` | — | Бэкенд сам закрыл диалог (таймаут или звонок ушёл) |

---

## Детекция звонков

Каждые **2 секунды** фоновый поток проверяет:

1. **Нативные приложения** — ищет в списке процессов: `zoom.exe`, `ms-teams.exe`, `teams.exe`, `slack.exe`, `webex.exe`, `discord.exe`, `skype.exe`

2. **Браузерные звонки** — проверяет WASAPI: есть ли у `brave.exe`, `chrome.exe`, `msedge.exe`, `firefox.exe`, `opera.exe` активная сессия захвата микрофона (state = AudioSessionStateActive). Это единственный надёжный сигнал — браузер открывает mic только когда пользователь нажимает "Join".

После обнаружения:
- ENTRY_DEBOUNCE = 1 тик (реагирует сразу, ~2 сек)
- После нажатия Skip: cooldown 300 сек для этой платформы, сбрасывается когда звонок полностью исчезает

После потери сигнала:
- EXIT_DEBOUNCE = 3 тика (~6 сек) перед остановкой записи — защита от кратких пауз

Диалог без ответа:
- PENDING_TIMEOUT = 60 сек — автоматически закрывается

---

## Захват и кодирование аудио

Два WASAPI потока одновременно:

**Loopback (output device → input)** — захватывает всё что играет через колонки/наушники: голоса собеседников, системные звуки. Это основной поток.

**Microphone (default input device)** — захватывает твой голос.

Смешивание в реальном времени:
- Mic-сэмплы накапливаются в `VecDeque<f32>`
- Loopback callback читает из очереди и суммирует: `(loopback + mic).clamp(-1.0, 1.0)`
- Если mic моно, а loopback стерео — mic-сэмпл дублируется на оба канала
- Если mic недоступен — пишется только loopback

Кодирование: f32 → i16 → LAME MP3 (128 kbps, стерео/моно по конфигу устройства)

Файл: `%TEMP%\audio_analyzer\<uuid>.mp3`

---

## Загрузка файла

`POST multipart/form-data` на `upload_url` с полем `audio`.

URL берётся из `%APPDATA%\audio_analyzer\config.json` → `upload_url`, дефолт: `http://localhost:8080/upload`.

3 попытки, 30 секунд между ними, таймаут 60 секунд на запрос. При успехе локальный файл удаляется. При полном провале — файл остаётся, UI показывает ошибку.

---

## Текущий UI (что сейчас есть)

### Экран разрешения (`PermissionDialog`)
- Показывается когда `view === "permission"`
- Props: `platform: string`, `onDismiss: () => void`
- Кнопка **Record** → `invoke("start_recording")`
- Кнопка **Skip** → `invoke("dismiss_recording")` + `onDismiss()`

### Экран записи (`RecordingIndicator`)
- Показывается когда `view === "recording"`
- Props: `platform`, `status: "recording"|"uploading"|"success"|"error"`, `errorMessage?`
- Живой таймер HH:MM:SS (считает пока `status === "recording"`)
- Кнопка **Stop Recording** → `invoke("stop_recording", { uploadUrl: "http://localhost:8080/upload" })`
- После success/error: "This window will close shortly."

### App.tsx — state machine
```
view: "hidden" | "permission" | "recording" | "uploading"
uploadStatus: "recording" | "uploading" | "success" | "error"
platform: string
```

Маппинг событий:
- `meeting-detected` → `setPlatform(p); setView("permission")`
- `recording-started` → `setView("recording"); setUploadStatus("recording")`
- `upload-started` → `setUploadStatus("uploading")`
- `upload-success` → `setUploadStatus("success")` → через 2 сек `win.hide()`
- `upload-failed` → `setUploadStatus("error"); setErrorMessage(msg)`
- `meeting-ended` → `setUploadStatus("uploading")`
- `permission-dismissed` → `setView("hidden")`

---

## Окно

- Размер: 420×220 px (сейчас — для нового UI можно менять)
- `visible: false` по умолчанию
- `alwaysOnTop: true`
- `resizable: false`
- `center: true`
- `decorations: true` (есть системные рамки)
- Фон: прозрачный (`background: transparent` в CSS)

Для скрытия окна из фронтенда:
```ts
import { getCurrentWindow } from "@tauri-apps/api/window";
const win = getCurrentWindow();
await win.hide();
```

---

## Файловая структура

```
audio_analyzer/
├── src/                          # React фронтенд
│   ├── App.tsx                   # Главный state machine
│   ├── main.tsx                  # Точка входа
│   ├── base.css                  # Reset: transparent body, no margin
│   ├── hooks/
│   │   └── useTauriEvents.ts     # Подписки на все Tauri события
│   └── components/
│       ├── PermissionDialog.tsx  # Экран "Record?"
│       └── RecordingIndicator.tsx # Экран записи с таймером
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs                # Tauri команды, poll loop, tray
│   │   ├── audio_capture.rs      # WASAPI loopback + mic + MP3 encoding
│   │   ├── meeting_poller.rs     # Детекция звонков (процессы + WASAPI mic)
│   │   ├── state.rs              # AppStatus, AppState, cooldown логика
│   │   ├── uploader.rs           # HTTP upload с retry
│   │   └── file_manager.rs       # Temp файлы (UUID .mp3 пути)
│   ├── tauri.conf.json           # Конфиг окна, withGlobalTauri: true
│   ├── capabilities/default.json # Разрешения (core:default + window:allow-hide)
│   └── Cargo.toml
└── test-server/
    └── main.py                   # FastAPI: POST /upload, GET /health
                                  # Сохраняет в test-server/received/
```
