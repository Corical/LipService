pub mod encoder;
pub mod recorder;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("No input device available")]
    NoInputDevice,
    #[error("Recording failed: {0}")]
    RecordingFailed(String),
    #[error("Not currently recording")]
    NotRecording,
    #[error("Encoding failed: {0}")]
    Encoding(#[from] encoder::EncoderError),
}

#[async_trait]
pub trait AudioRecorder: Send + Sync {
    fn start(&self) -> Result<(), AudioError>;
    async fn stop_and_get_audio(&self) -> Result<Vec<u8>, AudioError>;
    fn is_recording(&self) -> bool;
}
