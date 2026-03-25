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