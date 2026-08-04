# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GhostWriter is a macOS voice dictation app: a Tauri 2 + Vue 3 shell around a Rust core that records audio on a global hotkey, transcribes it locally with Whisper, and types the result into whatever app has focus.

macOS only, and honestly so — the `cfg(not(target_os = "macos"))` branches were deleted rather than left as a promise the code could not keep (they referenced an unimported symbol and a window `tauri.conf.json` never declared, so they could not compile). `tauri.conf.json` declares exactly one window, `main`.

## Commands

```bash
npm install
git lfs pull                        # the Whisper weights are an LFS object

cd overlay-helper && make install    # REQUIRED before running: builds the Obj-C HUD app and
                                     # copies it into src-tauri/overlay-helper/ for bundling

npm run tauri dev                    # run the app (Vite on :1421 + Rust)
npm run tauri build                  # production bundle

cd src-tauri
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib
cargo test transcriber                # single-module filter
```

Skipping `make install` fails silently at runtime: `OverlayHelper::new()` errors, `AppState.overlay` stays `None`, and dictation works with no HUD.

There is no JS lint/test tooling — `package.json` carries vite scripts only. `cargo test --lib` covers the transcript hallucination filter and the shortcut-string grammar; both are pure functions, so they need no display or device.

## Architecture

### Two-process HUD (the non-obvious part)

The recording HUD is **not** a Tauri window. `overlay-helper/` is a standalone Obj-C app (`main.m` + `HUDPanel.m`, built by its own `Makefile`) hosting a borderless `NSPanel` at `kCGMainMenuWindowLevel - 1` with `FullScreenAuxiliary | CanJoinAllSpaces | Stationary` collection behavior — the combination needed to float above fullscreen apps and Electron windows (VS Code). It renders `hud.html` in a `WKWebView`.

Rust drives it over a Unix socket in `$TMPDIR` (`overlay_helper.rs::socket_path()` ↔ `socketPath` in `main.m` — the two must stay in sync), one fresh connection per command:

1. Rust sends `SHOW_CENTERED` | `SHOW <x> <y>` | `HIDE` | `QUIT`.
2. Helper replies `OK\n`.

**The helper owns all geometry.** `SHOW_CENTERED` exists because it runs on the main thread with real `NSScreen` access, including `visibleFrame.origin`, which is routinely non-zero or negative on a multi-display setup. Rust holds no screen state at all. `SHOW x y` survives only for the `test-*.sh` scripts.

`OverlayHelper::new()` enforces a singleton: send `QUIT`, `pkill -9 -f GhostWriterOverlayHelper`, delete the socket, then launch — searching `Contents/Resources/overlay-helper/…` (bundled) before `../overlay-helper/…` (dev, where CWD is `src-tauri`). The `Child` is retained so it can be reaped.

### Hotkey state machine (`lib.rs`)

`DictationState` behind one mutex is the single source of truth. It replaced two independent spawned handlers that inferred state from `press_time` plus `AudioRecorder::is_recording()` — and that flag is flipped *asynchronously by the audio thread*, so it still read `true` for milliseconds after `stop_recording()` returned.

| Event | From | To | Action |
|---|---|---|---|
| Pressed | `Idle` | `Armed{now}` | start, show HUD, then mute |
| Pressed | `Toggled` | `Stopping` | stop + transcribe |
| Pressed | `Armed`/`Stopping` | — | ignore |
| Released | `Armed`, held > 350 ms | `Stopping` | stop + transcribe |
| Released | `Armed`, held ≤ 350 ms | `Toggled` | keep recording |
| Released | anything else | — | ignore |

Do not reintroduce `is_recording()` as control flow. `Stopping` → `Idle` happens in `logic_helper`'s `DictationGuard` on `Drop`, so a panic cannot wedge dictation.

The HUD is shown **before** muting: `mute_system_audio` shells out to `osascript` twice, which used to delay the overlay by 100-300 ms. `osascript` and Whisper inference both run on `spawn_blocking`.

### Talking to the UI

The hotkey is handled entirely in Rust, so the frontend learns nothing unless told. `emit_state` sends `dictation-state` (`recording` / `transcribing` / `idle`) and `emit_error` sends `dictation-error`; `App.vue` listens for both. Anything that fails silently in the backend should go through `emit_error` or the user never sees it.

### Audio pipeline (`audio_recorder.rs`)

`AudioRecorder::new()` spawns a thread that owns the `cpal` stream and receives `Start`/`Stop` over an mpsc channel, because a `cpal::Stream` isn't `Send`. The input callback keeps channel 0 only, pushes 1024-sample chunks through a `rubato::SincFixedIn` resampler to 16 kHz, and appends to an `Arc<Mutex<VecDeque<f32>>>` capped at 5 minutes — oldest samples are dropped at capacity. `get_audio()` drains it.

### Transcription (`transcriber.rs`)

`WhisperContext` is built once in `setup()` and held in `AppState`; the language code is passed per call. Output is post-filtered for Whisper hallucinations. Match against the **whole normalized transcript**, never as substrings — an earlier version used `contains("Thank you")` and silently discarded any dictation containing that phrase. Unreadable segments are skipped, not propagated, so one invalid-UTF-8 segment cannot drop the whole transcript.

**The model filename is load-bearing.** `src-tauri/models/ggml-base.bin` holds the multilingual `ggml-base` weights, named by `WHISPER_MODEL_FILE` in `lib.rs`. whisper.cpp is built with `-DWHISPER_USE_COREML` and derives the Core ML encoder path from this filename by swapping the extension for `-encoder.mlmodelc`. A `.en` name picks up an English-only encoder whose dimensions match, so nothing errors — the transcript is simply garbage. That shipped for months; see `git log` for `fix: remove mismatched Core ML encoder`.

Core ML acceleration is currently off (no encoder is bundled). `-DWHISPER_COREML_ALLOW_FALLBACK` means whisper falls back to CPU, around 0.4 s for a 5-second clip.

### Config (`config.rs`)

Everything lives under a single `"config"` key in the `config.json` store as one `AppConfig` (`hotkey`, `auto_mute_enabled`, `language`). Each setter reads the existing config, rebuilds the whole struct field by field, and saves. **Adding a field means touching `AppConfig`, its `Default`, the migration in `init_store`, and every `set_*` command** — miss one and that setter silently resets the new field.

### Auto-mute (`audio_control.rs`)

Volume is read and set by shelling out to `osascript`; the pre-mute level is stashed in `AppState.previous_volume` and restored **exactly** — an earlier version floored the restore at 30, which turned a deliberately-quiet Mac loud after every dictation. `cleanup_on_exit` on `RunEvent::Exit` restores volume and QUITs the helper, so quitting mid-recording no longer leaves the Mac silent and the helper orphaned.

### Frontend

One `App.vue` drives everything through `invoke` and the two events above. Every Rust command must be listed in the `invoke_handler!` block at the bottom of `lib.rs`. The `save_test_audio` / `transcribe_test_audio` / `inject_test_text` trio backs the "Debug Tools" panel and round-trips through `~/Desktop/test_audio.wav`; `save_test_audio` snapshots rather than drains, because draining destroyed a recording in progress.

No webfonts — the app must render offline. `csp` is set and `withGlobalTauri` is off, so `window.__TAURI__` does not exist; import from `@tauri-apps/api`.

## Conventions

- Design and implementation plans go in `docs/plans/YYYY-MM-DD-<topic>.md` before non-trivial work.
- Conventional commits (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`); feature branch → PR into `main`.
- `*.bin` is git-lfs tracked.
- Rust logs via `println!`/`eprintln!` to stdout; helper logs land in Console.app (filter `GhostWriterOverlay`).
- Root-level `test-*.sh` and `check_*.m` are ad-hoc window-level debugging aids, not a test suite.
- Requires Microphone and Accessibility permissions; usage strings live in `src-tauri/Info.plist`.
