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

enum AudioCommand {
    Start,
    Stop,
}

/// Maximum recording duration in seconds (5 minutes)
const MAX_RECORDING_SECONDS: usize = 300;
/// Sample rate for Whisper (16kHz)
const SAMPLE_RATE: usize = 16000;
/// Maximum samples in buffer (5 min @ 16kHz = 4,800,000 samples)
const MAX_BUFFER_SAMPLES: usize = SAMPLE_RATE * MAX_RECORDING_SECONDS;

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
            let mut _stream: Option<cpal::Stream> = None;

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Start => {
                        let host = cpal::default_host();
                        if let Some(device) = host.default_input_device() {
                            if let Ok(config) = device.default_input_config() {
                                let stream_config: cpal::StreamConfig = config.clone().into();
                                let target_sample_rate = 16000;
                                let source_sample_rate = stream_config.sample_rate.0 as usize;
                                let channels = stream_config.channels as usize;

                                let buf_ref = buffer_arc.clone();

                                // Rubato setup (0.14.1)
                                let params = SincInterpolationParameters {
                                    sinc_len: 256,
                                    f_cutoff: 0.95,
                                    interpolation: SincInterpolationType::Linear,
                                    oversampling_factor: 256,
                                    window: WindowFunction::BlackmanHarris2,
                                };

                                let resampler_res = SincFixedIn::<f32>::new(
                                    target_sample_rate as f64 / source_sample_rate as f64,
                                    2.0,
                                    params,
                                    1024,
                                    channels,
                                );

                                if let Ok(mut resampler) = resampler_res {
                                    let chunk_size = 1024;
                                    let mut input_buffer: Vec<Vec<f32>> =
                                        vec![vec![0.0; chunk_size]; channels];
                                    let mut input_accumulator: Vec<f32> = Vec::new();

                                    let err_fn =
                                        |err| eprintln!("an error occurred on stream: {}", err);

                                    let stream_res = device.build_input_stream(
                                        &stream_config,
                                        move |data: &[f32], _: &_| {
                                            for frame in data.chunks(channels) {
                                                input_accumulator.push(frame[0]);
                                            }

                                            while input_accumulator.len() >= chunk_size {
                                                let chunk: Vec<f32> = input_accumulator
                                                    .drain(0..chunk_size)
                                                    .collect();
                                                input_buffer[0] = chunk;

                                                if let Ok(resampled_waves) =
                                                    resampler.process(&input_buffer, None)
                                                {
                                                    if let Some(wave) = resampled_waves.first() {
                                                        if let Ok(mut locked) = buf_ref.lock() {
                                                            // Ring buffer behavior:
                                                            // Remove oldest samples if at capacity
                                                            for sample in wave.iter() {
                                                                if locked.len()
                                                                    >= MAX_BUFFER_SAMPLES
                                                                {
                                                                    locked.pop_front();
                                                                    // Remove oldest
                                                                }
                                                                locked.push_back(*sample);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                        err_fn,
                                        None,
                                    );

                                    if let Ok(s) = stream_res {
                                        if s.play().is_ok() {
                                            _stream = Some(s);
                                            recording_arc.store(true, Ordering::SeqCst);
                                            println!(
                                                "Recording started. Max buffer: {} samples ({} seconds)",
                                                MAX_BUFFER_SAMPLES,
                                                MAX_RECORDING_SECONDS
                                            );
                                        }
                                    }
                                }
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

    pub fn start_recording(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Clear buffer before starting new recording
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
        }

        self.tx
            .send(AudioCommand::Start)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.tx
            .send(AudioCommand::Stop)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    }
}
