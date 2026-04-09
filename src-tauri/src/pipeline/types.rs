use serde::Serialize;
use crate::audio::AudioError;
use crate::api::ApiError;
use crate::clipboard::ClipboardError;

#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    pub raw: String,
    pub cleaned: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineState {
    Idle,
    Recording,
    Transcribing,
    Processing,
    Pasting,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Recording failed: {0}")]
    Audio(#[from] AudioError),
    #[error("Transcription failed: {0}")]
    Transcription(ApiError),
    #[error("Post-processing failed: {0}")]
    PostProcessing(ApiError),
    #[error("Paste failed: {0}")]
    Clipboard(#[from] ClipboardError),
}
