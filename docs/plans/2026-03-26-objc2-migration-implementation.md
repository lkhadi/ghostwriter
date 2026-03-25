# objc2 Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace deprecated `cocoa` and `objc` crates with `objc2` in overlay_helper.rs for screen dimension queries.

**Architecture:** Extract screen dimension logic into a new `screen_info.rs` module using `objc2-app-kit` and `objc2-foundation` crates. This encapsulates all unsafe Objective-C interop in a single, testable module.

**Tech Stack:** Rust, objc2, objc2-app-kit, objc2-foundation, Tauri

---

## Task 1: Update Cargo.toml dependencies

**File:**
- Modify: `src-tauri/Cargo.toml:35-36`

**Step 1: Replace deprecated crates**

In Cargo.toml, replace:
```toml
cocoa = "0.26.1"
objc = "0.2.7"
```

With:
```toml
objc2 = "0.6"
objc2-app-kit = "0.3"
objc2-foundation = "0.3"
```

**Step 2: Verify dependency resolution**

Run: `cd src-tauri && cargo fetch`
Expected: Downloads new dependencies without errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: replace deprecated cocoa/objc with objc2"
```

---

## Task 2: Create screen_info.rs module

**File:**
- Create: `src-tauri/src/screen_info.rs`

**Step 1: Create the module with initial implementation**

```rust
//! Screen dimension queries using objc2 AppKit
//!
//! This module encapsulates all macOS-specific screen queries.
//! It replaces the deprecated cocoa/objc crates with objc2.

use objc2_app_kit::NSScreen;
use objc2_foundation::MainThreadMarker;

/// Get the screen width in points from the main screen's visible frame.
pub fn get_screen_width() -> i32 {
    let mtm = match MainThreadMarker::new() {
        Some(mtm) => mtm,
        None => return 1920, // Fallback for non-main-thread
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.width as i32)
        .unwrap_or(1920)
}

/// Get the screen height in points from the main screen's visible frame.
pub fn get_screen_height() -> i32 {
    let mtm = match MainThreadMarker::new() {
        Some(mtm) => mtm,
        None => return 1080, // Fallback for non-main-thread
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.height as i32)
        .unwrap_or(1080)
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build --lib 2>&1`
Expected: Compiles without errors (may have warnings about unused imports, that's OK)

**Step 3: Commit**

```bash
git add src-tauri/src/screen_info.rs
git commit -m "feat: add screen_info module with objc2"
```

---

## Task 3: Add unit tests to screen_info.rs

**File:**
- Modify: `src-tauri/src/screen_info.rs`

**Step 1: Add tests at the bottom of the file**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_dimensions_positive() {
        let width = get_screen_width();
        let height = get_screen_height();
        assert!(width > 0, "Screen width should be positive, got {}", width);
        assert!(height > 0, "Screen height should be positive, got {}", height);
    }

    #[test]
    fn test_width_greater_than_height() {
        let width = get_screen_width();
        let height = get_screen_height();
        // Most displays are wider than tall (landscape mode)
        assert!(width > height, "Most displays are landscape, got {}x{}", width, height);
    }
}
```

**Step 2: Run tests**

Run: `cd src-tauri && cargo test screen_info::tests --lib -- --nocapture 2>&1`
Expected: Tests pass showing actual screen dimensions

**Step 3: Commit**

```bash
git add src-tauri/src/screen_info.rs
git commit -m "test: add screen_info unit tests"
```

---

## Task 4: Update overlay_helper.rs

**File:**
- Modify: `src-tauri/src/overlay_helper.rs`

**Step 1: Replace imports**

Remove:
```rust
#![allow(unexpected_cfgs, deprecated)]

use cocoa::base::id;
use cocoa::foundation::NSRect;
use objc::msg_send;
use objc::{sel, sel_impl};
```

Add:
```rust
mod screen_info;
```

**Step 2: Remove old get_screen_width and get_screen_height functions**

Remove the entire `get_screen_width()` and `get_screen_height()` functions (lines 139-173 approximately).

**Step 3: Update call sites**

In `show_centered_bottom()`, change:
```rust
let screen_width = Self::get_screen_width();
let screen_height = Self::get_screen_height();
```

To:
```rust
let screen_width = screen_info::get_screen_width();
let screen_height = screen_info::get_screen_height();
```

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo build --lib 2>&1`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src-tauri/src/overlay_helper.rs
git commit -m "refactor: use screen_info module instead of cocoa/objc"
```

---

## Task 5: Full verification

**Step 1: Run clippy without workarounds**

Run: `cd src-tauri && cargo clippy -- -D warnings 2>&1`
Expected: Passes without `RUSTFLAGS="-A unexpected-cfgs"` workarounds

**Step 2: Run all tests**

Run: `cargo test --lib 2>&1`
Expected: All tests pass

**Step 3: Format check**

Run: `cargo fmt --check 2>&1`
Expected: Passes

**Step 4: Commit final verification**

```bash
git add -A && git commit -m "chore: verify objc2 migration - clippy passes without workarounds"
```

---

## Task 6: Push and create PR

**Step 1: Push branch**

```bash
git push origin objc2-migration
```

**Step 2: Create PR (if gh CLI available)**

Or provide the link:
```
https://github.com/lkhadi/ghostwriter/pull/new/objc2-migration
```

---

## Dependencies

- Tauri 2.x
- objc2 = "0.6"
- objc2-app-kit = "0.3"
- objc2-foundation = "0.3"

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| objc2 API differences from cocoa | Use MainThreadMarker for thread safety; use idiomatic objc2 patterns from docs |
| NSScreen returns Option on some systems | Use `.unwrap_or()` fallback to 1920x1080 |

## Notes

- The `show_centered_bottom()` function positions the HUD 100px from bottom - this behavior is preserved
- Fallback dimensions (1920x1080) are used when:
  - Called from non-main thread
  - NSScreen::mainScreen returns None
