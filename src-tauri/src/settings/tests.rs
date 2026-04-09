use super::*;
use super::types::*;

#[test]
fn test_default_settings() {
    let s = AppSettings::default();
    assert!(!s.has_completed_setup);
    assert!(s.api_key_encrypted.is_empty());
    assert_eq!(s.api_base_url, DEFAULT_API_BASE_URL);
}

#[test]
fn test_frontend_settings_omits_key() {
    let s = AppSettings {
        api_key_encrypted: "secret".to_string(),
        api_base_url: "https://example.com".to_string(),
        has_completed_setup: true,
    };
    let frontend: FrontendSettings = (&s).into();
    assert_eq!(frontend.api_base_url, "https://example.com");
    assert!(frontend.has_completed_setup);
}

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let original = "gsk_test_key_12345";
    let encrypted = encrypt_api_key(original).unwrap();
    assert_ne!(encrypted, original);
    let decrypted = decrypt_api_key(&encrypted).unwrap();
    assert_eq!(decrypted, original);
}

#[test]
fn test_decrypt_empty_string() {
    let result = decrypt_api_key("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_settings_serialization_roundtrip() {
    let settings = AppSettings {
        api_key_encrypted: "encrypted_value".to_string(),
        api_base_url: "https://custom.api.com/v1".to_string(),
        has_completed_setup: true,
    };
    let json = serde_json::to_string(&settings).unwrap();
    let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.api_key_encrypted, settings.api_key_encrypted);
    assert_eq!(deserialized.api_base_url, settings.api_base_url);
    assert_eq!(deserialized.has_completed_setup, settings.has_completed_setup);
}
