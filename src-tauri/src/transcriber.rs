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

    pub fn transcribe(
        &self,
        audio_data: &[f32],
        language: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("Failed to create state: {}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_translate(false);
        params.set_language(Some(language));
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

        let cleaned = sanitize(&text);
        if is_hallucination(&cleaned) {
            return Ok(String::new());
        }

        Ok(cleaned)
    }
}

const MUSIC_GLYPHS: &[char] = &['♪', '♫', '♬', '♭', '♮', '♯'];

/// Phrases Whisper emits as an ENTIRE transcript when fed silence or noise.
/// Compared against the whole normalized transcript — never as substrings,
/// because "thank you" is ordinary dictation.
const FULL_TEXT_HALLUCINATIONS: &[&str] = &[
    "you",
    "thank you",
    "thanks for watching",
    "please subscribe",
];

fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !MUSIC_GLYPHS.contains(c))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Trim, drop trailing sentence punctuation, lowercase.
fn normalize_for_match(text: &str) -> String {
    text.trim()
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?') || c.is_whitespace())
        .trim()
        .to_lowercase()
}

fn is_hallucination(text: &str) -> bool {
    let normalized = normalize_for_match(text);
    if normalized.is_empty() {
        return true;
    }
    // Whisper's subtitle-credit artifact; this domain never appears in dictation.
    if normalized.contains("amara.org") {
        return true;
    }
    // Nothing to type if there isn't a single alphanumeric character.
    if !normalized.chars().any(|c| c.is_alphanumeric()) {
        return true;
    }
    FULL_TEXT_HALLUCINATIONS.contains(&normalized.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_dictation_that_contains_a_hallucination_phrase() {
        assert!(!is_hallucination(
            "Thank you for the update, I'll review it tonight"
        ));
    }

    #[test]
    fn drops_bare_thank_you() {
        assert!(is_hallucination("Thank you."));
        assert!(is_hallucination("thank you"));
    }

    #[test]
    fn drops_amara_subtitle_credit() {
        assert!(is_hallucination("Subtitles by the Amara.org community"));
    }

    #[test]
    fn drops_empty_and_punctuation_only() {
        assert!(is_hallucination("   "));
        assert!(is_hallucination("..."));
    }

    #[test]
    fn keeps_short_real_words() {
        assert!(!is_hallucination("no"));
        assert!(!is_hallucination("A"));
    }

    #[test]
    fn strips_music_glyphs() {
        assert_eq!(sanitize("♪ hello ♪"), "hello");
    }
}
