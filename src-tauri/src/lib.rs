// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod audio_control;
mod audio_recorder;
mod config;
mod injector;
mod logic_helper;
mod transcriber;

mod overlay_helper;

use audio_control::{mute_system_audio, unmute_system_audio};
use audio_recorder::AudioRecorder;
use config::{init_store, AppConfig};
use injector::Injector;
use logic_helper::stop_and_transcribe_logic;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_store::StoreExt;
use transcriber::Transcriber;

use overlay_helper::OverlayHelper;

/// A press held longer than this stops recording on release (hold mode);
/// a shorter press leaves recording running until the next press (toggle mode).
const HOLD_THRESHOLD: Duration = Duration::from_millis(350);

/// Where dictation is in its lifecycle.
///
/// This replaces two independent spawned handlers that inferred state from
/// `press_time` plus `AudioRecorder::is_recording()`. That flag is flipped
/// asynchronously by the audio thread, so it still read `true` for some
/// milliseconds after `stop_recording()` returned — long enough for a press
/// and its release to both spawn a stop-and-transcribe. Owning the state
/// behind one mutex makes that unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DictationState {
    Idle,
    /// Recording, hotkey still held. `pressed_at` decides toggle vs hold.
    Armed {
        pressed_at: Instant,
    },
    /// Recording, hotkey released inside the threshold, so recording
    /// continues until the next press.
    Toggled,
    /// Stop requested and transcription in flight; further presses ignored.
    Stopping,
}

/// What the UI is told. Kept deliberately small — the frontend only needs to
/// distinguish "capturing", "working" and "done".
fn emit_state(app: &tauri::AppHandle, state: &str) {
    if let Err(e) = app.emit("dictation-state", state) {
        eprintln!("Failed to emit dictation-state: {}", e);
    }
}

fn emit_error(app: &tauri::AppHandle, message: &str) {
    eprintln!("[dictation] {}", message);
    if let Err(e) = app.emit("dictation-error", message) {
        eprintln!("Failed to emit dictation-error: {}", e);
    }
}

pub struct AppState {
    pub recorder: Mutex<AudioRecorder>,
    pub transcriber: Mutex<Option<Transcriber>>,
    pub dictation: Mutex<DictationState>,
    /// Stores the previous volume before muting (0-100), None if not muted by us
    pub previous_volume: Mutex<Option<u32>>,
    pub overlay: Mutex<Option<OverlayHelper>>,
}

#[tauri::command]
fn get_hotkey(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let config = store.get("config").ok_or("Config not found")?;
    let config: AppConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    Ok(config.hotkey)
}

/// Reads the stored config, falling back to defaults if it is absent or
/// unparseable. Callers rebuild with `..existing` so adding a field to
/// `AppConfig` can never silently reset it from one of the setters.
fn load_config(store: &std::sync::Arc<tauri_plugin_store::Store<tauri::Wry>>) -> AppConfig {
    store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn save_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    println!("[save_hotkey] Called with: {}", hotkey);

    let store = app.store("config.json").map_err(|e| {
        eprintln!("[save_hotkey] Store error: {}", e);
        e.to_string()
    })?;
    let existing = load_config(&store);

    // 1. Validate BEFORE touching the live registration, so a bad string
    //    can never leave the user with no working hotkey.
    let shortcut = hotkey
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| {
            eprintln!("[save_hotkey] Parse error: {}", e);
            format!("Invalid hotkey '{}': {}", hotkey, e)
        })?;

    // 2. Swap, restoring the previous binding if registration fails.
    app.global_shortcut().unregister_all().map_err(|e| {
        eprintln!("[save_hotkey] Unregister error: {}", e);
        e.to_string()
    })?;

    if let Err(e) = app.global_shortcut().register(shortcut) {
        eprintln!("[save_hotkey] Register error: {}", e);
        if let Ok(previous) = existing
            .hotkey
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
        {
            let _ = app.global_shortcut().register(previous);
        }
        return Err(format!("Failed to register '{}': {}", hotkey, e));
    }

    // 3. Persist.
    let config = AppConfig { hotkey, ..existing };
    store.set("config".to_string(), json!(config));
    store.save().map_err(|e| {
        eprintln!("[save_hotkey] Save error: {}", e);
        e.to_string()
    })?;

    println!("[save_hotkey] Success!");
    Ok(())
}

#[tauri::command]
fn get_auto_mute_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let config = store.get("config").ok_or("Config not found")?;
    let config: AppConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    Ok(config.auto_mute_enabled)
}

#[tauri::command]
fn set_auto_mute_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let existing = load_config(&store);

    let config = AppConfig {
        auto_mute_enabled: enabled,
        ..existing
    };
    store.set("config".to_string(), json!(config));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_language(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let config = store.get("config").ok_or("Config not found")?;
    let config: AppConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    Ok(config.language)
}

#[tauri::command]
fn set_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let existing = load_config(&store);

    let config = AppConfig {
        language,
        ..existing
    };
    store.set("config".to_string(), json!(config));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn check_permissions() -> String {
    "Permissions check initiated. Please ensure Microphone and Accessibility are granted in System Settings.".to_string()
}

#[tauri::command]
fn start_recording(state: State<AppState>) -> Result<String, String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.start_recording().map_err(|e| e.to_string())?;
    Ok("Recording started".to_string())
}

#[tauri::command]
fn stop_recording(state: State<AppState>) -> Result<String, String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.stop_recording().map_err(|e| e.to_string())?;
    Ok("Recording stopped".to_string())
}

#[tauri::command]
fn save_test_audio(state: State<AppState>) -> Result<String, String> {
    let recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    // Snapshot, not drain: this used to consume the buffer, so pressing
    // "Save WAV" mid-session destroyed the recording in progress.
    let audio_data = recorder.snapshot_audio();

    // Save to desktop
    let desktop_path = dirs::desktop_dir().ok_or("Could not find desktop")?;
    let file_path = desktop_path.join("test_audio.wav");

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(&file_path, spec).map_err(|e| e.to_string())?;
    for sample in audio_data {
        writer.write_sample(sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    Ok(format!("Saved to {:?}", file_path))
}

#[tauri::command]
fn transcribe_test_audio(app: tauri::AppHandle, state: State<AppState>) -> Result<String, String> {
    // Read test_audio.wav from Desktop
    let desktop_path = dirs::desktop_dir().ok_or("Could not find desktop")?;
    let file_path = desktop_path.join("test_audio.wav");

    if !file_path.exists() {
        return Err(
            "test_audio.wav not found on Desktop. Please record and save first.".to_string(),
        );
    }

    let mut reader = hound::WavReader::open(&file_path).map_err(|e| e.to_string())?;
    // We saved as float 32, so read as float 32
    let samples: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect();

    // Get language from config
    let language = {
        let store = app.store("config.json").ok();
        store
            .and_then(|s| s.get("config"))
            .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
            .map(|c| c.language)
            .unwrap_or_else(|| "en".to_string())
    };

    // Reuse the context loaded at startup rather than paying to load the
    // 148 MB model a second time.
    let transcriber = state.transcriber.lock().map_err(|e| e.to_string())?;
    let transcriber = transcriber
        .as_ref()
        .ok_or("Whisper model was not loaded at startup")?;

    transcriber
        .transcribe(&samples, &language)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn inject_test_text(text: String) -> Result<String, String> {
    let mut injector = Injector::new().map_err(|e| e.to_string())?;
    injector.type_text(&text).map_err(|e| e.to_string())?;
    Ok("Text injected".to_string())
}

/// Restores system audio if we muted it.
///
/// `osascript` is a subprocess, so this runs on the blocking pool rather than
/// stalling an async worker mid-dictation.
fn unmute_if_needed(app: tauri::AppHandle) {
    let previous_vol = {
        let state = app.state::<AppState>();
        let guard = state.previous_volume.lock();
        match guard {
            Ok(prev) => *prev,
            _ => None,
        }
    };

    let Some(vol) = previous_vol else {
        return;
    };

    tauri::async_runtime::spawn_blocking(move || match unmute_system_audio(vol) {
        Ok(_) => {
            println!("System audio restored to volume: {}", vol);
            let state = app.state::<AppState>();
            let previous = state.previous_volume.lock();
            if let Ok(mut prev) = previous {
                *prev = None;
            }
        }
        Err(e) => eprintln!("Failed to unmute system audio: {}", e),
    });
}

/// Runs once as the app tears down. Without this, quitting mid-recording
/// leaves the system muted and the overlay helper process running.
fn cleanup_on_exit(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();

    let previous_volume = state.previous_volume.lock().ok().and_then(|v| *v);
    if let Some(vol) = previous_volume {
        match unmute_system_audio(vol) {
            Ok(_) => println!("[exit] system audio restored to volume: {}", vol),
            Err(e) => eprintln!("[exit] failed to restore volume: {}", e),
        }
    }

    {
        let overlay = state.overlay.lock();
        if let Ok(ref overlay_guard) = overlay {
            if let Some(helper) = overlay_guard.as_ref() {
                helper.quit();
            }
        }
    }
}

/// Reads `auto_mute_enabled` without holding any lock the caller cares about.
fn auto_mute_enabled(app: &tauri::AppHandle) -> bool {
    app.store("config.json")
        .ok()
        .and_then(|s| s.get("config"))
        .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
        .map(|c| c.auto_mute_enabled)
        .unwrap_or(true)
}

/// Hotkey down.
///
/// `Idle` starts recording; `Toggled` stops it. `Armed` (key repeat) and
/// `Stopping` (transcription in flight) are ignored.
async fn on_hotkey_pressed(app: tauri::AppHandle) {
    let state = app.state::<AppState>();

    let current = match state.dictation.lock() {
        Ok(guard) => *guard,
        Err(e) => {
            eprintln!("Dictation state lock poisoned: {}", e);
            return;
        }
    };

    match current {
        DictationState::Idle => {
            // Start capture first: if the device cannot be opened we must not
            // mute the user's audio or show a HUD for a recording that is not
            // happening.
            let start_result = match state.recorder.lock() {
                Ok(mut recorder) => recorder.start_recording(),
                Err(e) => Err(format!("Recorder lock poisoned: {}", e).into()),
            };

            if let Err(e) = start_result {
                emit_error(&app, &format!("Could not start recording: {}", e));
                return;
            }

            if let Ok(mut guard) = state.dictation.lock() {
                *guard = DictationState::Armed {
                    pressed_at: Instant::now(),
                };
            }
            emit_state(&app, "recording");

            // Show the HUD before muting: muting shells out to osascript twice
            // and used to delay the overlay by 100-300 ms.
            {
                let overlay = state.overlay.lock();
                if let Ok(ref guard) = overlay {
                    if let Some(helper) = guard.as_ref() {
                        if let Err(e) = helper.show_centered_bottom() {
                            eprintln!("Failed to show overlay: {}", e);
                        }
                    }
                }
            }

            if auto_mute_enabled(&app) {
                let app_for_mute = app.clone();
                // osascript is a subprocess; keep it off the async runtime.
                tauri::async_runtime::spawn_blocking(move || match mute_system_audio() {
                    Ok(previous) => {
                        let state = app_for_mute.state::<AppState>();
                        if let Ok(mut prev) = state.previous_volume.lock() {
                            *prev = Some(previous);
                        }
                        println!("System audio muted (previous volume: {})", previous);
                    }
                    Err(e) => eprintln!("Failed to mute system audio: {}", e),
                });
            }
        }

        DictationState::Toggled => {
            if let Ok(mut guard) = state.dictation.lock() {
                *guard = DictationState::Stopping;
            }
            println!("Toggle off");
            stop_recording_and_transcribe(app.clone());
        }

        DictationState::Armed { .. } | DictationState::Stopping => {
            // Key repeat while held, or a press during transcription.
        }
    }
}

/// Hotkey up. Only `Armed` is meaningful: long press stops, short press
/// switches to toggle mode.
async fn on_hotkey_released(app: tauri::AppHandle) {
    let state = app.state::<AppState>();

    let pressed_at = {
        let mut guard = match state.dictation.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Dictation state lock poisoned: {}", e);
                return;
            }
        };

        match *guard {
            DictationState::Armed { pressed_at } => {
                if pressed_at.elapsed() > HOLD_THRESHOLD {
                    *guard = DictationState::Stopping;
                    Some(pressed_at)
                } else {
                    *guard = DictationState::Toggled;
                    None
                }
            }
            // Includes a release whose task outran its own press.
            _ => return,
        }
    };

    match pressed_at {
        Some(pressed_at) => {
            println!("Long press ({:?}) - stopping", pressed_at.elapsed());
            stop_recording_and_transcribe(app.clone());
        }
        None => println!("Short press - kept recording (toggle mode)"),
    }
}

/// Stops capture, restores volume, and hands off to transcription.
/// The caller has already moved the state machine to `Stopping`.
fn stop_recording_and_transcribe(app: tauri::AppHandle) {
    {
        let state = app.state::<AppState>();
        let recorder = state.recorder.lock();
        if let Ok(mut recorder) = recorder {
            if let Err(e) = recorder.stop_recording() {
                eprintln!("Failed to stop recording: {}", e);
            }
        }
    }

    emit_state(&app, "transcribing");
    unmute_if_needed(app.clone());
    stop_and_transcribe_logic(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    let app_handle = app.clone();
                    match event.state() {
                        ShortcutState::Pressed => {
                            println!("Shortcut pressed: {:?}", shortcut);
                            tauri::async_runtime::spawn(async move {
                                on_hotkey_pressed(app_handle).await
                            });
                        }
                        ShortcutState::Released => {
                            println!("Shortcut released: {:?}", shortcut);
                            tauri::async_runtime::spawn(async move {
                                on_hotkey_released(app_handle).await
                            });
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Init store
            init_store(app.handle())?;

            // --- SYSTEM TRAY SETUP ---
            let quit_i =
                tauri::menu::MenuItem::with_id(app.handle(), "quit", "Quit", true, None::<&str>)?;
            let show_i = tauri::menu::MenuItem::with_id(
                app.handle(),
                "show",
                "Open Settings",
                true,
                None::<&str>,
            )?;
            let menu = tauri::menu::Menu::with_items(app.handle(), &[&show_i, &quit_i])?;

            let _tray =
                tauri::tray::TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                        match event.id().as_ref() {
                            "quit" => {
                                app.exit(0);
                            }
                            "show" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;
            // -------------------------

            // Init Transcriber
            let resource_path = app
                .path()
                .resource_dir()?
                .join("models")
                .join("ggml-base.en.bin");
            let transcriber = if resource_path.exists() {
                Transcriber::new(resource_path.to_str().unwrap()).ok()
            } else {
                None
            };

            // Init Overlay Helper (macOS)
            let overlay_helper = match OverlayHelper::new() {
                Ok(helper) => {
                    println!("Overlay helper started successfully");
                    Some(helper)
                }
                Err(e) => {
                    eprintln!("Failed to start overlay helper: {}", e);
                    None
                }
            };

            // Init State
            app.manage(AppState {
                recorder: Mutex::new(AudioRecorder::new()),
                transcriber: Mutex::new(transcriber),
                dictation: Mutex::new(DictationState::Idle),
                previous_volume: Mutex::new(None),
                overlay: Mutex::new(overlay_helper),
            });

            // Register hotkey from config. Never propagate an error here: a
            // malformed config would panic on every launch, leaving the app
            // unstartable until the store was deleted by hand.
            let store = app.store("config.json")?;
            let config = load_config(&store);

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_permissions,
            start_recording,
            stop_recording,
            save_test_audio,
            transcribe_test_audio,
            inject_test_text,
            get_hotkey,
            save_hotkey,
            get_auto_mute_enabled,
            set_auto_mute_enabled,
            get_language,
            set_language
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Err(e) = window.hide() {
                    eprintln!("Failed to hide window on close: {}", e);
                }
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                cleanup_on_exit(app_handle);
            }
        });
}

#[cfg(test)]
mod hotkey_tests {
    use tauri_plugin_global_shortcut::Shortcut;

    /// HotkeyRecorder.vue builds shortcut strings client-side from `e.code`.
    /// This pins the grammar those strings must satisfy.
    #[test]
    fn strings_the_ui_can_emit_all_parse() {
        for s in [
            "Cmd+Option+Space", // the shipped default
            "Command+Alt+Space",
            "Command+Shift+KeyA",
            "Control+Digit1",
            "Command+ArrowUp",
            "Command+Alt+Minus",
            "Command+Shift+BracketLeft",
            "F5",
        ] {
            assert!(s.parse::<Shortcut>().is_ok(), "{s} should parse");
        }
    }

    /// Configs written by the previous `e.key`-based recorder must keep
    /// working after the switch to `e.code`.
    #[test]
    fn previously_stored_hotkey_formats_still_parse() {
        for s in ["Command+Shift+.", "Command+Alt+SPACE", "Control+A"] {
            assert!(s.parse::<Shortcut>().is_ok(), "{s} should still parse");
        }
    }

    /// Modifiers must precede the key, and a key is required.
    #[test]
    fn rejects_malformed_shortcuts() {
        assert!("Command+KeyA+Shift".parse::<Shortcut>().is_err());
        assert!("Command+Shift".parse::<Shortcut>().is_err());
    }
}
