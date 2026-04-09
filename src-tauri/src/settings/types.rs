use serde::{Deserialize, Serialize};

pub const DEFAULT_API_BASE_URL: &str = "https://api.groq.com/openai/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_key_encrypted: String,
    pub api_base_url: String,
    pub has_completed_setup: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_key_encrypted: String::new(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            has_completed_setup: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSettings {
    pub api_base_url: String,
    pub has_completed_setup: bool,
}

impl From<&AppSettings> for FrontendSettings {
    fn from(s: &AppSettings) -> Self {
        Self {
            api_base_url: s.api_base_url.clone(),
            has_completed_setup: s.has_completed_setup,
        }
    }
}
