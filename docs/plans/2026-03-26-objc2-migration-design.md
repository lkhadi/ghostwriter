# objc2 Migration Design

**Date:** 2026-03-26
**Status:** Approved
**Motivation:** Future-proof the codebase - cocoa/objc crates are deprecated and cause clippy warnings

## Overview

Replace deprecated `cocoa` and `objc` crates with `objc2` in `overlay_helper.rs` for screen dimension queries.

## Dependencies

**Replace in Cargo.toml:**
```toml
# REMOVE
cocoa = "0.26.1"
objc = "0.2.7"

# ADD
objc2 = "0.6"
objc2-app-kit = "0.3"
objc2-foundation = "0.3"
```

## New Module: screen_info.rs

**Path:** `src-tauri/src/screen_info.rs`

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
        None => return 1920, // Fallback
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.width as i32)
        .unwrap_or(1920)
}

/// Get the screen height in points from the main screen's visible frame.
pub fn get_screen_height() -> i32 {
    let mtm = match MainThreadMarker::new() {
        Some(mtm) => mtm,
        None => return 1080, // Fallback
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.height as i32)
        .unwrap_or(1080)
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_dimensions_positive() {
        let width = get_screen_width();
        let height = get_screen_height();
        assert!(width > 0, "Screen width should be positive");
        assert!(height > 0, "Screen height should be positive");
    }

    #[test]
    fn test_width_greater_than_height() {
        let width = get_screen_width();
        let height = get_screen_height();
        // Most displays are wider than tall (landscape mode)
        assert!(width > height, "Most displays are landscape");
    }
}
```

## Updated overlay_helper.rs

**Changes:**
1. Remove all `cocoa` and `objc` imports
2. Remove `get_screen_width()` and `get_screen_height()` functions
3. Add `mod screen_info;` declaration
4. Update call sites to use `screen_info::get_screen_width()` and `screen_info::get_screen_height()`
5. Remove `#![allow(unexpected_cfgs, deprecated)]` (no longer needed)

## Migration Table

| Old (cocoa/objc) | New (objc2) |
|-------------------|--------------|
| `msg_send![screen_class, mainScreen]` | `NSScreen::mainScreen(mtm)` |
| `NSRect` with deprecated fields | `NSRect.size.width` (idiomatic) |
| Manual class lookup via `objc::runtime::Class::get()` | Direct struct method |
| `#![allow(unexpected_cfgs, deprecated)]` | Not needed |

## Verification

1. `cargo check` passes without warnings
2. `cargo clippy -- -D warnings` passes without workarounds
3. `cargo test` passes (including new unit tests)
4. HUD overlay appears centered when recording starts
