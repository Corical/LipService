pub mod types;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use types::AppSettings;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
}

pub fn settings_dir() -> PathBuf {
    let app_data = dirs::data_dir().expect("no APPDATA directory");
    app_data.join("VTT")
}

fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

pub fn load() -> Result<AppSettings, SettingsError> {
    let path = settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(&path)?;
    let settings: AppSettings = serde_json::from_str(&content)?;
    Ok(settings)
}

pub fn save(settings: &AppSettings) -> Result<(), SettingsError> {
    let dir = settings_dir();
    fs::create_dir_all(&dir)?;
    let content = serde_json::to_string_pretty(settings)?;
    let temp_path = dir.join("settings.json.tmp");
    let final_path = settings_path();
    fs::write(&temp_path, &content)?;
    fs::rename(&temp_path, &final_path)?;
    Ok(())
}

pub fn encrypt_api_key(plaintext: &str) -> Result<String, SettingsError> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Security::Cryptography::*;

        let data_bytes = plaintext.as_bytes();
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: data_bytes.len() as u32,
            pbData: data_bytes.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptProtectData(
                &mut data_in,
                None,
                None,
                None,
                None,
                0u32,
                &mut data_out,
            ).map_err(|e| SettingsError::Encryption(e.to_string()))?;

            let slice = std::slice::from_raw_parts(
                data_out.pbData,
                data_out.cbData as usize,
            );
            let encoded = BASE64.encode(slice);
            // Free the buffer allocated by CryptProtectData
            windows::Win32::Foundation::LocalFree(
                windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _)
            );
            Ok(encoded)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(BASE64.encode(plaintext.as_bytes()))
    }
}

pub fn decrypt_api_key(encrypted: &str) -> Result<String, SettingsError> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Security::Cryptography::*;

        let encrypted_bytes = BASE64.decode(encrypted)
            .map_err(|e| SettingsError::Encryption(e.to_string()))?;
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: encrypted_bytes.len() as u32,
            pbData: encrypted_bytes.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptUnprotectData(
                &mut data_in,
                None,
                None,
                None,
                None,
                0u32,
                &mut data_out,
            ).map_err(|e| SettingsError::Encryption(e.to_string()))?;

            let slice = std::slice::from_raw_parts(
                data_out.pbData,
                data_out.cbData as usize,
            );
            let plaintext = String::from_utf8_lossy(slice).to_string();
            windows::Win32::Foundation::LocalFree(
                windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _)
            );
            Ok(plaintext)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let bytes = BASE64.decode(encrypted)
            .map_err(|e| SettingsError::Encryption(e.to_string()))?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

pub fn load_api_key() -> Result<String, SettingsError> {
    let settings = load()?;
    decrypt_api_key(&settings.api_key_encrypted)
}

pub fn save_with_api_key(api_key: &str, api_base_url: &str) -> Result<(), SettingsError> {
    let encrypted = encrypt_api_key(api_key)?;
    let settings = AppSettings {
        api_key_encrypted: encrypted,
        api_base_url: api_base_url.to_string(),
        has_completed_setup: true,
    };
    save(&settings)
}
