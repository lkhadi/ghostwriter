# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GhostWriter is a macOS voice dictation app: a Tauri 2 + Vue 3 shell around a Rust core that records audio on a global hotkey, transcribes it locally with Whisper, and types the result into whatever app has focus.

macOS is the only working target. The `#[cfg(not(target_os = "macos"))]` branches in `lib.rs` are stale — they call `Mouse::get_mouse_position()` with no matching import (`lib.rs:369`) and reach for a `hud` webview window that `tauri.conf.json` doesn't declare, which also makes `src/components/Hud.vue` dead code on macOS.

## Commands

```bash
npm install
git lfs pull                        # Whisper weights + CoreML encoder are LFS objects

cd overlay-helper && make install    # REQUIRED before running: builds the Obj-C HUD app and
                                     # copies it into src-tauri/overlay-helper/ for bundling

npm run tauri dev                    # run the app (Vite on :1421 + Rust)
npm run tauri build                  # production bundle

cd src-tauri
cargo fmt && cargo clippy && cargo test
cargo test screen_info                # single-module filter
```

Skipping `make install` fails silently at runtime: `OverlayHelper::new()` errors, `AppState.overlay` stays `None`, and dictation works with no HUD.

There is no JS lint/test tooling — `package.json` carries vite scripts only. `cargo test` currently covers `screen_info` alone, and those assertions query the real display, so they need a GUI session.

## Architecture

### Two-process HUD (the non-obvious part)

The recording HUD is **not** a Tauri window. `overlay-helper/` is a standalone Obj-C app (`main.m` + `HUDPanel.m`, built by its own `Makefile`) hosting a borderless `NSPanel` at `kCGMainMenuWindowLevel - 1` with `FullScreenAuxiliary | CanJoinAllSpaces | Stationary` collection behavior — the combination needed to float above fullscreen apps and Electron windows (VS Code). It renders `hud.html` in a `WKWebView`.

Rust drives it over a Unix socket at `/tmp/ghostwriter_overlay.sock` (`overlay_helper.rs` ↔ the `SocketServer` in `main.m`), one fresh connection per command:

1. Helper writes `DIMENSIONS <w> <h>` immediately on connect.
2. Rust sends `SHOW <x> <y>` | `HIDE` | `SET_LEVEL MAIN|FLOATING|STATUS` | `QUIT`.
3. Helper replies `OK\n`.

`OverlayHelper::new()` enforces a singleton: send `QUIT`, `pkill -9 -f GhostWriterOverlayHelper`, delete the socket, then launch — searching `Contents/Resources/overlay-helper/…` (bundled) before `../overlay-helper/…` (dev, where CWD is `src-tauri`).

### Screen dimensions

`screen_info.rs` caches the `DIMENSIONS` the helper pushes on each connect because the `NSScreen` fallback requires a `MainThreadMarker` and returns hardcoded 1920x1080 off the main thread — which the shortcut handler always is. `show_centered_bottom()` positions the 220x60 overlay from those cached values, 100 px above the bottom edge.

### Hotkey state machine (`lib.rs` global-shortcut handler)

- **Pressed** while idle → start recording, optionally mute system audio, stamp `press_time`, show HUD.
- **Pressed** while recording → stop and transcribe (toggle-off).
- **Released** → held > 350 ms stops and transcribes (hold mode); shorter keeps recording (toggle mode).

Both stop paths call `logic_helper::stop_and_transcribe_logic`, which spawns an async task: drain audio → Whisper → restore volume → `enigo` types the text → hide HUD. Its early returns (empty audio, missing transcriber, transcription error) skip the HUD hide, so a no-speech recording leaves the overlay on screen.

### Audio pipeline (`audio_recorder.rs`)

`AudioRecorder::new()` spawns a thread that owns the `cpal` stream and receives `Start`/`Stop` over an mpsc channel, because a `cpal::Stream` isn't `Send`. The input callback keeps channel 0 only, pushes 1024-sample chunks through a `rubato::SincFixedIn` resampler to 16 kHz, and appends to an `Arc<Mutex<VecDeque<f32>>>` capped at 5 minutes — oldest samples are dropped at capacity. `get_audio()` drains it.

### Transcription (`transcriber.rs`)

`WhisperContext` is built once in `setup()` and held in `AppState`; the language code is passed per call. Output is aggressively post-filtered for Whisper hallucinations (music glyphs, "Subtitles by …", bare "you", non-alphanumeric output, under 2 chars) and returns an empty string rather than injecting junk.

**Model naming trap:** `src-tauri/models/ggml-base.en.bin` holds the *multilingual* `ggml-base` weights. The `.en` name was kept so the hardcoded path and the sibling `ggml-base.en-encoder.mlmodelc` keep resolving — whisper-rs's `coreml` feature derives the encoder path from the model filename. The path is hardcoded twice in `lib.rs` (setup and `transcribe_test_audio`).

### Config (`config.rs`)

Everything lives under a single `"config"` key in the `config.json` store as one `AppConfig` (`hotkey`, `auto_mute_enabled`, `language`). Each setter reads the existing config, rebuilds the whole struct field by field, and saves. **Adding a field means touching `AppConfig`, its `Default`, the migration in `init_store`, and every `set_*` command** — miss one and that setter silently resets the new field.

### Auto-mute (`audio_control.rs`)

Volume is read and set by shelling out to `osascript`; the pre-mute level is stashed in `AppState.previous_volume`. Restore floors at 30 when the previous volume was under 20.

### Frontend

A single `App.vue` switches on the webview label (`main` vs `hud`) and drives everything through `invoke`. Every Rust command must be listed in the `invoke_handler!` block at the bottom of `lib.rs`. The `save_test_audio` / `transcribe_test_audio` / `inject_test_text` trio backs the "Debug Tools" panel and round-trips through `~/Desktop/test_audio.wav`.

## Conventions

- Design and implementation plans go in `docs/plans/YYYY-MM-DD-<topic>.md` before non-trivial work.
- Conventional commits (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`); feature branch → PR into `main`.
- `*.bin` is git-lfs tracked.
- Rust logs via `println!`/`eprintln!` to stdout; helper logs land in Console.app (filter `GhostWriterOverlay`).
- Root-level `test-*.sh` and `check_*.m` are ad-hoc window-level debugging aids, not a test suite.
- Requires Microphone and Accessibility permissions; usage strings live in `src-tauri/Info.plist`.
