pub mod transcription;
pub mod post_process;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Request failed (HTTP {status}): {body}")]
    RequestFailed { status: u16, body: String },
    #[error("Request timed out after {0}s")]
    Timeout(u64),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Network error: {0}")]
    Network(String),
}

#[async_trait]
pub trait TranscriptionService: Send + Sync {
    async fn transcribe(&self, audio_wav: &[u8]) -> Result<String, ApiError>;
}

#[async_trait]
pub trait PostProcessingService: Send + Sync {
    async fn process(&self, transcript: &str) -> Result<String, ApiError>;
}

pub async fn validate_api_key(api_key: &str, base_url: &str) -> bool {
    let client = reqwest::Client::new();
    let url = format!("{}/models", base_url);
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    matches!(response, Ok(r) if r.status().is_success())
}
