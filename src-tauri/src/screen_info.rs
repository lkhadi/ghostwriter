//! Screen dimension queries using objc2 AppKit
//!
//! This module encapsulates all macOS-specific screen queries.
//! It replaces the deprecated cocoa/objc crates with objc2.

use objc2_app_kit::NSScreen;
use objc2_foundation::MainThreadMarker;
use std::sync::Mutex;

/// Cached screen dimensions received from the helper app.
/// The helper app queries NSScreen directly when it starts.
static CACHED_DIMENSIONS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

/// Set cached screen dimensions received from the helper app.
pub fn set_cached_dimensions(width: i32, height: i32) {
    if let Ok(mut cached) = CACHED_DIMENSIONS.lock() {
        *cached = Some((width, height));
    }
}

/// Get the screen width in points.
/// First checks cached dimensions from helper app, then falls back to NSScreen query.
pub fn get_screen_width() -> i32 {
    // First check cached dimensions from helper app
    if let Ok(cached) = CACHED_DIMENSIONS.lock() {
        if let Some((width, _)) = *cached {
            return width;
        }
    }

    // Fallback to NSScreen query
    let mtm = match MainThreadMarker::new() {
        Some(mtm) => mtm,
        None => return 1920,
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.width as i32)
        .unwrap_or(1920)
}

/// Get the screen height in points.
/// First checks cached dimensions from helper app, then falls back to NSScreen query.
pub fn get_screen_height() -> i32 {
    // First check cached dimensions from helper app
    if let Ok(cached) = CACHED_DIMENSIONS.lock() {
        if let Some((_, height)) = *cached {
            return height;
        }
    }

    // Fallback to NSScreen query
    let mtm = match MainThreadMarker::new() {
        Some(mtm) => mtm,
        None => return 1080,
    };

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame().size.height as i32)
        .unwrap_or(1080)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_dimensions_positive() {
        let width = get_screen_width();
        let height = get_screen_height();
        assert!(width > 0, "Screen width should be positive, got {}", width);
        assert!(
            height > 0,
            "Screen height should be positive, got {}",
            height
        );
    }

    #[test]
    fn test_width_greater_than_height() {
        let width = get_screen_width();
        let height = get_screen_height();
        // Most displays are wider than tall (landscape mode)
        assert!(
            width > height,
            "Most displays are landscape, got {}x{}",
            width,
            height
        );
    }
}
