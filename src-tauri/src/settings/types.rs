use serde::{Deserialize, Serialize};

pub const DEFAULT_API_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";
pub const DEFAULT_TRANSCRIPTION_MODEL: &str = "whisper-large-v3";
pub const DEFAULT_POST_PROCESSING_MODEL: &str = "llama-3.3-70b-versatile";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_key_encrypted: String,
    pub api_base_url: String,
    pub has_completed_setup: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default = "default_transcription_model")]
    pub transcription_model: String,
    #[serde(default = "default_post_processing_model")]
    pub post_processing_model: String,
    #[serde(default = "default_true")]
    pub preserve_clipboard: bool,
}

fn default_shortcut() -> String { DEFAULT_SHORTCUT.to_string() }
fn default_transcription_model() -> String { DEFAULT_TRANSCRIPTION_MODEL.to_string() }
fn default_post_processing_model() -> String { DEFAULT_POST_PROCESSING_MODEL.to_string() }
fn default_true() -> bool { true }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_key_encrypted: String::new(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            has_completed_setup: false,
            shortcut: DEFAULT_SHORTCUT.to_string(),
            transcription_model: DEFAULT_TRANSCRIPTION_MODEL.to_string(),
            post_processing_model: DEFAULT_POST_PROCESSING_MODEL.to_string(),
            preserve_clipboard: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSettings {
    pub api_base_url: String,
    pub has_completed_setup: bool,
    pub shortcut: String,
    pub transcription_model: String,
    pub post_processing_model: String,
    pub preserve_clipboard: bool,
}

impl From<&AppSettings> for FrontendSettings {
    fn from(s: &AppSettings) -> Self {
        Self {
            api_base_url: s.api_base_url.clone(),
            has_completed_setup: s.has_completed_setup,
            shortcut: s.shortcut.clone(),
            transcription_model: s.transcription_model.clone(),
            post_processing_model: s.post_processing_model.clone(),
            preserve_clipboard: s.preserve_clipboard,
        }
    }
}
