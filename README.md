# GhostWriter

A macOS voice dictation desktop application built with Tauri 2 + Vue 3 + Rust. It records audio via a global hotkey, transcribes speech to text using Whisper, and injects the transcribed text into the active application.

> **Platform status**: macOS only. Earlier versions carried `cfg(not(target_os = "macos"))` branches, but they referenced an unimported symbol and a window the app never declared, so they could not compile and never ran — they have been removed rather than left as a misleading promise.
>
> The record → transcribe → inject core does use cross-platform libraries (cpal, whisper-rs, enigo). A real port would need: a replacement for the Objective-C `NSPanel` HUD helper and its Unix-socket protocol, a substitute for the `osascript` auto-mute, and platform equivalents for the Accessibility and Microphone permission flows.

## Features

- **Global Hotkey**: Configurable shortcut to start/stop recording
- **Press Modes**:
  - Short press = toggle mode (press again to stop)
  - Long press (>350ms) = hold mode
- **Recording HUD**: Visual overlay showing recording status (macOS native overlay, webview fallback on other platforms)
- **Whisper Transcription**: Offline speech-to-text using Whisper model
- **Text Injection**: Automatically types transcribed text into the active application
- **System Tray**: App minimizes to tray for quick access

## Prerequisites

- macOS (requires Microphone and Accessibility permissions)
- [Node.js](https://nodejs.org/) v18+
- [Rust](https://www.rust-lang.org/) latest stable
- Whisper model: `src-tauri/models/ggml-base.bin` (place in resources)

## Setup

1. Install dependencies:
   ```bash
   npm install
   ```

2. Download the Whisper model and place it in `src-tauri/models/`

## Development

```bash
# Start Vite dev server
npm run dev

# Start Tauri development mode
npm run tauri dev
```

## Build

```bash
# Build frontend
npm run build

# Build production Tauri app
npm run tauri build
```

## Architecture

### Frontend (Vue 3)
- `src/App.vue` - Main settings window with hotkey configuration
- `src/components/HotkeyRecorder.vue` - Global hotkey capture component
- `src/components/Hud.vue` - Recording status overlay

### Backend (Rust/Tauri)
- `src-tauri/src/lib.rs` - Main app setup, command handlers, global shortcut handling
- `src-tauri/src/audio_recorder.rs` - Audio capture using `cpal` crate
- `src-tauri/src/transcriber.rs` - Whisper speech-to-text integration
- `src-tauri/src/injector.rs` - Text injection via keyboard simulation (`enigo`)
- `src-tauri/src/overlay_helper.rs` - macOS HUD overlay management
- `src-tauri/src/config.rs` - Configuration persistence via `tauri-plugin-store`
- `src-tauri/src/logic_helper.rs` - Stop-and-transcribe workflow

### External Dependencies
- `overlay-helper/` - Separate macOS helper app for displaying the recording HUD overlay (macOS only)

### Cross-Platform Libraries
- **enigo** - Keyboard simulation (Windows, macOS, Linux)
- **cpal** - Cross-platform audio capture
- **Whisper** - Offline speech-to-text transcription

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
