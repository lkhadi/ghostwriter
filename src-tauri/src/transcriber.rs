use std::error::Error;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    pub fn new(model_path: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = Path::new(model_path);
        if !path.exists() {
            return Err(format!("Model not found at {}", model_path).into());
        }

        // Use new_with_params for 0.13 compatibility
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| format!("Failed to load model: {}", e))?;

        Ok(Self { ctx })
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("Failed to create state: {}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_translate(false);
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Suppress non-speech tokens like [MUSIC], [NOISE], etc.
        params.set_suppress_non_speech_tokens(true);
        // Suppress blank audio/silence hallucinations
        params.set_suppress_blank(true);
        // Set a threshold for no_speech probability (default is usually 0.6)
        params.set_no_speech_thold(0.6);

        state
            .full(params, audio_data)
            .map_err(|e| format!("Failed to run model: {}", e))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get segments: {}", e))?;
        let mut text = String::new();
        for i in 0..num_segments {
            let segment = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {}", e))?;
            text.push_str(&segment);
        }

        // Sanitize text - remove music notes and other unwanted characters
        // Sanitize text - remove music notes and other unwanted characters
        let mut cleaned_text = text
            .replace("♪", "")
            .replace("♫", "")
            .replace("♬", "")
            .replace("♭", "")
            .replace("♮", "")
            .replace("♯", "")
            .trim()
            .to_string();

        // Common Whisper hallucinations to filter out
        let hallucinations = [
            "Subtitles by",
            "Amara.org",
            "Thank you",
            "Translated by",
            "captioned by",
        ];

        for h in hallucinations.iter() {
            if cleaned_text.contains(h) {
                cleaned_text = String::new();
                break;
            }
        }

        // Check for specific "you" or "you." hallucination
        if cleaned_text == "you"
            || cleaned_text == "you."
            || cleaned_text == "You"
            || cleaned_text == "You."
        {
            cleaned_text = String::new();
        }

        // Remove repeated dots or spaces (e.g. ". . . .")
        // Simple manual check for repetitive non-alphanumeric content
        let alphanumeric_count = cleaned_text.chars().filter(|c| c.is_alphanumeric()).count();
        if alphanumeric_count == 0 {
            cleaned_text = String::new();
        }

        // Final sanity check for short length
        if cleaned_text.len() < 2 {
            cleaned_text = String::new();
        }

        Ok(cleaned_text)
    }
}
