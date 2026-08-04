use crate::config::AppConfig;
use crate::injector::Injector;
use crate::{emit_error, emit_state, AppState, DictationState};
use tauri::Manager;
use tauri_plugin_store::StoreExt;

/// Restores the app to a usable resting state when the transcription task
/// ends, by any path — early return, error, or panic.
///
/// Without this, the HUD stayed on screen and the state machine stayed in
/// `Stopping`, which ignores presses, so an empty recording would wedge
/// dictation until restart.
struct DictationGuard {
    app: tauri::AppHandle,
}

impl Drop for DictationGuard {
    fn drop(&mut self) {
        {
            let state = self.app.state::<AppState>();
            // Bind before matching so the guard drops before `state`.
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

        {
            let state = self.app.state::<AppState>();
            let dictation = state.dictation.lock();
            match dictation {
                Ok(mut guard) => *guard = DictationState::Idle,
                Err(e) => eprintln!("Dictation lock poisoned while resetting: {}", e),
            }
        }

        emit_state(&self.app, "idle");
    }
}

fn configured_language(app: &tauri::AppHandle) -> String {
    app.store("config.json")
        .ok()
        .and_then(|s| s.get("config"))
        .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
        .map(|c| c.language)
        .unwrap_or_else(|| "en".to_string())
}

pub fn stop_and_transcribe_logic(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _guard = DictationGuard { app: app.clone() };

        let audio = {
            let state = app.state::<AppState>();
            let recorder = state.recorder.lock();
            match recorder {
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
        let language = configured_language(&app);

        // Whisper inference is seconds of CPU work. Running it on an async
        // worker would block that thread for the whole duration, so it goes
        // to the blocking pool. `State<'_>` is not 'static, hence the clone.
        let app_for_inference = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let state = app_for_inference.state::<AppState>();
            let transcriber = state.transcriber.lock();
            match transcriber {
                Ok(guard) => match guard.as_ref() {
                    Some(transcriber) => transcriber
                        .transcribe(&audio, &language)
                        .map_err(|e| format!("Transcription failed: {}", e)),
                    None => Err(
                        "Whisper model was not loaded at startup — check the models directory"
                            .to_string(),
                    ),
                },
                Err(e) => Err(format!("Transcriber lock poisoned: {}", e)),
            }
        })
        .await;

        let text = match result {
            Ok(Ok(text)) => text,
            Ok(Err(e)) => {
                emit_error(&app, &e);
                return;
            }
            Err(e) => {
                emit_error(&app, &format!("Transcription task failed: {}", e));
                return;
            }
        };

        println!("Transcribed: {}", text);

        if text.is_empty() {
            println!("Nothing to inject.");
            return;
        }

        // System audio was already restored by the caller before this task
        // was spawned.

        let mut injector = match Injector::new() {
            Ok(injector) => injector,
            Err(e) => {
                emit_error(
                    &app,
                    &format!(
                        "Could not type text — check Accessibility permission: {}",
                        e
                    ),
                );
                return;
            }
        };

        if let Err(e) = injector.type_text(&text) {
            emit_error(&app, &format!("Failed to type text: {}", e));
        }

        // HUD hidden and state reset by `_guard` when this task returns.
    });
}
