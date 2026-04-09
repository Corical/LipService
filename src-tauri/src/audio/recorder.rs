use super::encoder;
use super::{AudioError, AudioRecorder};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use std::sync::{Arc, Mutex};

struct RecordingState {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

// Stream is !Send, so we wrap it in a way that's safe for our use case.
// The stream is only accessed from the main thread (start/stop).
struct StreamWrapper(Stream);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

pub struct CpalRecorder {
    state: Arc<Mutex<Option<RecordingState>>>,
    stream: Mutex<Option<StreamWrapper>>,
}

// Safety: state is behind Arc<Mutex> (Send+Sync), stream is behind Mutex and
// only accessed on the main thread. The StreamWrapper makes this explicit.
unsafe impl Send for CpalRecorder {}
unsafe impl Sync for CpalRecorder {}

impl CpalRecorder {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            stream: Mutex::new(None),
        }
    }
}

#[async_trait]
impl AudioRecorder for CpalRecorder {
    fn start(&self) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;

        let config = device
            .default_input_config()
            .map_err(|e| AudioError::RecordingFailed(e.to_string()))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        {
            let mut state_guard = self.state.lock().unwrap();
            *state_guard = Some(RecordingState {
                samples: Vec::new(),
                sample_rate,
                channels,
            });
        }

        let state_for_callback = Arc::clone(&self.state);
        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut guard) = state_for_callback.lock() {
                        if let Some(ref mut recording) = *guard {
                            recording.samples.extend_from_slice(data);
                        }
                    }
                },
                |err| {
                    eprintln!("Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| AudioError::RecordingFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::RecordingFailed(e.to_string()))?;

        *self.stream.lock().unwrap() = Some(StreamWrapper(stream));

        Ok(())
    }

    async fn stop_and_get_audio(&self) -> Result<Vec<u8>, AudioError> {
        {
            let mut stream_guard = self.stream.lock().unwrap();
            *stream_guard = None;
        }

        let recording = {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.take().ok_or(AudioError::NotRecording)?
        };

        if recording.samples.is_empty() {
            return Err(AudioError::RecordingFailed(
                "No audio samples captured".to_string(),
            ));
        }

        let samples = recording.samples;
        let sample_rate = recording.sample_rate;
        let channels = recording.channels;

        tokio::task::spawn_blocking(move || {
            encoder::encode_to_wav(&samples, sample_rate, channels)
                .map_err(AudioError::Encoding)
        })
        .await
        .map_err(|e| AudioError::RecordingFailed(e.to_string()))?
    }

    fn is_recording(&self) -> bool {
        self.stream.lock().map(|s| s.is_some()).unwrap_or(false)
    }
}
