use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

enum AudioCommand {
    /// Carries a reply channel so stream-setup failures reach the caller
    /// instead of being swallowed on the audio thread.
    Start(Sender<Result<(), String>>),
    Stop,
}

/// How long `start_recording` waits for the audio thread to report back.
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum recording duration in seconds (5 minutes)
const MAX_RECORDING_SECONDS: usize = 300;
/// Sample rate for Whisper (16kHz)
const SAMPLE_RATE: usize = 16000;
/// Maximum samples in buffer (5 min @ 16kHz = 4,800,000 samples)
const MAX_BUFFER_SAMPLES: usize = SAMPLE_RATE * MAX_RECORDING_SECONDS;
/// Frames handed to the resampler at a time.
const CHUNK_SIZE: usize = 1024;

/// Opens the default input device and starts streaming resampled 16 kHz mono
/// audio into `buffer`.
///
/// Every failure here used to be an `if let Ok` with no `else`, so a denied
/// microphone permission or missing device produced a recording session that
/// captured nothing and reported success.
fn build_stream(buffer: Arc<Mutex<VecDeque<f32>>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| {
        "No input device available — check Microphone permission in System Settings".to_string()
    })?;

    let supported = device
        .default_input_config()
        .map_err(|e| format!("No supported input configuration: {}", e))?;

    // The callback below is typed `&[f32]`; building an f32 stream on a device
    // that reports another format would fail opaquely inside cpal.
    if supported.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!(
            "Unsupported input sample format {:?} (expected F32)",
            supported.sample_format()
        ));
    }

    let stream_config: cpal::StreamConfig = supported.into();
    let source_sample_rate = stream_config.sample_rate.0 as usize;
    let channels = stream_config.channels as usize;
    if channels == 0 {
        return Err("Input device reports zero channels".to_string());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // Only channel 0 is fed through, so resample a single channel rather than
    // paying for `channels` of which all but one are silence.
    let mut resampler = SincFixedIn::<f32>::new(
        SAMPLE_RATE as f64 / source_sample_rate as f64,
        2.0,
        params,
        CHUNK_SIZE,
        1,
    )
    .map_err(|e| {
        format!(
            "Failed to create resampler for {} Hz -> {} Hz: {}",
            source_sample_rate, SAMPLE_RATE, e
        )
    })?;

    let mut input_buffer: Vec<Vec<f32>> = vec![vec![0.0; CHUNK_SIZE]; 1];
    let mut accumulator: Vec<f32> = Vec::with_capacity(CHUNK_SIZE * 2);

    let stream = device
        .build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| {
                for frame in data.chunks(channels) {
                    accumulator.push(frame[0]);
                }

                while accumulator.len() >= CHUNK_SIZE {
                    input_buffer[0] = accumulator.drain(0..CHUNK_SIZE).collect();

                    match resampler.process(&input_buffer, None) {
                        Ok(resampled) => {
                            if let Some(wave) = resampled.first() {
                                if let Ok(mut locked) = buffer.lock() {
                                    // Ring buffer: drop oldest samples at capacity.
                                    for sample in wave.iter() {
                                        if locked.len() >= MAX_BUFFER_SAMPLES {
                                            locked.pop_front();
                                        }
                                        locked.push_back(*sample);
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!("Resampler error: {}", e),
                    }
                }
            },
            |err| eprintln!("Input stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {}", e))?;

    println!(
        "Input stream ready: {} Hz, {} ch -> {} Hz mono",
        source_sample_rate, channels, SAMPLE_RATE
    );

    Ok(stream)
}

pub struct AudioRecorder {
    tx: Sender<AudioCommand>,
    /// Fixed-capacity buffer that discards oldest samples when full (ring buffer behavior)
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
    is_recording: Arc<AtomicBool>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        // Create VecDeque with pre-allocated capacity
        let audio_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFER_SAMPLES)));
        let is_recording = Arc::new(AtomicBool::new(false));

        let buffer_arc = audio_buffer.clone();
        let recording_arc = is_recording.clone();

        thread::spawn(move || {
            // Owned here because a cpal::Stream is not Send. The binding is
            // never read — it exists so the stream stays alive until Stop
            // replaces it with None, which drops it and ends capture.
            let mut _stream: Option<cpal::Stream> = None;

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Start(reply) => {
                        let result = build_stream(buffer_arc.clone());
                        match result {
                            Ok(new_stream) => {
                                _stream = Some(new_stream);
                                recording_arc.store(true, Ordering::SeqCst);
                                println!(
                                    "Recording started. Max buffer: {} samples ({} seconds)",
                                    MAX_BUFFER_SAMPLES, MAX_RECORDING_SECONDS
                                );
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                eprintln!("Failed to start recording: {}", e);
                                recording_arc.store(false, Ordering::SeqCst);
                                let _ = reply.send(Err(e));
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        _stream = None; // Drop stream
                        recording_arc.store(false, Ordering::SeqCst);
                        println!("Recording stopped.");
                    }
                }
            }
        });

        Self {
            tx,
            audio_buffer,
            is_recording,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Get all audio samples and clear the buffer.
    /// Returns samples in chronological order (oldest first).
    pub fn get_audio(&self) -> Vec<f32> {
        match self.audio_buffer.lock() {
            Ok(mut buffer) => {
                let len = buffer.len();
                if len == 0 {
                    return Vec::new();
                }

                // Log buffer usage
                let usage_percent = (len as f64 / MAX_BUFFER_SAMPLES as f64) * 100.0;
                let duration_secs = len / SAMPLE_RATE;
                println!(
                    "Audio buffer: {} samples ({} seconds, {:.1}% of max capacity)",
                    len, duration_secs, usage_percent
                );

                if len >= MAX_BUFFER_SAMPLES {
                    println!("WARNING: Buffer was at max capacity. Oldest audio was discarded.");
                }

                // Drain all samples from buffer (converts VecDeque to Vec)
                let data: Vec<f32> = buffer.drain(..).collect();

                // Buffer is now empty
                println!("Buffer cleared after get_audio(). RAM freed.");

                data
            }
            _ => {
                Vec::new() // Return empty if poisoned
            }
        }
    }

    /// Copies the buffered audio without consuming it.
    ///
    /// `get_audio` drains, which is right for the transcription path but wrong
    /// for the debug "Save WAV" button — that used to destroy a recording that
    /// was still in progress.
    pub fn snapshot_audio(&self) -> Vec<f32> {
        match self.audio_buffer.lock() {
            Ok(buffer) => buffer.iter().copied().collect(),
            Err(e) => {
                eprintln!("Audio buffer lock poisoned: {}", e);
                Vec::new()
            }
        }
    }

    /// Returns the current buffer usage as a percentage (0.0 - 100.0)
    #[allow(dead_code)]
    pub fn buffer_usage_percent(&self) -> f64 {
        match self.audio_buffer.lock() {
            Ok(buffer) => (buffer.len() as f64 / MAX_BUFFER_SAMPLES as f64) * 100.0,
            _ => 0.0,
        }
    }

    /// Returns the current buffer duration in seconds
    #[allow(dead_code)]
    pub fn buffer_duration_seconds(&self) -> usize {
        match self.audio_buffer.lock() {
            Ok(buffer) => buffer.len() / SAMPLE_RATE,
            _ => 0,
        }
    }

    /// Starts capture, returning the real reason if the device could not be
    /// opened rather than reporting success and recording silence.
    pub fn start_recording(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Clear buffer before starting new recording
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
        }

        let (reply_tx, reply_rx) = channel();
        self.tx
            .send(AudioCommand::Start(reply_tx))
            .map_err(|e| format!("Audio thread is gone: {}", e))?;

        match reply_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(format!("Audio thread did not respond in time: {}", e).into()),
        }
    }

    pub fn stop_recording(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.tx
            .send(AudioCommand::Stop)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    }
}
