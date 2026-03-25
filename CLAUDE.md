# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GhostWriter is a macOS voice dictation desktop application built with Tauri 2 + Vue 3 + Rust. It records audio via a global hotkey, transcribes speech to text using Whisper, and injects the transcribed text into the active application.

## Commands

```bash
# Development
npm run dev           # Start Vite dev server
npm run tauri dev     # Start Tauri development mode

# Build
npm run build         # Build frontend
npm run tauri build  # Build production Tauri app
```

## Architecture

### Frontend (Vue 3)
- `src/App.vue` - Main settings window with hotkey configuration and debug tools
- `src/components/HotkeyRecorder.vue` - Global hotkey capture component
- `src/components/Hud.vue` - Recording status overlay

### Backend (Rust/Tauri)
- `src-tauri/src/lib.rs` - Main Tauri app setup, command handlers, global shortcut handling
- `src-tauri/src/audio_recorder.rs` - Audio capture using `cpal` crate
- `src-tauri/src/transcriber.rs` - Whisper speech-to-text integration
- `src-tauri/src/injector.rs` - Text injection via keyboard simulation (`enigo`)
- `src-tauri/src/overlay_helper.rs` - macOS HUD overlay management
- `src-tauri/src/config.rs` - Configuration persistence via `tauri-plugin-store`
- `src-tauri/src/logic_helper.rs` - Stop-and-transcribe workflow

### External Dependencies
- `overlay-helper/` - Separate macOS helper app for displaying the recording HUD overlay
- Whisper model: `src-tauri/models/ggml-base.en.bin` (must be placed in resources)

## Key Behaviors

- **Global hotkey**: Configurable shortcut to start/stop recording
- **Press modes**: Short press = toggle mode (press again to stop), Long press (>350ms) = hold mode
- **Recording flow**: Press hotkey → shows HUD → release to continue recording or press again to stop → transcribes → injects text
- **System tray**: App minimizes to tray, accessible via "Open Settings" menu item
- **Permissions**: Requires Microphone and Accessibility permissions on macOS
