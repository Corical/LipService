use super::{ApiError, TranscriptionService};
use async_trait::async_trait;
use reqwest::multipart;
use std::time::Duration;

const TRANSCRIPTION_TIMEOUT_SECS: u64 = 20;
const WHISPER_MODEL: &str = "whisper-large-v3";

pub struct GroqTranscription {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl GroqTranscription {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");

        Self { api_key, base_url, model, client }
    }
}

#[async_trait]
impl TranscriptionService for GroqTranscription {
    async fn transcribe(&self, audio_wav: &[u8]) -> Result<String, ApiError> {
        let url = format!("{}/audio/transcriptions", self.base_url);

        let audio_part = multipart::Part::bytes(audio_wav.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let form = multipart::Form::new()
            .text("model", if self.model.is_empty() { WHISPER_MODEL.to_string() } else { self.model.clone() })
            .part("file", audio_part);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ApiError::Timeout(TRANSCRIPTION_TIMEOUT_SECS)
                } else {
                    ApiError::Network(e.to_string())
                }
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::RequestFailed { status, body });
        }

        let json: serde_json::Value = response.json().await
            .map_err(|e| ApiError::InvalidResponse(e.to_string()))?;

        json["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::InvalidResponse("Missing 'text' field".to_string()))
    }
}
