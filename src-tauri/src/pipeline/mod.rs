pub mod types;

use crate::api::{PostProcessingService, TranscriptionService};
use crate::audio::AudioRecorder;
use crate::clipboard::ClipboardService;
use std::sync::Arc;
use types::{PipelineError, PipelineResult};

pub struct DictationPipeline {
    recorder: Arc<dyn AudioRecorder>,
    transcriber: Arc<dyn TranscriptionService>,
    processor: Arc<dyn PostProcessingService>,
    clipboard: Arc<dyn ClipboardService>,
}

impl DictationPipeline {
    pub fn new(
        recorder: Arc<dyn AudioRecorder>,
        transcriber: Arc<dyn TranscriptionService>,
        processor: Arc<dyn PostProcessingService>,
        clipboard: Arc<dyn ClipboardService>,
    ) -> Self {
        Self { recorder, transcriber, processor, clipboard }
    }

    pub fn start_recording(&self) -> Result<(), PipelineError> {
        self.recorder.start().map_err(PipelineError::Audio)
    }

    pub async fn execute(&self) -> Result<PipelineResult, PipelineError> {
        let audio = self.recorder.stop_and_get_audio().await?;

        let transcript = self.transcriber.transcribe(&audio).await
            .map_err(PipelineError::Transcription)?;

        if transcript.trim().is_empty() {
            return Ok(PipelineResult {
                raw: transcript,
                cleaned: String::new(),
            });
        }

        let cleaned = self.processor.process(&transcript).await
            .map_err(PipelineError::PostProcessing)?;

        if !cleaned.is_empty() {
            self.clipboard.paste(&cleaned)?;
        }

        Ok(PipelineResult { raw: transcript, cleaned })
    }
}
