# HUD Positioning Refactor — Design & Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the recording HUD appear where it is supposed to — centered near the bottom of whichever display the user is working on — on the first recording of every session and on every display.

**Architecture:** Move all geometry into the overlay helper. The helper already runs on the main thread with direct `NSScreen` access, so it can compute its own position correctly; Rust should only say *"show yourself"*. This deletes the cross-process dimension handshake, the `screen_info` cache, and the objc2 dependency tree.

**Tech Stack:** Objective-C (Cocoa), Rust, Tauri 2

**Branch:** `fix/hud-positioning`

**Findings covered:** #7, #8, #9, #10 (plus the dead `_process` handle from #16)

**Sequencing:** Land `fix/critical-bugs` first — it adds `OverlayHelper::quit()` to a file this plan rewrites.

---

## Design

### What is wrong today

Four defects compound into "the HUD is never where you want it":

1. **Both centering implementations put the HUD near the top.** AppKit's y-origin is bottom-left, so `origin.y + size.height - overlayHeight - 100` lands **160pt below the top** of the visible frame. 100pt above the bottom is `origin.y + 100`. This is the symptom already recorded in `test-positioning.sh`.

2. **The first recording of every session uses hardcoded 1920×1080.** `show_centered_bottom()` reads the dimension cache *before* `send_command()` refreshes it. On the first call the cache is empty and `screen_info`'s `NSScreen` fallback needs a `MainThreadMarker` it can never obtain — the global-shortcut handler runs on a tokio worker — so it returns 1920/1080. On a 1440×900 panel that computes `y = 920`, above the top of the screen.

3. **The protocol carries size but not origin.** `DIMENSIONS w h` drops `visibleFrame.origin`, and Rust builds absolute coordinates as if the origin were `(0,0)`. On this machine the desktop union spans `-1871,-1080 → 2609,900` across three displays, so a non-zero or negative origin is the normal case, not the edge case.

4. **The HUD swallows clicks.** `ignoresMouseEvents = NO` creates a 220×60 dead zone that cannot be clicked through and cannot become key either — clicks land nowhere.

### Why the helper should own it

Every one of the above is a symptom of computing geometry in the wrong process. The helper has what Rust lacks: main-thread `NSScreen` access, the full `visibleFrame` including origin, and `mainScreen` tracking whichever display holds the active window. Passing numbers across a socket adds a cache, a staleness window, and a lossy wire format for no benefit.

After this change the protocol is `SHOW_CENTERED` / `HIDE` and Rust holds no screen state at all.

### What gets deleted

- `src-tauri/src/screen_info.rs` (and its two unit tests)
- `CACHED_DIMENSIONS`, `set_cached_dimensions`, `receive_dimensions`
- The `DIMENSIONS` write in `main.m`
- `objc2`, `objc2-app-kit`, `objc2-foundation` from `Cargo.toml`
- Unused `OverlayHelper::show()` and `OverlayHelper::set_window_level()`

The objc2 migration's goal — no deprecated `cocoa`/`objc` crates — is satisfied more completely by having no Objective-C interop in Rust at all. The helper's `SHOW x y` handler stays so `test-standalone.sh` keeps working.

### Test coverage note

Deleting `screen_info.rs` removes the crate's only unit tests. The `fix/critical-bugs` branch adds tests to `transcriber.rs` and `lib.rs` first, which is the other reason to land it before this one.

---

## Task 1: Fix the centering math in the helper

**Files:**
- Modify: `overlay-helper/HUDPanel.m:47-51, 126-148`

**Step 1: Add geometry constants below the imports**

```objc
static const CGFloat kHUDWidth = 220.0;
static const CGFloat kHUDHeight = 60.0;
static const CGFloat kHUDBottomMargin = 100.0;
```

**Step 2: Use them in the initializer**

```objc
    NSRect frame = NSMakeRect(0, 0, kHUDWidth, kHUDHeight);
```

**Step 3: Rewrite `centerNearBottom` with the corrected origin math**

```objc
- (void)centerNearBottom {
    // visibleFrame is in bottom-left origin coordinates and already excludes
    // the menu bar and Dock. On a multi-display setup origin is frequently
    // non-zero or negative, so it must be added, not assumed to be (0,0).
    NSRect visible = [[NSScreen mainScreen] visibleFrame];

    CGFloat x = visible.origin.x + (visible.size.width - kHUDWidth) / 2.0;
    CGFloat y = visible.origin.y + kHUDBottomMargin;

    NSLog(@"Centering HUD on screen %.0f,%.0f %.0fx%.0f -> %.0f,%.0f",
          visible.origin.x, visible.origin.y,
          visible.size.width, visible.size.height, x, y);

    [self setFrameOrigin:NSMakePoint(x, y)];
}
```

**Step 4: Add a `showCentered` entry point**

```objc
- (void)showCentered {
    [self centerNearBottom];
    [self orderFrontRegardless];
}
```

Declare it in `overlay-helper/HUDPanel.h` next to `showAtX:y:`.

**Step 5: Drop the misleading `makeKeyAndOrderFront:` in `showAtX:y:`**

`canBecomeKeyWindow` returns `NO`, so that call does nothing beyond what `orderFrontRegardless` already did. Remove the line.

**Step 6: Commit**

```bash
git add overlay-helper/HUDPanel.h overlay-helper/HUDPanel.m
git commit -m "fix: position HUD above the bottom edge using visibleFrame origin"
```

---

## Task 2: Add SHOW_CENTERED to the socket protocol

**Files:**
- Modify: `overlay-helper/main.m:124-175`

**Step 1: Handle SHOW_CENTERED *before* the SHOW branch**

This ordering is load-bearing. `processCommand` currently opens with `if ([command hasPrefix:@"SHOW"])`, which **also matches `SHOW_CENTERED`** — it would fall into the coordinate parser, find fewer than 3 components, and silently do nothing. Put the exact-match branch first:

```objc
    if ([command isEqualToString:@"SHOW_CENTERED"]) {
        NSLog(@"SocketServer: Showing HUD centered");
        dispatch_async(dispatch_get_main_queue(), ^{
            [self.hudPanel showCentered];
        });
    } else if ([command hasPrefix:@"SHOW"]) {
        // ... existing SHOW x y handler, kept for test-standalone.sh ...
```

**Step 2: Verify the helper standalone before touching Rust**

```bash
cd overlay-helper && make
./GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper &
printf 'SHOW_CENTERED' | nc -U /tmp/ghostwriter_overlay.sock
```
Expected: the HUD appears centered horizontally, 100pt above the bottom of the active display, and `OK` comes back. Then `printf 'HIDE' | nc -U ...` and `printf 'QUIT' | nc -U ...`.

**Step 3: Commit**

```bash
git add overlay-helper/main.m
git commit -m "feat: add SHOW_CENTERED command to overlay helper"
```

---

## Task 3: Remove the DIMENSIONS handshake

**Files:**
- Modify: `overlay-helper/main.m:100-112`

**Step 1: Delete the dimension write at the top of `handleClient`**

Remove the `NSRect screenFrame` / `dimCommand` block entirely. The client no longer reads it, and it was the reason every command paid for a screen query.

**Step 2: Re-verify with `nc`**

Repeat Task 2 Step 2. The socket should now return only `OK` per command.

**Step 3: Commit**

```bash
git add overlay-helper/main.m
git commit -m "refactor: drop DIMENSIONS handshake from overlay helper"
```

---

## Task 4: Simplify `overlay_helper.rs`

**Files:**
- Modify: `src-tauri/src/overlay_helper.rs`

**Step 1: Delete the dimension plumbing**

- Remove `use crate::screen_info;`
- Remove the entire first `impl OverlayHelper` block containing `receive_dimensions`

**Step 2: Reduce `show_centered_bottom` to a command**

```rust
    /// Shows the HUD centered near the bottom of the active display.
    /// Geometry lives in the helper, which has main-thread NSScreen access.
    pub fn show_centered_bottom(&self) -> Result<(), String> {
        self.send_command("SHOW_CENTERED")
    }
```

**Step 3: Delete dead methods**

`show(&self, x, y)` and `set_window_level(&self, level)` have no callers. Remove both.

**Step 4: Simplify `send_command`**

Drop the `receive_dimensions` call; the first line read from the socket is now the `OK` ack:

```rust
    fn send_command(&self, command: &str) -> Result<(), String> {
        let mut stream = UnixStream::connect(SOCKET_PATH)
            .map_err(|e| format!("Failed to connect to helper: {}", e))?;

        stream
            .write_all(command.as_bytes())
            .map_err(|e| format!("Failed to send command: {}", e))?;
        stream
            .flush()
            .map_err(|e| format!("Failed to flush command: {}", e))?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if response.trim() != "OK" {
            return Err(format!("Unexpected response: {}", response.trim()));
        }

        Ok(())
    }
```

**Step 5: Actually keep the child handle**

`launch_helper` returns a `Child` that `new()` discards, so `_process` is permanently `None` and the child is never reaped. Store it:

```rust
pub struct OverlayHelper {
    process: Mutex<Option<Child>>,
}
```

Capture it in the launch loop, pass it into `Self { process: Mutex::new(Some(child)) }`, and reap in `Drop`:

```rust
impl Drop for OverlayHelper {
    fn drop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            if let Some(mut child) = process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
}
```

The `quit()` added on `fix/critical-bugs` remains the graceful path; this is the backstop.

**Step 6: Commit**

```bash
git add src-tauri/src/overlay_helper.rs
git commit -m "refactor: let the helper own HUD geometry"
```

---

## Task 5: Delete `screen_info.rs` and the objc2 dependencies

**Files:**
- Delete: `src-tauri/src/screen_info.rs`
- Modify: `src-tauri/src/lib.rs:7`
- Modify: `src-tauri/Cargo.toml:35-37`

**Step 1: Remove the module**

```bash
git rm src-tauri/src/screen_info.rs
```
and delete `mod screen_info;` from `lib.rs`. Note it is currently declared **unconditionally**, so it also compiles objc2 code on non-macOS targets — removing it is a small step toward finding #18.

**Step 2: Drop the dependencies**

Remove from `Cargo.toml`:
```toml
objc2 = "0.6"
objc2-app-kit = "0.3"
objc2-foundation = "0.3"
```

**Step 3: Verify**

```bash
cd src-tauri
cargo build --lib
cargo clippy -- -D warnings
```
Expected: clean, with no unused-import or dead-code warnings from the removals.

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove screen_info module and objc2 dependencies"
```

---

## Task 6: Stop the HUD from swallowing clicks

**Files:**
- Modify: `overlay-helper/HUDPanel.m:70`

**Step 1: Make it click-through**

```objc
        // Passive indicator: never intercept clicks meant for the app below.
        self.ignoresMouseEvents = YES;
```

**Step 2: Commit**

```bash
git add overlay-helper/HUDPanel.m
git commit -m "fix: make the HUD click-through"
```

---

## Task 7: Rebuild, install, and verify on all three displays

**Step 1: Rebuild and install the helper**

```bash
cd overlay-helper && make install
```

**Step 2: Cold-start check — this is the regression that motivated the refactor**

```bash
pkill -f GhostWriterOverlayHelper
npm run tauri dev
```
Trigger the **first** recording of the session. It must appear centered near the bottom. Previously this first show used the 1920×1080 fallback and landed off-center near the top.

**Step 3: Multi-display check**

For each of the three displays: focus a window on it, trigger recording, confirm the HUD is centered near the bottom **of that display**.

**Step 4: Fullscreen / Space check**

Enter fullscreen in VS Code, trigger recording, confirm the HUD floats above it. Switch Spaces while it is visible and confirm the KVO reposition keeps it near the bottom — both paths now share `centerNearBottom`, so they can no longer disagree.

**Step 5: Click-through check**

With the HUD visible, click where it sits and confirm the click reaches the window underneath.

**Step 6: Commit any fixes and push**

```bash
git push -u origin fix/hud-positioning
gh pr create --title "fix: HUD positioning owned by the overlay helper" --body "..."
```

PR body: the design summary plus the checks above as the test plan. No AI attribution footer.

---

## Dependencies

Removes three: `objc2`, `objc2-app-kit`, `objc2-foundation`. Adds none.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| `hasPrefix:@"SHOW"` silently swallows `SHOW_CENTERED` | Task 2 Step 1 orders the exact-match branch first; Step 2 verifies over `nc` before Rust changes land |
| `mainScreen` may not be the display the user is typing on | `mainScreen` is defined as the screen holding the active window — this is the behavior we want, and Task 7 Step 3 verifies it per display |
| Deleting `screen_info.rs` removes the crate's only tests | `fix/critical-bugs` lands first and adds tests to `transcriber.rs` and `lib.rs` |
| Helper and app get out of sync (old helper bundled, new command sent) | `make install` in Task 7 Step 1; `OverlayHelper::new()` already kills any stale helper on startup |
| `ignoresMouseEvents = YES` might be needed later for an interactive HUD | Trivially reversible; the HUD has no controls today and cannot become key |

## Notes

- `test-standalone.sh` and `test-positioning.sh` still work: the helper keeps its `SHOW x y` handler. Consider updating `test-positioning.sh`'s expectations, which currently describe the bug.
- After this change Rust holds no screen state, so the `mouse_position` crate in `Cargo.toml` becomes the only remaining geometry dependency — and it is used solely by the stale non-macOS branch (finding #18). Removing it belongs with that cleanup.
