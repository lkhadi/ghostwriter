// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod audio_control;
mod audio_recorder;
mod config;
mod injector;
mod logic_helper;
mod transcriber;

#[cfg(target_os = "macos")]
mod overlay_helper;

use audio_control::{mute_system_audio, unmute_system_audio};
use audio_recorder::AudioRecorder;
use config::{init_store, AppConfig};
use injector::Injector;
use logic_helper::stop_and_transcribe_logic;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri::State;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_store::StoreExt;
use transcriber::Transcriber;

#[cfg(target_os = "macos")]
use overlay_helper::OverlayHelper;

pub struct AppState {
    pub recorder: Mutex<AudioRecorder>,
    pub transcriber: Mutex<Option<Transcriber>>,
    pub press_time: Mutex<Option<Instant>>,
    /// Stores the previous volume before muting (0-100), None if not muted by us
    pub previous_volume: Mutex<Option<u32>>,
    #[cfg(target_os = "macos")]
    pub overlay: Mutex<Option<OverlayHelper>>,
}

#[tauri::command]
fn get_hotkey(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let config = store.get("config").ok_or("Config not found")?;
    let config: AppConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    Ok(config.hotkey)
}

#[tauri::command]
fn save_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    println!("[save_hotkey] Called with: {}", hotkey);

    let store = app.store("config.json").map_err(|e| {
        eprintln!("[save_hotkey] Store error: {}", e);
        e.to_string()
    })?;

    // 1. Unregister old if possible
    println!("[save_hotkey] Unregistering old shortcuts...");
    app.global_shortcut().unregister_all().map_err(|e| {
        eprintln!("[save_hotkey] Unregister error: {}", e);
        e.to_string()
    })?;

    // 2. Register new
    println!("[save_hotkey] Parsing shortcut: {}", hotkey);
    let shortcut = hotkey
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| {
            eprintln!("[save_hotkey] Parse error: {}", e);
            e.to_string()
        })?;

    println!("[save_hotkey] Registering shortcut...");
    app.global_shortcut().register(shortcut).map_err(|e| {
        eprintln!("[save_hotkey] Register error: {}", e);
        e.to_string()
    })?;

    // 3. Save to store
    println!("[save_hotkey] Saving to store...");
    // Get existing config to preserve auto_mute_enabled
    let existing_config: Option<AppConfig> = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok());

    let config = AppConfig {
        hotkey: hotkey.clone(),
        auto_mute_enabled: existing_config
            .as_ref()
            .map(|c| c.auto_mute_enabled)
            .unwrap_or(true),
        language: existing_config
            .as_ref()
            .map(|c| c.language.clone())
            .unwrap_or_else(|| "en".to_string()),
    };
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

    // Get existing config to preserve hotkey
    let existing_config: Option<AppConfig> = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok());

    let config = AppConfig {
        hotkey: existing_config
            .as_ref()
            .map(|c| c.hotkey.clone())
            .unwrap_or_else(|| "Cmd+Option+Space".to_string()),
        auto_mute_enabled: enabled,
        language: existing_config
            .as_ref()
            .map(|c| c.language.clone())
            .unwrap_or_else(|| "en".to_string()),
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

    let existing_config: Option<AppConfig> = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok());

    let config = AppConfig {
        hotkey: existing_config
            .as_ref()
            .map(|c| c.hotkey.clone())
            .unwrap_or_else(|| "Cmd+Option+Space".to_string()),
        auto_mute_enabled: existing_config
            .as_ref()
            .map(|c| c.auto_mute_enabled)
            .unwrap_or(true),
        language: language.clone(),
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
    let audio_data = recorder.get_audio();

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
fn transcribe_test_audio(app: tauri::AppHandle) -> Result<String, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("models")
        .join("ggml-base.en.bin");

    if !resource_path.exists() {
        return Err(format!("Model not found at {:?}", resource_path));
    }

    let transcriber =
        Transcriber::new(resource_path.to_str().unwrap()).map_err(|e| e.to_string())?;

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

    let text = transcriber
        .transcribe(&samples, &language)
        .map_err(|e| e.to_string())?;
    Ok(text)
}

#[tauri::command]
fn inject_test_text(text: String) -> Result<String, String> {
    let mut injector = Injector::new().map_err(|e| e.to_string())?;
    injector.type_text(&text).map_err(|e| e.to_string())?;
    Ok("Text injected".to_string())
}

/// Helper function to unmute system audio if we muted it
fn unmute_if_needed(state: &State<AppState>) {
    let previous_vol = {
        match state.previous_volume.lock() {
            Ok(prev) => *prev,
            _ => None,
        }
    };

    if let Some(vol) = previous_vol {
        match unmute_system_audio(vol) {
            Ok(_) => {
                println!("System audio restored to volume: {}", vol);
                if let Ok(mut prev) = state.previous_volume.lock() {
                    *prev = None;
                }
            }
            Err(e) => {
                eprintln!("Failed to unmute system audio: {}", e);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    let app_handle = app.clone();

                    if event.state() == ShortcutState::Pressed {
                        println!("Shortcut Pressed: {:?}", shortcut);
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();

                            // RECORD START LOGIC
                            let mut recorder = match state.recorder.lock() {
                                Ok(g) => g,
                                Err(e) => {
                                    eprintln!("Recorder lock error: {}", e);
                                    return;
                                }
                            };

                            if !recorder.is_recording() {
                                // START RECORDING
                                println!("Starting recording...");
                                if recorder.start_recording().is_ok() {
                                    // Check if auto-mute is enabled
                                    let auto_mute_enabled = {
                                        let store = app_handle.store("config.json").ok();
                                        store
                                            .and_then(|s| s.get("config"))
                                            .and_then(|c| {
                                                serde_json::from_value::<AppConfig>(c).ok()
                                            })
                                            .map(|c| c.auto_mute_enabled)
                                            .unwrap_or(true)
                                    };

                                    // MUTE SYSTEM AUDIO (if enabled)
                                    if auto_mute_enabled {
                                        match mute_system_audio() {
                                            Ok(previous_vol) => {
                                                if let Ok(mut prev) = state.previous_volume.lock() {
                                                    *prev = Some(previous_vol);
                                                }
                                                println!(
                                                    "System audio muted (previous volume: {})",
                                                    previous_vol
                                                );
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to mute system audio: {}", e);
                                            }
                                        }
                                    }

                                    if let Ok(mut pt) = state.press_time.lock() {
                                        *pt = Some(Instant::now());
                                    }

                                    // SHOW HUD centered near bottom of screen (macOS)
                                    #[cfg(target_os = "macos")]
                                    {
                                        let overlay = state.overlay.lock();
                                        if let Ok(ref overlay_guard) = overlay {
                                            if let Some(helper) = overlay_guard.as_ref() {
                                                if let Err(e) = helper.show_centered_bottom() {
                                                    eprintln!("Failed to show overlay: {}", e);
                                                }
                                            }
                                        }
                                    }

                                    #[cfg(not(target_os = "macos"))]
                                    if let Some(hud) = app_handle.get_webview_window("hud") {
                                        let mouse = Mouse::get_mouse_position();
                                        if let Mouse::Position { x, y } = mouse {
                                            let new_x = x - 110;
                                            let new_y = y - 80;
                                            let _ = hud.set_position(tauri::Position::Physical(
                                                tauri::PhysicalPosition { x: new_x, y: new_y },
                                            ));
                                        }
                                        let _ = hud.show();
                                    }
                                }
                            } else {
                                // ALREADY RECORDING - TOGGLE OFF?
                                // If we are here, it means we pressed again while recording.
                                // This is the "Toggle Off" action.
                                println!("Toggle Off (Pressed again)");
                                let _ = recorder.stop_recording();
                                drop(recorder); // Release lock before transcribe

                                // Unmute system audio
                                unmute_if_needed(&state);

                                // Spawn transcribe task logic (helper needed or inline)
                                stop_and_transcribe_logic(app_handle.clone());
                            }
                        });
                    } else if event.state() == ShortcutState::Released {
                        println!("Shortcut Released: {:?}", shortcut);
                        // Check for HOLD logic
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();

                            let start_time = {
                                let pt = state.press_time.lock().unwrap(); // safe unwrap or match
                                *pt
                            };

                            if let Some(time) = start_time {
                                let duration = time.elapsed();
                                println!("Press duration: {:?}", duration);

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
                                        unmute_if_needed(&state);

                                        // Reset press time so we don't trigger again
                                        if let Ok(mut pt) = state.press_time.lock() {
                                            *pt = None;
                                        }
                                        stop_and_transcribe_logic(app_handle.clone());
                                    }
                                } else {
                                    // SHORT PRESS -> DETECT TOGGLE MODE
                                    // Do nothing. Keep recording.
                                    println!("Short press detected - Kept recording (Toggle Mode)");
                                }
                            }
                        });
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
            #[cfg(target_os = "macos")]
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
                press_time: Mutex::new(None),
                previous_volume: Mutex::new(None),
                #[cfg(target_os = "macos")]
                overlay: Mutex::new(overlay_helper),
            });

            // Register hotkey from config
            let store = app.store("config.json")?;
            if let Some(config_val) = store.get("config") {
                let config: AppConfig = serde_json::from_value(config_val)?;
                let shortcut = config
                    .hotkey
                    .parse::<tauri_plugin_global_shortcut::Shortcut>()?;
                app.global_shortcut().register(shortcut)?;
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
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
