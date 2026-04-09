# LipService

Free and open-source voice-to-text for Windows. Hold a hotkey, speak, release — your words get transcribed, cleaned up by AI, and pasted into whatever app you're using.

Inspired by [FreeFlow](https://github.com/zachlatta/freeflow) (macOS), rebuilt from scratch for Windows with Tauri + Rust + Svelte.

## How it works

1. Hold **Ctrl+Shift+Space** and talk
2. Release — audio is sent to [Groq](https://groq.com/) for transcription (Whisper)
3. An LLM cleans up the transcript (removes filler, fixes punctuation, handles self-corrections)
4. Cleaned text is pasted into your active app

No server, no subscription. Your audio goes directly to Groq's API (free tier available). Nothing is stored or retained.

## Setup

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable, MSVC target)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++"
- A free [Groq API key](https://console.groq.com/)

### Build & Run

```bash
cd C:\Personal\VTT
npm install
npm run tauri dev
```

On first launch, enter your Groq API key in the setup wizard. After that, the app lives in your system tray.

### Build Release

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/`.

## Architecture

```
Svelte (UI only)          -- setup wizard, listens to events
  |
Tauri Commands            -- validate_api_key, save_settings
  |
Rust Backend              -- all business logic
  |- audio/               -- cpal WASAPI recording + rubato resampling + WAV encoding
  |- api/                 -- Groq Whisper transcription + LLM post-processing
  |- clipboard/           -- arboard clipboard + enigo Ctrl+V simulation
  |- pipeline/            -- orchestrator (record -> transcribe -> process -> paste)
  |- settings/            -- DPAPI-encrypted JSON storage
  |- hotkey/              -- global shortcut registration
  |- tray.rs              -- system tray menu
```

All external services are behind traits for testability and future extensibility (e.g., swap Groq for Ollama).

## Post-processing

The LLM cleanup handles:
- Filler removal ("um", "uh", hesitations)
- Self-corrections ("Thursday, no actually Wednesday" -> "Wednesday")
- Punctuation and capitalization
- Email formatting (salutation + body)
- Developer syntax ("underscore" -> `_`, "dash dash fix" -> `--fix`)
- Multilingual text preservation

## License

MIT
