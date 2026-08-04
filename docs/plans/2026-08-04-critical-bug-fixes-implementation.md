# Critical Bug Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix six defects that silently lose dictation, strand the recording HUD on screen, change the user's system volume behind their back, or leave the app with no working hotkey.

**Architecture:** Small independent patches. No structural change. The only new shape is a `Drop` guard in `logic_helper.rs` so every exit path from the transcription task clears the HUD, and an app-level exit hook in `lib.rs` for cleanup that currently never runs.

**Tech Stack:** Rust, Tauri 2, Vue 3

**Branch:** `fix/critical-bugs`

**Findings covered:** #1, #2, #3, #4, #5, #6 (plus #15 partially, and two `unwrap()` panics)

---

## Task 1: Stop the hallucination filter from deleting real dictation

Finding #1. `cleaned_text.contains("Thank you")` blanks the **entire** transcript. Dictating *"Thank you for the update, I'll review it tonight"* types nothing and reports no error.

**Files:**
- Modify: `src-tauri/src/transcriber.rs:64-112`

**Step 1: Replace the sanitize block with pure, testable helpers**

Delete everything from `// Sanitize text - remove music notes...` (line 64) through `if cleaned_text.len() < 2 { ... }` (line 110), and replace the tail of `transcribe()` with:

```rust
        let cleaned = sanitize(&text);
        if is_hallucination(&cleaned) {
            return Ok(String::new());
        }
        Ok(cleaned)
    }
}

const MUSIC_GLYPHS: &[char] = &['♪', '♫', '♬', '♭', '♮', '♯'];

/// Phrases Whisper emits as an ENTIRE transcript when fed silence or noise.
/// Compared against the whole normalized transcript — never as substrings,
/// because "thank you" is ordinary dictation.
const FULL_TEXT_HALLUCINATIONS: &[&str] = &[
    "you",
    "thank you",
    "thanks for watching",
    "please subscribe",
];

fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !MUSIC_GLYPHS.contains(c))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Trim, drop trailing sentence punctuation, lowercase.
fn normalize_for_match(text: &str) -> String {
    text.trim()
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?') || c.is_whitespace())
        .trim()
        .to_lowercase()
}

fn is_hallucination(text: &str) -> bool {
    let normalized = normalize_for_match(text);
    if normalized.is_empty() {
        return true;
    }
    // Whisper's subtitle-credit artifact; this domain never appears in dictation.
    if normalized.contains("amara.org") {
        return true;
    }
    // Nothing to type if there isn't a single alphanumeric character.
    if !normalized.chars().any(|c| c.is_alphanumeric()) {
        return true;
    }
    FULL_TEXT_HALLUCINATIONS.contains(&normalized.as_str())
}
```

Note the deliberate removal of the old `cleaned_text.len() < 2` rule: `len()` counts **bytes**, so it discarded legitimate one-character dictation. The `is_alphanumeric` rule already rejects the junk it was aiming at.

**Step 2: Add unit tests at the bottom of the file**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_dictation_that_contains_a_hallucination_phrase() {
        assert!(!is_hallucination(
            "Thank you for the update, I'll review it tonight"
        ));
    }

    #[test]
    fn drops_bare_thank_you() {
        assert!(is_hallucination("Thank you."));
        assert!(is_hallucination("thank you"));
    }

    #[test]
    fn drops_amara_subtitle_credit() {
        assert!(is_hallucination("Subtitles by the Amara.org community"));
    }

    #[test]
    fn drops_empty_and_punctuation_only() {
        assert!(is_hallucination("   "));
        assert!(is_hallucination("..."));
    }

    #[test]
    fn keeps_short_real_words() {
        assert!(!is_hallucination("no"));
        assert!(!is_hallucination("A"));
    }

    #[test]
    fn strips_music_glyphs() {
        assert_eq!(sanitize("♪ hello ♪"), "hello");
    }
}
```

**Step 3: Run the tests**

Run: `cd src-tauri && cargo test transcriber --lib -- --nocapture`
Expected: 6 tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/transcriber.rs
git commit -m "fix: only drop whole-transcript hallucinations, not substrings"
```

---

## Task 2: Guarantee the HUD is hidden on every exit path

Finding #2. Five `return`s in `stop_and_transcribe_logic` sit before the only `hide()` call. Repro: double-tap the hotkey quickly — the buffer is empty, the task returns early, and the overlay stays on screen until the app is quit.

**Files:**
- Modify: `src-tauri/src/logic_helper.rs`

**Step 1: Add a Drop guard above `stop_and_transcribe_logic`**

```rust
/// Hides the recording HUD when dropped, so every exit path from the
/// transcription task clears the overlay — including early returns and panics.
struct HudGuard {
    app: tauri::AppHandle,
}

impl Drop for HudGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        {
            let state = self.app.state::<AppState>();
            match state.overlay.lock() {
                Ok(overlay) => {
                    if let Some(helper) = overlay.as_ref() {
                        if let Err(e) = helper.hide() {
                            eprintln!("Failed to hide overlay: {}", e);
                        }
                    }
                }
                Err(e) => eprintln!("Overlay lock poisoned while hiding HUD: {}", e),
            }
        }

        #[cfg(not(target_os = "macos"))]
        if let Some(hud) = self.app.get_webview_window("hud") {
            let _ = hud.hide();
        }
    }
}
```

**Step 2: Create the guard first thing in the spawned task**

```rust
pub fn stop_and_transcribe_logic(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _hud = HudGuard { app: app.clone() };
        let state = app.state::<AppState>();
        // ... unchanged ...
```

**Step 3: Delete the now-redundant tail**

Remove the `// HIDE HUD using overlay helper` block (both the `#[cfg(target_os = "macos")]` and `#[cfg(not(...))]` arms) at the end of the task — the guard owns this now.

**Step 4: Delete the dead unmute block**

Both callers in `lib.rs` already call `unmute_if_needed` before invoking this function, so `previous_volume` is always `None` here. Remove the `// Unmute system audio` block (`logic_helper.rs:65-85`).

**Step 5: Verify**

Run: `cd src-tauri && cargo build --lib`
Expected: compiles clean.

**Step 6: Commit**

```bash
git add src-tauri/src/logic_helper.rs
git commit -m "fix: always hide HUD when transcription task exits"
```

---

## Task 3: Restore the exact previous volume, and restore it on quit

Finding #3. Two separate defects: dictating with the Mac near-silent leaves it at **30%**, and quitting from the tray mid-recording leaves it at **0** forever.

**Files:**
- Modify: `src-tauri/src/audio_control.rs:53-64`
- Modify: `src-tauri/src/overlay_helper.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Restore what was actually read**

```rust
/// Restores the volume captured by `mute_system_audio`.
pub fn unmute_system_audio(previous_volume: u32) -> Result<(), String> {
    set_system_volume(previous_volume)
}
```

The old "floor at 30 if below 20" rule turned a deliberately-quiet Mac loud after every dictation.

**Step 2: Add a fire-and-forget `quit` to OverlayHelper**

The helper terminates as soon as it receives QUIT, so it may die before writing its `OK` ack. Do not route this through `send_command`, which requires the ack:

```rust
    /// Asks the helper to exit. Fire-and-forget: the helper terminates
    /// immediately, so it may die before acking.
    pub fn quit(&self) {
        if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
            let _ = writeln!(stream, "QUIT");
            let _ = stream.flush();
        }
    }
```

**Step 3: Add an exit hook in `lib.rs`**

Add next to `unmute_if_needed`:

```rust
/// Runs once as the app tears down. Without this, quitting mid-recording
/// leaves the system muted and the overlay helper running.
fn cleanup_on_exit(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();

    let previous_volume = state.previous_volume.lock().ok().and_then(|v| *v);
    if let Some(vol) = previous_volume {
        if let Err(e) = unmute_system_audio(vol) {
            eprintln!("[exit] Failed to restore volume: {}", e);
        }
    }

    #[cfg(target_os = "macos")]
    if let Ok(overlay) = state.overlay.lock() {
        if let Some(helper) = overlay.as_ref() {
            helper.quit();
        }
    }
}
```

**Step 4: Switch from `.run(context)` to `.build(context).run(closure)`**

Replace the tail of `run()`:

```rust
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                cleanup_on_exit(app_handle);
            }
        });
```

**Step 5: Verify the event actually fires on both quit paths**

`RunEvent::Exit` should fire for both tray → Quit (`app.exit(0)`) and Cmd+Q, but confirm rather than assume. Add a temporary `println!("[exit] cleanup running")` at the top of `cleanup_on_exit`, then:

- `npm run tauri dev`, start recording (audio mutes), quit from the tray → the line prints and system volume returns.
- Repeat with Cmd+Q.

If `Exit` does not fire on one path, match on `RunEvent::ExitRequested { .. }` as well. Remove the temporary println once confirmed.

**Step 6: Commit**

```bash
git add src-tauri/src/audio_control.rs src-tauri/src/overlay_helper.rs src-tauri/src/lib.rs
git commit -m "fix: restore exact volume and clean up helper on exit"
```

---

## Task 4: Never leave the user without a hotkey

Finding #4. `save_hotkey` calls `unregister_all()` **before** parsing the new shortcut, so a parse failure strands the app with no global shortcut while `config.json` still holds the old one.

**Files:**
- Modify: `src-tauri/src/lib.rs:48-170`

**Step 1: Reorder `save_hotkey` — validate, then swap**

```rust
#[tauri::command]
fn save_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;

    let existing: AppConfig = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // 1. Validate BEFORE touching the live registration.
    let shortcut = hotkey
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("Invalid hotkey '{}': {}", hotkey, e))?;

    // 2. Swap, restoring the previous binding if registration fails.
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    if let Err(e) = app.global_shortcut().register(shortcut) {
        if let Ok(previous) = existing
            .hotkey
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
        {
            let _ = app.global_shortcut().register(previous);
        }
        return Err(format!("Failed to register '{}': {}", hotkey, e));
    }

    // 3. Persist.
    let config = AppConfig {
        hotkey,
        ..existing
    };
    store.set("config".to_string(), json!(config));
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}
```

**Step 2: Apply struct-update syntax to the other two setters**

`set_auto_mute_enabled` and `set_language` currently rebuild every field by hand with `.unwrap_or(...)` defaults — the footgun documented in `CLAUDE.md`. Replace both bodies with the same shape:

```rust
    let existing: AppConfig = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let config = AppConfig { language, ..existing };   // or `auto_mute_enabled: enabled, ..existing`
```

Adding a field to `AppConfig` can no longer silently reset it from one of these paths.

**Step 3: Add a regression test for the strings the UI can emit**

The recorder builds shortcut strings client-side, so pin the accepted grammar. Add to `lib.rs`:

```rust
#[cfg(test)]
mod hotkey_tests {
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn strings_the_ui_can_emit_all_parse() {
        for s in [
            "Cmd+Option+Space", // the shipped default
            "Command+Alt+Space",
            "Command+Shift+KeyA",
            "Control+Digit1",
            "Command+ArrowUp",
            "F5",
        ] {
            assert!(s.parse::<Shortcut>().is_ok(), "{s} should parse");
        }
    }
}
```

Run it **before** writing Task 5. If a form fails, that dictates the mapping Task 5 must produce — adjust there, not here.

**Step 4: Verify**

Run: `cd src-tauri && cargo test --lib`

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: validate hotkey before unregistering the old one"
```

---

## Task 5: Fix hotkey capture in the UI

Findings #4 (source) and #5. Two defects in one component: the box never displays the saved hotkey, and it captures `e.key`, which on macOS is the **Option-modified character** (Option+A → `å` → `"Command+Alt+Å"` → parse failure → Task 4's dead-hotkey path).

**Files:**
- Modify: `src/components/HotkeyRecorder.vue`

**Step 1: Watch the prop**

`App.vue` loads the hotkey in `onMounted`, after this component's `setup()` has already snapshotted the prop. Import `watch` and add:

```js
watch(
  () => props.initialHotkey,
  (value) => {
    currentHotkey.value = value;
    if (!isRecording.value) displayHotkey.value = value;
  }
);
```

**Step 2: Capture physical key codes**

Replace `formatKey` and delete the unused `keyMap` object:

```js
const MODIFIER_CODES = [
  "MetaLeft", "MetaRight",
  "ControlLeft", "ControlRight",
  "AltLeft", "AltRight",
  "ShiftLeft", "ShiftRight",
];

// e.code is the physical key ("KeyA", "Space", "Digit1", "ArrowUp"),
// unaffected by Option producing dead keys / accented characters on macOS.
const formatKey = (e) => {
  const keys = [];
  if (e.metaKey) keys.push("Command");
  if (e.ctrlKey) keys.push("Control");
  if (e.altKey) keys.push("Alt");
  if (e.shiftKey) keys.push("Shift");
  if (!MODIFIER_CODES.includes(e.code)) keys.push(e.code);
  return keys.join("+");
};
```

Update the two `hasNonModifier` / commit checks to test `e.code` against `MODIFIER_CODES` instead of `e.key` against `["Meta","Control","Alt","Shift"]`. Keep the `e.key === "Escape"` cancel check as-is (Escape is unambiguous), or switch it to `e.code === "Escape"` for consistency.

**Step 3: Surface save failures in the box**

`App.vue`'s `saveHotkey` logs the error but the recorder keeps showing the rejected combination. Revert the display on failure.

> **Corrected during execution.** The original draft of this step set
> `hotkey.value = ""` and then reassigned, to "force the watcher to fire".
> That is a no-op: Vue queues the watcher job once and compares the final
> value against the old value at flush time. On a failed save both are the
> last-known-good hotkey, so `hasChanged` is false and the callback never
> runs. Use an explicit exposed revert instead.

Child (`HotkeyRecorder.vue`):

```js
const revertDisplay = () => {
  isRecording.value = false;
  currentHotkey.value = props.initialHotkey;
  displayHotkey.value = props.initialHotkey;
};

defineExpose({ revertDisplay });
```

Parent (`App.vue`), with `ref="hotkeyRecorder"` on the component:

```js
  } catch (e) {
    log("Error saving hotkey: " + e);
    hotkeyRecorder.value?.revertDisplay();
  }
```

**Step 4: Verify manually**

`npm run tauri dev`:
- The box shows the configured hotkey on launch (previously always blank).
- Record `Cmd+Option+A` → saves and works (previously produced `Å` and failed).
- Record something the backend rejects → the box snaps back and the old hotkey still fires.

**Step 5: Commit**

```bash
git add src/components/HotkeyRecorder.vue src/App.vue
git commit -m "fix: display saved hotkey and capture physical key codes"
```

---

## Task 6: A bad config must not brick startup

Finding #6. `serde_json::from_value(...)?` and `.parse::<Shortcut>()?` inside `setup()` propagate to `.expect(...)` — one malformed value in `config.json` panics the app on **every** launch until the file is deleted by hand.

**Files:**
- Modify: `src-tauri/src/lib.rs:515-523`, `:402`, `:543`

**Step 1: Fall back instead of propagating**

```rust
            // Register hotkey from config. Never propagate: a malformed config
            // must not make the app unlaunchable.
            let store = app.store("config.json")?;
            let config: AppConfig = store
                .get("config")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();

            let shortcut = config
                .hotkey
                .parse::<tauri_plugin_global_shortcut::Shortcut>()
                .or_else(|e| {
                    eprintln!(
                        "[setup] Invalid hotkey '{}' in config ({}), falling back to default",
                        config.hotkey, e
                    );
                    AppConfig::default()
                        .hotkey
                        .parse::<tauri_plugin_global_shortcut::Shortcut>()
                });

            match shortcut {
                Ok(shortcut) => {
                    if let Err(e) = app.global_shortcut().register(shortcut) {
                        eprintln!("[setup] Failed to register hotkey: {}", e);
                    }
                }
                Err(e) => eprintln!("[setup] No usable hotkey: {}", e),
            }
```

**Step 2: Remove two panics in the same file**

- `:402` — `state.press_time.lock().unwrap()` (the comment even says "safe unwrap or match"). Use `match`/`if let Ok`, returning early on poison.
- `:543` — `window.hide().unwrap()` in the window-close handler. Use `let _ = window.hide();`.

**Step 3: Verify the recovery path**

```bash
# find the store, corrupt it, confirm the app still launches
python3 -c "import json,pathlib;p=pathlib.Path.home()/'Library/Application Support/com.ghostwriter.app/config.json';d=json.loads(p.read_text());d['config']['hotkey']='Not+A+Real+Key';p.write_text(json.dumps(d))"
npm run tauri dev
```
Expected: app launches, logs the fallback, and `Cmd+Option+Space` works.

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: fall back to defaults instead of panicking on bad config"
```

---

## Task 7: Full verification and PR

**Step 1: Checks**

```bash
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test --lib
```

**Step 2: End-to-end smoke test**

`npm run tauri dev`, then confirm each fixed defect:

| Finding | Check |
|---------|-------|
| #1 | Dictate "Thank you for the update" → the full sentence is typed |
| #2 | Double-tap the hotkey fast → HUD disappears (no stranded overlay) |
| #3 | Set volume to 5, dictate → volume returns to 5, not 30 |
| #3 | Quit from the tray mid-recording → volume restored, helper process gone (`pgrep -f GhostWriterOverlayHelper` is empty) |
| #4 | Record a hotkey with Option held → saves and fires |
| #5 | Reopen settings → the box shows the configured hotkey |
| #6 | Corrupt `config.json` → app still launches |

**Step 3: Push and open the PR**

```bash
git push -u origin fix/critical-bugs
gh pr create --title "fix: six critical dictation and hotkey defects" --body "..."
```

PR body: summary, the table above as the test plan. No AI attribution footer.

---

## Dependencies

None added. All changes are within existing crates.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| ~~`RunEvent::Exit` may not fire for tray-quit~~ | **Resolved during execution.** tauri 2.10.3 `app.rs:531` documents `AppHandle::exit()` as triggering `ExitRequested` *and* `Exit`; `app.rs:1304-1307` invokes the run callback before `cleanup_before_exit()`, so `AppState` is still available. No runtime check needed. |
| `Shortcut` grammar may reject `e.code` names like `KeyA` | Task 4 Step 3 pins the grammar in a test that runs *before* Task 5 writes the mapping |
| Removing the `len() < 2` rule lets single stray characters through | The `is_alphanumeric` rule still rejects punctuation-only output; `suppress_blank` and `no_speech_thold` remain set in `transcribe()` |
| `HudGuard` hides the HUD even when the caller wanted it to persist | There is no such caller — `stop_and_transcribe_logic` is the only place the HUD is ever hidden |

## Notes

- Task 4 Step 3 is a **blocking prerequisite** for Task 5. Run it first; its result decides the mapping.
- Task 3 also resolves finding #15 (orphaned helper process) as a side effect, since the cleanup hook is the natural home for both.
- This plan deliberately leaves the toggle/hold state-machine race (#13) and the blocking-`osascript`-under-lock issue (#14) alone; both touch the same `lib.rs` region and belong in their own change.
