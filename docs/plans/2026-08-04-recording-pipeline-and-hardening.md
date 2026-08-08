# Recording Pipeline & Hardening Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining findings from the 2026-08-04 review: the recording pipeline is silent about failure, racy about state, blocks its own runtime, and never tells the UI anything. Then a hardening/hygiene pass.

**Architecture:** Replace the two independent spawned hotkey handlers and the async `is_recording` flag with one explicit state machine behind a single mutex. Blocking work (`osascript`, Whisper inference) moves to `spawn_blocking`. The audio thread reports start failures back over a channel instead of swallowing them.

**Tech Stack:** Rust, Tauri 2, Vue 3, Objective-C

**Branches:** `fix/recording-pipeline` (stacked on `fix/hud-positioning`), then `chore/hardening-and-hygiene`

**Findings covered:** #11, #13, #14, #16, #18, #20 then #17, #19, #21, #22

---

## Why the current design cannot be patched incrementally

Three defects share one root cause — recording state is inferred rather than owned:

- `is_recording` is flipped by the **audio thread** when it processes `Stop`, so the toggle-off path checks a flag that is still `true` for some milliseconds after `stop_recording()` returns. Two stop-and-transcribe tasks can be spawned for one keypress.
- `press_time` is only cleared after a long-press stop, so after any toggle-mode cycle it is stale and the Released handler takes the >350 ms branch on garbage.
- Pressed and Released are independent `spawn`ed tasks with no ordering guarantee.

A single `DictationState` mutex makes all three unrepresentable, so tasks 1-3 are one change, not three.

---

## Task 1: Audio thread reports start failures (#11)

**Files:** `src-tauri/src/audio_recorder.rs`

Every failure path is a silent `if let Ok`: no input device, unsupported config, resampler construction, stream build, `play()`. `start_recording()` returns `Ok` regardless, so the app mutes audio, shows the HUD, records nothing, and reports nothing.

**Step 1:** Change the command to carry a reply channel:

```rust
enum AudioCommand {
    Start(Sender<Result<(), String>>),
    Stop,
}
```

**Step 2:** Extract stream setup into `fn build_stream(...) -> Result<cpal::Stream, String>` so each failure becomes `?` with a real message. The thread sends the result back.

**Step 3:** `start_recording()` waits on the reply with `recv_timeout(Duration::from_secs(5))` and returns the real error.

**Step 4:** Verify with `cargo test --lib`; confirm the error text surfaces by temporarily returning `Err` from `build_stream`.

---

## Task 2: One state machine for the hotkey (#13)

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/logic_helper.rs`

**Step 1:** Replace `press_time: Mutex<Option<Instant>>` in `AppState` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DictationState {
    Idle,
    /// Recording, hotkey still held. `pressed_at` decides toggle vs hold.
    Armed { pressed_at: Instant },
    /// Recording, hotkey released inside the toggle threshold.
    Toggled,
    /// Stop requested, transcription in flight. Further presses ignored.
    Stopping,
}
```

**Step 2:** Transitions, each taken under one lock:

| Event | From | To | Action |
|---|---|---|---|
| Pressed | `Idle` | `Armed{now}` | start recording, show HUD, mute |
| Pressed | `Toggled` | `Stopping` | stop + transcribe |
| Pressed | `Armed` / `Stopping` | — | ignore (key repeat, or already stopping) |
| Released | `Armed{t}`, held > 350 ms | `Stopping` | stop + transcribe |
| Released | `Armed{t}`, held ≤ 350 ms | `Toggled` | keep recording |
| Released | anything else | — | ignore |

`is_recording()` is no longer consulted for control flow — that was the race.

**Step 3:** `Stopping` → `Idle` happens in the transcription task's guard, so a panic cannot wedge the app in `Stopping`.

**Step 4:** Out-of-order tasks are now safe: a Released that runs before its Press finds `Idle` and is ignored.

---

## Task 3: Stop blocking the async runtime (#14)

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/logic_helper.rs`

Two `osascript` subprocess spawns run between "recording started" and "HUD shown", while holding the recorder `MutexGuard`, on a tokio worker. That is the 100-300 ms HUD lag. Whisper inference likewise blocks a worker for seconds while holding the transcriber lock.

**Step 1:** Show the HUD **before** muting — the visual should not wait on two subprocesses.

**Step 2:** Wrap both `osascript` call sites in `tauri::async_runtime::spawn_blocking` (verified present at `tauri-2.10.3/src/async_runtime.rs:278`).

**Step 3:** Run Whisper inference inside `spawn_blocking`, re-acquiring state from a cloned `AppHandle` (`AppHandle` is `Clone + 'static`; `State<'_>` is not).

**Step 4:** Never hold the recorder lock across a subprocess or socket call.

---

## Task 4: Tell the UI what is happening (#16)

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/logic_helper.rs`, `src/App.vue`

`isRecording` and the Activity Log only update from the debug buttons, so the status dot is dead during real use.

**Step 1:** Emit on every state change: `app.emit("dictation-state", "recording" | "transcribing" | "idle")` (needs `use tauri::Emitter`).

**Step 2:** Emit `dictation-error` with a message when recording fails to start or transcription fails, so Task 1's diagnostics reach the user rather than stdout.

**Step 3:** `listen()` for both in `App.vue`, drive `isRecording` and `log()`.

**Step 4:** `core:default` already grants `core:event:default`, so no capability change.

---

## Task 5: Delete the dead cross-platform code (#18)

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/logic_helper.rs`, `src-tauri/Cargo.toml`, `src/App.vue`, `src/components/Hud.vue`, `README.md`

The `#[cfg(not(target_os = "macos"))]` branches call `Mouse::get_mouse_position()` with no import and target a `hud` window `tauri.conf.json` never declares — they cannot compile and never ran.

**Step 1:** Delete those branches; drop `mouse_position` from `Cargo.toml`.

**Step 2:** Delete `src/components/Hud.vue` and the label switch in `App.vue`.

**Step 3:** Correct the README's cross-platform claim to describe what porting would actually require.

---

## Task 6: Fix the debug commands (#20)

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/audio_recorder.rs`

**Step 1:** `transcribe_test_audio` loads a second 148 MB `WhisperContext`; reuse `state.transcriber`.

**Step 2:** `save_test_audio` calls `get_audio()`, which **drains** the shared buffer, so clicking it mid-session destroys the pending recording. Add a non-draining `snapshot_audio()` and use it here.

---

## Task 7: Verify and PR

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`, `npm run build`, plus the socket smoke test for the HUD path.

---

# Branch 2: `chore/hardening-and-hygiene`

## Task 8: Entitlements (#17)

`com.apple.security.app-sandbox = true` will break the app the moment it is signed: a sandboxed app cannot post synthetic events into other apps (enigo), spawn/`pkill` an external helper, bind `/tmp/…sock`, or write `~/Desktop`. `com.apple.security.accessibility` is not a real entitlement key. Set sandbox to false and document why.

## Task 9: Socket path (#19)

`/tmp/ghostwriter_overlay.sock` is world-accessible and unauthenticated. Move to `$TMPDIR` (per-user) in both `overlay_helper.rs` and `main.m`, keeping the two in sync.

## Task 10: Frontend hardening (#21)

Drop the runtime Google Fonts import from `App.vue` (an offline dictation app should not phone home), set a real CSP in `tauri.conf.json`, and remove `withGlobalTauri` — the frontend imports from `@tauri-apps/api`.

## Task 11: Hygiene (#22)

Window title `tauri-appghostwriter` → `GhostWriter`; `index.html` title; gitignore root `target/` and untrack the two rust-analyzer flycheck logs; untrack the committed `check_*` / `find_flags` / `test_collection` Mach-O binaries; drop unused JS deps; make `check_permissions` actually check or say plainly that it does not.

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| State machine changes recording feel | The transition table above preserves the documented toggle/hold semantics exactly; only the racy paths change |
| `spawn_blocking` + `State<'_>` lifetimes | Re-acquire state from a cloned `AppHandle` inside the closure |
| Deleting `Hud.vue` breaks a window that does exist | `tauri.conf.json` declares only `main`; verified before deleting |
| Moving the socket path desyncs Rust and Obj-C | Both sides changed in the same commit and verified over `nc` before the app is run |
| Untracking committed binaries loses them | They are ad-hoc debugging aids, remain in git history, and rebuild from the `.m` sources beside them |
