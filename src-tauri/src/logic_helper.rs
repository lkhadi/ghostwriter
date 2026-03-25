use crate::audio_control::unmute_system_audio;
use crate::injector::Injector;
use crate::AppState;
use tauri::Manager;

pub fn stop_and_transcribe_logic(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
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
                        match transcriber.transcribe(&audio) {
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

        // Unmute system audio
        let previous_vol = {
            if let Ok(prev) = state.previous_volume.lock() {
                *prev
            } else {
                None
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

        // HIDE HUD using overlay helper
        #[cfg(target_os = "macos")]
        {
            let overlay = state.overlay.lock();
            if let Ok(ref overlay_guard) = overlay {
                if let Some(helper) = overlay_guard.as_ref() {
                    if let Err(e) = helper.hide() {
                        eprintln!("Failed to hide overlay: {}", e);
                    }
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        if let Some(hud) = app.get_webview_window("hud") {
            let _ = hud.hide();
        }
    });
}
