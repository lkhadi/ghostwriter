use crate::config::AppConfig;
use crate::injector::Injector;
use crate::AppState;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

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
            // Bind before matching so the guard is dropped before `state`.
            let overlay = state.overlay.lock();
            match overlay {
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

pub fn stop_and_transcribe_logic(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _hud = HudGuard { app: app.clone() };
        let state = app.state::<AppState>();

        let audio = {
            match state.recorder.lock() {
                Ok(recorder) => recorder.get_audio(),
                Err(e) => {
                    eprintln!("Recorder lock poisoned: {}", e);
                    Vec::new()
                }
            }
        };

        if audio.is_empty() {
            println!("Audio empty, skipping transcription.");
            return;
        }

        println!("Transcribing {} samples...", audio.len());

        // Transcribe
        let text = {
            match state.transcriber.lock() {
                Ok(transcriber_guard) => {
                    if let Some(transcriber) = transcriber_guard.as_ref() {
                        // Get language from config
                        let language = {
                            let store = app.store("config.json").ok();
                            store
                                .and_then(|s| s.get("config"))
                                .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
                                .map(|c| c.language)
                                .unwrap_or_else(|| "en".to_string())
                        };

                        match transcriber.transcribe(&audio, &language) {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("Transcription error: {}", e);
                                return;
                            }
                        }
                    } else {
                        eprintln!("Transcriber not initialized");
                        return;
                    }
                }
                Err(e) => {
                    eprintln!("Transcriber lock poisoned: {}", e);
                    return;
                }
            }
        };

        println!("Transcribed: {}", text);

        // Note: system audio is already restored by the caller in lib.rs
        // before this task is spawned.

        // Inject
        let mut injector = match Injector::new() {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Injector init error: {}", e);
                return;
            }
        };
        if let Err(e) = injector.type_text(&text) {
            eprintln!("Injection error: {}", e);
        }

        // HUD is hidden by `_hud` (HudGuard) when this task returns.
    });
}
