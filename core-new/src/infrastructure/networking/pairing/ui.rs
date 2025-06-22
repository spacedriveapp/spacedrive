//! User interface abstractions for pairing confirmation
//!
//! This module provides the core trait for pairing user interfaces,
//! without any CLI-specific implementations. CLI implementations
//! should be placed in the CLI infrastructure modules.

use async_trait::async_trait;
use crate::networking::{DeviceInfo, NetworkError, Result};
use super::PairingState;

/// Trait for pairing user interface
#[async_trait]
pub trait PairingUserInterface: Send + Sync {
    /// Ask user to confirm pairing with remote device
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool>;
    
    /// Show pairing progress to user
    async fn show_pairing_progress(&self, state: PairingState);
    
    /// Display pairing error to user  
    async fn show_pairing_error(&self, error: &NetworkError);
    
    /// Display pairing code to user
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32);
    
    /// Prompt user to enter pairing code
    async fn prompt_pairing_code(&self) -> Result<[String; 12]>;
    
    /// Display pairing code object to user
    async fn display_pairing_code(&self, code: &super::PairingCode) -> Result<()> {
        let expires_in = (code.expires_at - chrono::Utc::now()).num_seconds().max(0) as u32;
        self.show_pairing_code(&code.as_string(), expires_in).await;
        Ok(())
    }
    
    /// Get pairing code from user as vector of strings
    async fn get_pairing_code_from_user(&self) -> Result<Vec<String>> {
        let words = self.prompt_pairing_code().await?;
        Ok(words.to_vec())
    }
}

// NOTE: ConsolePairingUI has been moved to CLI infrastructure
// See: src/infrastructure/cli/pairing_ui.rs for CLI-specific implementations

/// Mock UI for testing
pub struct MockPairingUI {
    pub should_confirm: bool,
    pub pairing_code_response: Option<[String; 12]>,
}

impl MockPairingUI {
    pub fn new(should_confirm: bool) -> Self {
        Self {
            should_confirm,
            pairing_code_response: None,
        }
    }
    
    pub fn with_pairing_code(mut self, code: [String; 12]) -> Self {
        self.pairing_code_response = Some(code);
        self
    }
}

#[async_trait]
impl PairingUserInterface for MockPairingUI {
    async fn confirm_pairing(&self, _remote_device: &DeviceInfo) -> Result<bool> {
        Ok(self.should_confirm)
    }
    
    async fn show_pairing_progress(&self, _state: PairingState) {
        // Silent for tests
    }
    
    async fn show_pairing_error(&self, _error: &NetworkError) {
        // Silent for tests
    }
    
    async fn show_pairing_code(&self, _code: &str, _expires_in_seconds: u32) {
        // Silent for tests
    }
    
    async fn prompt_pairing_code(&self) -> Result<[String; 12]> {
        self.pairing_code_response
            .clone()
            .ok_or_else(|| NetworkError::AuthenticationFailed("No pairing code set in mock".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::identity::PublicKey;
    use uuid::Uuid;

    fn create_test_device_info() -> DeviceInfo {
        use crate::networking::{DeviceInfo, PublicKey, NetworkFingerprint};
        use chrono::Utc;
        
        let device_id = Uuid::new_v4();
        let public_key = PublicKey::from_bytes(vec![42u8; 32]).unwrap();
        
        DeviceInfo {
            device_id,
            device_name: "Test Device".to_string(),
            public_key: public_key.clone(),
            network_fingerprint: NetworkFingerprint::from_device(device_id, &public_key),
            last_seen: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_mock_ui_confirm_accept() {
        let ui = MockPairingUI::new(true);
        let device = create_test_device_info();
        
        let result = ui.confirm_pairing(&device).await.unwrap();
        assert_eq!(result, true);
    }

    #[tokio::test]
    async fn test_mock_ui_confirm_reject() {
        let ui = MockPairingUI::new(false);
        let device = create_test_device_info();
        
        let result = ui.confirm_pairing(&device).await.unwrap();
        assert_eq!(result, false);
    }

    #[tokio::test]
    async fn test_mock_ui_pairing_code() {
        let test_code = [
            "word1".to_string(),
            "word2".to_string(),
            "word3".to_string(),
            "word4".to_string(),
            "word5".to_string(),
            "word6".to_string(),
            "word7".to_string(),
            "word8".to_string(),
            "word9".to_string(),
            "word10".to_string(),
            "word11".to_string(),
            "word12".to_string(),
        ];
        
        let ui = MockPairingUI::new(true).with_pairing_code(test_code.clone());
        
        let result = ui.prompt_pairing_code().await.unwrap();
        assert_eq!(result, test_code);
    }
}