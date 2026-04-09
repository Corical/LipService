use std::thread;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("Failed to write to clipboard: {0}")]
    WriteFailed(String),
    #[error("Failed to simulate paste: {0}")]
    PasteFailed(String),
}

pub trait ClipboardService: Send + Sync {
    fn paste(&self, text: &str) -> Result<(), ClipboardError>;
}

pub struct WindowsClipboard;

impl WindowsClipboard {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardService for WindowsClipboard {
    fn paste(&self, text: &str) -> Result<(), ClipboardError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| ClipboardError::WriteFailed(e.to_string()))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| ClipboardError::WriteFailed(e.to_string()))?;

        thread::sleep(Duration::from_millis(50));

        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| ClipboardError::PasteFailed(e.to_string()))?;
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| ClipboardError::PasteFailed(e.to_string()))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| ClipboardError::PasteFailed(e.to_string()))?;
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| ClipboardError::PasteFailed(e.to_string()))?;

        Ok(())
    }
}
