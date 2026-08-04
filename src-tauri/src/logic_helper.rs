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

/// Peak amplitude below which a recording is treated as "no signal".
///
/// This exists to catch a microphone that returned *nothing* — macOS hands
/// back zero-filled buffers when it denies capture, rather than failing. A
/// live microphone always carries some thermal noise, so anything at or
/// above this is real audio and belongs to Whisper, however quiet.
///
/// Deliberately near-zero: an earlier value of 0.005 was high enough to
/// reject genuinely quiet speech (a mic at low input gain peaks around
/// 0.003), which turned this diagnostic into a bug of its own.
const SILENCE_PEAK: f32 = 0.0001;

/// Peak and RMS amplitude of a capture buffer.
fn audio_level(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    (peak, rms)
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

        // Whisper hallucinates confidently when fed silence, and the
        // hallucination filter only knows English phrases — so a dead
        // microphone shows up as "nothing typed" in English and "random
        // words typed" in other languages. Neither looks like a mic problem.
        // Check the signal itself and say so.
        let (peak, rms) = audio_level(&audio);
        println!(
            "Captured {} samples ({:.1}s), peak {:.4}, rms {:.4}",
            audio.len(),
            audio.len() as f32 / 16000.0,
            peak,
            rms
        );

        if peak < SILENCE_PEAK {
            emit_error(
                &app,
                &format!(
                    "Microphone produced no signal (peak {:.4}). Check GhostWriter's \
                     Microphone permission in System Settings and the selected input device.",
                    peak
                ),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_zero_level_for_digital_silence() {
        let (peak, rms) = audio_level(&[0.0; 1000]);
        assert_eq!(peak, 0.0);
        assert_eq!(rms, 0.0);
        assert!(peak < SILENCE_PEAK, "digital silence must trip the check");
    }

    #[test]
    fn room_tone_is_not_treated_as_silence() {
        // ~0.003 peak: a live microphone at low input gain. Quiet, but real
        // speech lives here and must not be discarded.
        let tone: Vec<f32> = (0..1000)
            .map(|i| 0.003 * ((i as f32) * 0.1).sin())
            .collect();
        let (peak, _) = audio_level(&tone);
        assert!(
            peak > SILENCE_PEAK,
            "live mic room tone must pass, got {peak}"
        );
    }

    #[test]
    fn handles_empty_buffer() {
        assert_eq!(audio_level(&[]), (0.0, 0.0));
    }
}
