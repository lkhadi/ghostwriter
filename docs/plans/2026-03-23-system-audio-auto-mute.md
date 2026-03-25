# System Audio Auto-Mute Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Automatically mute macOS system audio when dictation recording starts and restore volume when transcription completes.

**Architecture:** Add a new `audio_control.rs` module using CoreAudio FFI to control system mute state. Store previous volume level in `AppState` for restoration. Integrate mute calls into the recording lifecycle in `lib.rs`.

**Tech Stack:** Rust, CoreAudio framework (macOS), Tauri, FFI bindings

---

## Prerequisites

- Read `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs` to understand the recording lifecycle
- Read `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/logic_helper.rs` to understand transcription completion flow
- Read `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/Cargo.toml` to check existing dependencies

---

### Task 1: Read Existing Code

**Files:**
- Read: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs`
- Read: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/logic_helper.rs`
- Read: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/state.rs` (if exists) or understand `AppState` in lib.rs
- Read: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/Cargo.toml`

**Step 1: Understand recording start flow**

In `lib.rs`, find where recording starts when the global shortcut is pressed. Look for the `ShortcutState::Pressed` handler around line 186.

**Step 2: Understand recording stop flow**

In `lib.rs`, find where recording stops on `ShortcutState::Released` (long press) and in the toggle-off logic. Also check `logic_helper.rs` for `stop_and_transcribe_logic` function.

**Step 3: Check dependencies**

In `Cargo.toml`, note any existing audio-related dependencies (cpal, coreaudio-sys, etc.).

**Step 4: Commit (no code changes, just observation)**

```bash
git add .
git commit -m "docs: analyze existing code for auto-mute feature"
```

---

### Task 2: Create CoreAudio FFI Bindings Module

**Files:**
- Create: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/audio_control.rs`

**Step 1: Create the audio control module with CoreAudio FFI**

```rust
use std::ffi::c_void;
use std::mem;
use std::os::raw::c_uint;
use std::ptr;

// CoreAudio constants
const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 0x676C6F62; // 'glob'
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = 0x646F7574; // 'dout'
const K_AUDIO_DEVICE_PROPERTY_MUTE: u32 = 0x6D757465; // 'mute'
const K_AUDIO_DEVICE_PROPERTY_VOLUME_SCALAR: u32 = 0x76766C6D; // 'vvol'

// CoreAudio types
#[repr(C)]
struct AudioObjectPropertyAddress {
    m_selector: u32,
    m_scope: u32,
    m_element: u32,
}

type AudioObjectID = u32;
type OSStatus = i32;
type AudioUnit = u32;

// CoreAudio functions
#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyData(
        in_object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;

    fn AudioObjectSetPropertyData(
        in_object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        in_data_size: u32,
        in_data: *const c_void,
    ) -> OSStatus;

    fn AudioObjectGetPropertyDataSize(
        in_object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> OSStatus;
}

/// Gets the default output device ID
fn get_default_output_device() -> Result<AudioObjectID, String> {
    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut device_id: AudioObjectID = 0;
    let mut size = mem::size_of::<AudioObjectID>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            &mut size,
            &mut device_id as *mut _ as *mut c_void,
        )
    };

    if status != 0 {
        return Err(format!("Failed to get default output device: {}", status));
    }

    if device_id == 0 {
        return Err("No default output device found".to_string());
    }

    Ok(device_id)
}

/// Gets the current mute state of a device (0 = unmuted, 1 = muted)
fn get_mute_state(device_id: AudioObjectID) -> Result<u32, String> {
    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_DEVICE_PROPERTY_MUTE,
        m_scope: 0x6F757470, // 'outp' - output scope
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut mute_state: u32 = 0;
    let mut size = mem::size_of::<u32>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            &mut size,
            &mut mute_state as *mut _ as *mut c_void,
        )
    };

    if status != 0 {
        return Err(format!("Failed to get mute state: {}", status));
    }

    Ok(mute_state)
}

/// Sets the mute state of a device (0 = unmute, 1 = mute)
fn set_mute_state(device_id: AudioObjectID, mute: u32) -> Result<(), String> {
    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_DEVICE_PROPERTY_MUTE,
        m_scope: 0x6F757470, // 'outp' - output scope
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let status = unsafe {
        AudioObjectSetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            mem::size_of::<u32>() as u32,
            &mute as *const _ as *const c_void,
        )
    };

    if status != 0 {
        return Err(format!("Failed to set mute state: {}", status));
    }

    Ok(())
}

/// Public function to mute system audio
/// Returns the previous mute state so it can be restored
pub fn mute_system_audio() -> Result<bool, String> {
    let device_id = get_default_output_device()?;
    let previous_state = get_mute_state(device_id)?;
    set_mute_state(device_id, 1)?; // 1 = muted
    Ok(previous_state == 1)
}

/// Public function to unmute system audio
pub fn unmute_system_audio() -> Result<(), String> {
    let device_id = get_default_output_device()?;
    set_mute_state(device_id, 0)?; // 0 = unmuted
    Ok(())
}
```

**Step 2: Add the module to lib.rs**

In `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs`, add the module declaration at the top with the other modules:

```rust
mod audio_control;
mod audio_recorder;
mod config;
mod injector;
mod logic_helper;
mod transcriber;
```

**Step 3: Test the module compiles**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo check
```

Expected: Success with no errors

**Step 4: Commit**

```bash
git add src-tauri/src/audio_control.rs src-tauri/src/lib.rs
git commit -m "feat: add CoreAudio FFI bindings for system mute control"
```

---

### Task 3: Update AppState to Store Mute State

**Files:**
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs:28-34`

**Step 1: Add fields to AppState struct**

Add a field to track whether we muted the system (so we only unmute if we muted it):

```rust
pub struct AppState {
    pub recorder: Mutex<AudioRecorder>,
    pub transcriber: Mutex<Option<Transcriber>>,
    pub press_time: Mutex<Option<Instant>>,
    pub system_was_muted: Mutex<bool>, // NEW: track if we muted the system
    #[cfg(target_os = "macos")]
    pub overlay: Mutex<Option<OverlayHelper>>,
}
```

**Step 2: Initialize the new field**

In the `setup` closure where `AppState` is created (around line 353), add the new field:

```rust
// Init State
app.manage(AppState {
    recorder: Mutex::new(AudioRecorder::new()),
    transcriber: Mutex::new(transcriber),
    press_time: Mutex::new(None),
    system_was_muted: Mutex::new(false), // NEW: initialize to false
    #[cfg(target_os = "macos")]
    overlay: Mutex::new(overlay_helper),
});
```

**Step 3: Test compilation**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo check
```

Expected: Success

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add system_was_muted tracking to AppState"
```

---

### Task 4: Integrate Mute into Recording Start

**Files:**
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs:200-233`

**Step 1: Add import for audio_control**

At the top of `lib.rs` with other imports:

```rust
use audio_control::{mute_system_audio, unmute_system_audio};
```

**Step 2: Add mute call when recording starts**

In the `ShortcutState::Pressed` handler, inside the `if !recorder.is_recording()` block (around line 203), add mute logic right after `start_recording()` succeeds:

```rust
if !recorder.is_recording() {
    // START RECORDING
    println!("Starting recording...");
    if let Ok(_) = recorder.start_recording() {
        // MUTE SYSTEM AUDIO
        match mute_system_audio() {
            Ok(was_already_muted) => {
                if let Ok(mut was_muted) = state.system_was_muted.lock() {
                    *was_muted = !was_already_muted; // Only mark as "we muted it" if it wasn't already muted
                }
                println!("System audio muted (was already muted: {})", was_already_muted);
            }
            Err(e) => {
                eprintln!("Failed to mute system audio: {}", e);
                // Continue recording anyway - don't block dictation
            }
        }

        if let Ok(mut pt) = state.press_time.lock() {
            *pt = Some(Instant::now());
        }

        // SHOW HUD centered near bottom of screen (macOS)
        #[cfg(target_os = "macos")]
        {
            // ... existing HUD code ...
        }
        // ... rest of existing code ...
    }
}
```

**Step 3: Test compilation**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo check
```

Expected: Success

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: mute system audio when recording starts"
```

---

### Task 5: Create Unmute Helper Function

**Files:**
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs`
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/logic_helper.rs`

**Step 1: Add unmute function to lib.rs**

Add this function after the imports but before `pub struct AppState`:

```rust
/// Unmutes system audio if we muted it during recording
fn maybe_unmute_system_audio(app_handle: &tauri::AppHandle) {
    let state = app_handle.state::<AppState>();

    let should_unmute = {
        if let Ok(was_muted) = state.system_was_muted.lock() {
            *was_muted
        } else {
            false
        }
    };

    if should_unmute {
        match unmute_system_audio() {
            Ok(_) => {
                println!("System audio unmuted");
                if let Ok(mut was_muted) = state.system_was_muted.lock() {
                    *was_muted = false;
                }
            }
            Err(e) => {
                eprintln!("Failed to unmute system audio: {}", e);
            }
        }
    }
}
```

**Step 2: Export the function for use in logic_helper**

Add this line after the function definition to make it available to other modules:

```rust
pub use crate::maybe_unmute_system_audio;
```

Actually, a better approach is to move the unmute logic to `logic_helper.rs` since that's where transcription completes. Let's instead export what we need.

**Alternative Step 2: Add the unmute logic to logic_helper.rs**

Read `logic_helper.rs` first to understand its structure, then add the unmute call at the end of `stop_and_transcribe_logic` before text injection.

First, add import at top of `logic_helper.rs`:
```rust
use crate::audio_control::unmute_system_audio;
use crate::AppState;
use tauri::Manager;
```

Then at the end of the transcription flow, before text injection, add:
```rust
// Unmute system audio
let state = app_handle.state::<AppState>();
let should_unmute = {
    if let Ok(was_muted) = state.system_was_muted.lock() {
        *was_muted
    } else {
        false
    }
};

if should_unmute {
    match unmute_system_audio() {
        Ok(_) => {
            println!("System audio unmuted");
            if let Ok(mut was_muted) = state.system_was_muted.lock() {
                *was_muted = false;
            }
        }
        Err(e) => {
            eprintln!("Failed to unmute system audio: {}", e);
        }
    }
}
```

**Step 3: Test compilation**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo check
```

Expected: Success

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/logic_helper.rs
git commit -m "feat: unmute system audio after transcription completes"
```

---

### Task 6: Add Unmute to Recording Stop Flows

**Files:**
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/lib.rs:234-284`

**Step 1: Add unmute to toggle-off path**

In the "Toggle Off" section (around line 238), after `recorder.stop_recording()` and before `stop_and_transcribe_logic`, add unmute:

```rust
} else {
    // ALREADY RECORDING - TOGGLE OFF
    println!("Toggle Off (Pressed again)");
    let _ = recorder.stop_recording();

    // Unmute system audio
    drop(recorder); // Release lock before unmute
    {
        let state = app_handle.state::<AppState>();
        let should_unmute = {
            if let Ok(was_muted) = state.system_was_muted.lock() {
                *was_muted
            } else {
                false
            }
        };

        if should_unmute {
            match unmute_system_audio() {
                Ok(_) => {
                    println!("System audio unmuted");
                    if let Ok(mut was_muted) = state.system_was_muted.lock() {
                        *was_muted = false;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to unmute system audio: {}", e);
                }
            }
        }
    }

    stop_and_transcribe_logic(app_handle.clone());
}
```

**Step 2: Add unmute to long-press stop path**

In the long-press section (around line 261-276), after `recorder.stop_recording()` and before `stop_and_transcribe_logic`, add the same unmute logic:

```rust
if duration > Duration::from_millis(350) {
    // LONG PRESS -> STOP RECORDING (Hold Mode)
    let mut recorder = match state.recorder.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if recorder.is_recording() {
        println!("Long press detected - Stopping.");
        let _ = recorder.stop_recording();
        drop(recorder);

        // Unmute system audio
        {
            let state = app_handle.state::<AppState>();
            let should_unmute = {
                if let Ok(was_muted) = state.system_was_muted.lock() {
                    *was_muted
                } else {
                    false
                }
            };

            if should_unmute {
                match unmute_system_audio() {
                    Ok(_) => {
                        println!("System audio unmuted");
                        if let Ok(mut was_muted) = state.system_was_muted.lock() {
                            *was_muted = false;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to unmute system audio: {}", e);
                    }
                }
            }
        }

        // Reset press time so we don't trigger again
        if let Ok(mut pt) = state.press_time.lock() {
            *pt = None;
        }
        stop_and_transcribe_logic(app_handle.clone());
    }
}
```

**Step 3: Refactor to avoid code duplication (optional but recommended)**

Create a helper function in lib.rs to handle unmute logic:

```rust
/// Helper function to unmute system audio if we muted it
fn unmute_if_needed(state: &State<AppState>) {
    let should_unmute = {
        if let Ok(was_muted) = state.system_was_muted.lock() {
            *was_muted
        } else {
            false
        }
    };

    if should_unmute {
        match unmute_system_audio() {
            Ok(_) => {
                println!("System audio unmuted");
                if let Ok(mut was_muted) = state.system_was_muted.lock() {
                    *was_muted = false;
                }
            }
            Err(e) => {
                eprintln!("Failed to unmute system audio: {}", e);
            }
        }
    }
}
```

Then call `unmute_if_needed(&state)` in each location instead of duplicating the code.

**Step 4: Test compilation**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo check
```

Expected: Success

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: ensure system audio unmutes in all recording stop paths"
```

---

### Task 7: Add Settings Toggle (Optional Enhancement)

**Files:**
- Read: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/config.rs`
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/src/config.rs`
- Modify: `/Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src/App.vue`

**Step 1: Add setting to config**

In `config.rs`, add to `AppConfig` struct:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub hotkey: String,
    pub auto_mute_enabled: bool, // NEW
}
```

Update default config initialization to include `auto_mute_enabled: true`.

**Step 2: Add UI toggle**

In `App.vue`, add a checkbox to enable/disable auto-mute.

**Step 3: Conditionally mute based on setting**

In `lib.rs`, wrap mute calls with a check for the setting:

```rust
// Only mute if auto-mute is enabled in settings
let store = app.store("config.json").ok();
let auto_mute = store
    .and_then(|s| s.get("config"))
    .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
    .map(|c| c.auto_mute_enabled)
    .unwrap_or(true); // Default to true

if auto_mute {
    match mute_system_audio() {
        // ... existing code ...
    }
}
```

**Step 4: Commit**

```bash
git add src-tauri/src/config.rs src/App.vue src-tauri/src/lib.rs
git commit -m "feat: add setting to toggle auto-mute feature"
```

---

### Task 8: Build and Test

**Files:**
- All modified files

**Step 1: Build the project**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri
cargo build --release
```

Expected: Successful build

**Step 2: Test the feature**

1. Start the app: `npm run tauri dev`
2. Play some music or video
3. Press the hotkey to start recording
4. Verify audio mutes
5. Speak some text
6. Release or press again to stop
7. Verify audio unmutes after transcription

**Step 3: Edge case testing**

- Test when system is already muted (should stay muted after)
- Test rapid start/stop presses
- Test long-press vs toggle modes

**Step 4: Final commit**

```bash
git add .
git commit -m "feat: complete system audio auto-mute implementation"
```

---

## Testing Checklist

- [ ] Audio mutes when recording starts
- [ ] Audio unmutes when recording stops (toggle mode)
- [ ] Audio unmutes when recording stops (long-press mode)
- [ ] If system was already muted before recording, it stays muted after
- [ ] If mute fails, recording continues (graceful degradation)
- [ ] Build succeeds without warnings

## Documentation References

- CoreAudio documentation: https://developer.apple.com/documentation/coreaudio
- Tauri state management: https://tauri.app/develop/state-management
- FFI in Rust: https://doc.rust-lang.org/nomicon/ffi.html
