# VTT — Voice-to-Text for Windows (MVP Design)

**Date:** 2026-04-09
**Stack:** Tauri 2 + Svelte 5 + Rust
**Target:** Windows 10/11
**Origin:** Port of [FreeFlow](https://github.com/zachlatta/freeflow) (macOS-only Swift app)

---

## 1. Overview

VTT is a Windows system tray application that provides AI-powered voice dictation. The user holds `Ctrl+Shift+Space`, speaks, releases the keys, and the transcribed + cleaned text is pasted into whatever app is focused. It uses Groq's free Whisper API for transcription and an LLM for post-processing cleanup.

### MVP Scope

| In scope | Out of scope (v2+) |
|---|---|
| System tray app | Floating recording overlay |
| `Ctrl+Shift+Space` hold-to-talk | Toggle mode / configurable shortcuts |
| Groq Whisper transcription | Custom LLM providers (Ollama) |
| LLM post-processing (cleanup) | Context awareness (window title, selected text, screenshots) |
| Blind paste (Ctrl+V) | Clipboard preservation/restore |
| First-run API key setup wizard | Voice macros |
| Encrypted settings storage | Pipeline history / run log |
| | Microphone selection UI |
| | Launch at login |

---

## 2. Architecture

### 2.1 Layer Boundaries

```
┌─────────────────────────────────────┐
│  Svelte Frontend (UI only)          │
│  - Setup wizard                     │
│  - Listens to Tauri events          │
│  - Calls Tauri commands             │
└──────────────┬──────────────────────┘
               │ invoke() / listen()
┌──────────────▼──────────────────────┐
│  Tauri Commands (Bridge)            │
│  - validate_api_key                 │
│  - save_settings / get_settings     │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│  Rust Backend (all business logic)  │
│  - Pipeline orchestrator            │
│  - Audio recording (cpal/WASAPI)    │
│  - API clients (reqwest)            │
│  - Clipboard + paste (arboard/enigo)│
│  - Hotkey listener (global-hotkey)  │
│  - Settings persistence             │
└─────────────────────────────────────┘
```

**Rule:** Rust owns all system interaction and business logic. Svelte is purely presentation. No HTTP calls, no filesystem access, no business logic in the frontend.

### 2.2 Project Structure

```
C:\Personal\VTT\
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Tauri entry point, wires modules together
│   │   ├── lib.rs               # Module declarations
│   │   ├── tray.rs              # System tray setup & menu
│   │   ├── hotkey/
│   │   │   ├── mod.rs           # Global hotkey registration & event emission
│   │   │   └── types.rs         # HotkeyEvent enum
│   │   ├── audio/
│   │   │   ├── mod.rs           # Recording orchestration (trait + impl)
│   │   │   ├── recorder.rs      # WASAPI capture via cpal
│   │   │   └── encoder.rs       # PCM f32 buffer → 16kHz mono WAV bytes
│   │   ├── api/
│   │   │   ├── mod.rs           # Service traits (TranscriptionService, PostProcessingService)
│   │   │   ├── transcription.rs # GroqTranscription impl (Whisper endpoint)
│   │   │   └── post_process.rs  # GroqPostProcessing impl (chat completions)
│   │   ├── pipeline/
│   │   │   ├── mod.rs           # DictationPipeline orchestrator
│   │   │   └── types.rs         # PipelineState, PipelineResult, PipelineError
│   │   ├── clipboard/
│   │   │   └── mod.rs           # ClipboardService trait + WindowsClipboard impl
│   │   └── settings/
│   │       ├── mod.rs           # Load/save/encrypt settings
│   │       └── types.rs         # AppSettings struct
│   └── Cargo.toml
├── src/
│   ├── lib/
│   │   ├── components/          # Reusable Svelte components
│   │   └── stores/              # Svelte stores (Tauri command/event bridges)
│   ├── routes/
│   │   └── +page.svelte         # Setup wizard (shown on first run)
│   ├── app.html
│   └── app.css
├── package.json
├── svelte.config.js
├── vite.config.ts
└── tauri.conf.json
```

---

## 3. Module Design

### 3.1 Traits (Dependency Inversion)

Every external service is behind a trait. This enables testing with mocks and future replacement (e.g., swap Groq for Ollama).

```rust
#[async_trait]
pub trait TranscriptionService: Send + Sync {
    async fn transcribe(&self, audio_wav: &[u8]) -> Result<String, ApiError>;
}

#[async_trait]
pub trait PostProcessingService: Send + Sync {
    async fn process(&self, transcript: &str) -> Result<String, ApiError>;
}

#[async_trait]
pub trait AudioRecorder: Send + Sync {
    fn start(&self) -> Result<(), AudioError>;
    async fn stop_and_get_audio(&self) -> Result<Vec<u8>, AudioError>;
    fn is_recording(&self) -> bool;
}

pub trait ClipboardService: Send + Sync {
    fn paste(&self, text: &str) -> Result<(), ClipboardError>;
}
```

### 3.2 Pipeline Orchestrator

Single-responsibility: coordinates the steps, owns no implementation details.

```rust
pub struct DictationPipeline {
    recorder: Arc<dyn AudioRecorder>,
    transcriber: Arc<dyn TranscriptionService>,
    processor: Arc<dyn PostProcessingService>,
    clipboard: Arc<dyn ClipboardService>,
}

impl DictationPipeline {
    pub async fn execute(&self) -> Result<PipelineResult, PipelineError> {
        let audio = self.recorder.stop_and_get_audio().await?;
        let transcript = self.transcriber.transcribe(&audio).await
            .map_err(PipelineError::Transcription)?;
        let cleaned = self.processor.process(&transcript).await
            .map_err(PipelineError::PostProcessing)?;
        self.clipboard.paste(&cleaned)?;
        Ok(PipelineResult { raw: transcript, cleaned })
    }
}
```

### 3.3 Audio Recording

- **Crate:** `cpal` (cross-platform, uses WASAPI on Windows)
- **Format:** Capture at device native rate, resample to 16kHz mono f32 in `encoder.rs` using the `rubato` crate (async sinc resampler)
- **Storage:** In-memory `Vec<f32>` buffer behind a `Mutex`. No temp files.
- **WAV encoding:** `hound` crate converts the f32 buffer to 16-bit PCM WAV bytes for upload.
- **Lifecycle:** `start()` spins up cpal stream, `stop_and_get_audio()` stops stream and returns WAV bytes.

### 3.4 Groq API Clients

**Transcription (Whisper):**
- Endpoint: `POST {base_url}/audio/transcriptions`
- Multipart form: `model=whisper-large-v3`, `file=@audio.wav`
- Auth: `Bearer {api_key}`
- Timeout: 20 seconds
- Response: `{ "text": "..." }`

**Post-processing (Chat Completions):**
- Endpoint: `POST {base_url}/chat/completions`
- Model: `llama-3.3-70b-versatile` (fallback to `llama-3.1-8b-instant` on 429)
- **Fallback mechanism:** `GroqPostProcessing::process()` calls the primary model first. If the response is HTTP 429, it retries once with the fallback model. Any other error propagates immediately — no retry.
- System prompt: Port the FreeFlow post-processing prompt (filler removal, punctuation, self-corrections, email formatting, developer syntax)
- Temperature: 0.0
- Timeout: 20 seconds

**API key validation:**
- `GET {base_url}/models` with `Bearer` header
- 200 = valid, anything else = invalid

### 3.5 Clipboard & Paste

- **Write:** `arboard::Clipboard::set_text()`
- **Simulate Ctrl+V:** `enigo` crate — `enigo.key(Key::Control, Direction::Press)`, `enigo.key(Key::Unicode('v'), Direction::Click)`, `enigo.key(Key::Control, Direction::Release)`
- **Delay:** 50ms between clipboard write and key simulation to ensure the OS has registered the clipboard content

### 3.6 Global Hotkey

- **Crate:** `tauri-plugin-global-shortcut` (Tauri 2 official plugin — integrates with Tauri's event loop, unlike the bare `global-hotkey` crate which conflicts with it)
- **Shortcut:** `Ctrl+Shift+Space`
- **On press:** Start audio recording, emit `pipeline:state { state: "recording" }` event
- **On release:** Trigger `pipeline.execute()`, emit state transitions through the pipeline
- **Re-entrancy guard:** An `AtomicBool` flag (`is_pipeline_running`) prevents a second hotkey press from starting a new pipeline while one is already executing. Press events are silently ignored while the flag is set.
- **Wiring:** `main.rs` connects hotkey events to the pipeline. The hotkey module has no knowledge of recording or transcription.

### 3.7 Settings

```rust
#[derive(Serialize, Deserialize)]
pub struct AppSettings {
    pub api_key_encrypted: String,  // DPAPI-encrypted, base64-encoded
    pub api_base_url: String,       // default: "https://api.groq.com/openai/v1"
    pub has_completed_setup: bool,
}
```

- **Location:** `%APPDATA%\VTT\settings.json`
- **Encryption:** API key encrypted via Windows DPAPI (`CryptProtectData` / `CryptUnprotectData` from the `windows` crate). DPAPI is tied to the Windows user account — only the same user on the same machine can decrypt.
- **Load:** On app start, read and decrypt. If file missing or corrupt, show setup wizard.
- **Save:** Encrypt API key, write to a temporary file in the same directory, then rename over the target (atomic write-then-rename pattern). This prevents corruption if the app crashes mid-write.

---

## 4. Tauri Bridge

### 4.1 Commands (Svelte → Rust)

```rust
#[tauri::command]
async fn validate_api_key(key: String, base_url: String) -> Result<bool, String>;

#[tauri::command]
async fn save_settings(key: String, base_url: String) -> Result<(), String>;

#[tauri::command]
fn get_settings() -> Result<FrontendSettings, String>;

// FrontendSettings is defined in settings/types.rs:
#[derive(Serialize, Deserialize)]
pub struct FrontendSettings {
    pub api_base_url: String,
    pub has_completed_setup: bool,
    // Intentionally omits api_key — never sent to frontend
}
```

### 4.2 Events (Rust → Svelte)

| Event | Payload | When |
|---|---|---|
| `pipeline:state` | `{ state: "idle" \| "recording" \| "transcribing" \| "processing" \| "pasting" }` | Each pipeline stage transition |
| `pipeline:result` | `{ raw: string, cleaned: string }` | Successful completion |
| `pipeline:error` | `{ message: string }` | Any pipeline failure |

---

## 5. Svelte Frontend

### 5.1 Setup Wizard

Single page shown on first run (`has_completed_setup == false`):

1. Text input for Groq API key
2. Optional text input for custom base URL (collapsed by default)
3. "Validate & Save" button
4. On success: saves settings, hides window, starts hotkey listener
5. On failure: shows error message inline

### 5.2 System Tray

- Static icon (microphone or waveform)
- Right-click menu: "Settings..." | separator | "Quit"
- "Settings..." reopens the setup wizard window

### 5.3 Window Behavior

- Main window hidden by default after setup
- App does not appear in taskbar (tray-only)
- Window shown only for setup/settings

---

## 6. Error Handling

### 6.1 Error Types

Each module defines its own error enum via `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("No input device available")]
    NoInputDevice,
    #[error("Recording failed: {0}")]
    RecordingFailed(String),
    #[error("Not currently recording")]
    NotRecording,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Request failed (HTTP {status}): {body}")]
    RequestFailed { status: u16, body: String },
    #[error("Request timed out after {0}s")]
    Timeout(u64),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Network error: {0}")]
    Network(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("Failed to write to clipboard: {0}")]
    WriteFailed(String),
    #[error("Failed to simulate paste: {0}")]
    PasteFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Recording failed: {0}")]
    Audio(#[from] AudioError),
    #[error("Transcription failed: {0}")]
    Transcription(ApiError),
    #[error("Post-processing failed: {0}")]
    PostProcessing(ApiError),
    #[error("Paste failed: {0}")]
    Clipboard(#[from] ClipboardError),
}
```

### 6.2 Error Surfacing

All pipeline errors are emitted as `pipeline:error` events. For MVP, errors are shown as Windows toast notifications via Tauri's notification API. No silent failures.

---

## 7. Rust Crate Dependencies

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-notification = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["multipart", "json"] }
cpal = "0.15"
hound = "3"
rubato = "0.14"             # Audio resampling (device rate → 16kHz)
arboard = "3"
enigo = "0.2"
tauri-plugin-global-shortcut = "2"
async-trait = "0.1"
thiserror = "2"
dirs = "5"
base64 = "0.22"
windows = { version = "0.58", features = ["Win32_Security_Cryptography"] }
```

---

## 8. Post-Processing Prompt

Port the FreeFlow prompt from `PostProcessingService.swift` verbatim. Key behaviors:

- Remove filler, hesitations, duplicate starts, abandoned fragments
- Fix punctuation, capitalization, spacing, ASR mistakes
- Handle self-corrections ("Thursday, no actually Wednesday" → "Wednesday")
- Email formatting (salutation + blank line + body)
- Developer syntax conversion ("underscore" → `_`, "dash dash fix" → `--fix`)
- Multilingual support (preserve mixed-language text)
- Return `EMPTY` for empty/filler-only transcripts

The prompt is a const string in `api/post_process.rs`.

---

## 9. Future Extension Points

The trait-based architecture makes these straightforward to add later:

| Feature | What to add |
|---|---|
| Context awareness | New `ContextService` trait + Win32/UI Automation impl, pass context to `PostProcessingService` |
| Custom LLM (Ollama) | New `OllamaTranscription` / `OllamaPostProcessing` trait impls |
| Toggle mode | Extend hotkey module with state machine (port `DictationShortcutSessionController`) |
| Overlay | New Tauri window with transparent + always-on-top + click-through flags |
| Microphone selection | Expose `cpal` device enumeration via Tauri command |
| Clipboard preservation | Extend `ClipboardService` with `save()` / `restore()` |
| Voice macros | Add macro matching between transcription and post-processing in the pipeline |
