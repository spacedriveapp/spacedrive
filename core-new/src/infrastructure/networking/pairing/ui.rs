//! User interface abstractions for pairing confirmation

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

/// Console-based pairing UI for CLI
pub struct ConsolePairingUI;

#[async_trait]
impl PairingUserInterface for ConsolePairingUI {
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool> {
        use dialoguer::Confirm;
        
        println!("\n🔗 Device Pairing Request");
        println!("=========================");
        println!("Device wants to pair with you:");
        println!("  Name: {}", remote_device.device_name);
        println!("  ID: {}", remote_device.device_id);
        println!("  Fingerprint: {}", remote_device.network_fingerprint);
        println!();
        
        let confirmed = Confirm::new()
            .with_prompt("Do you want to pair with this device?")
            .default(false)
            .interact()
            .map_err(|e| NetworkError::AuthenticationFailed(format!("UI interaction failed: {}", e)))?;
        
        if confirmed {
            println!("✅ Pairing confirmed by user");
        } else {
            println!("❌ Pairing rejected by user");
        }
        
        Ok(confirmed)
    }
    
    async fn show_pairing_progress(&self, state: PairingState) {
        use colored::*;
        
        let (status, message) = match state {
            PairingState::Idle => ("⏸️", "Waiting to start pairing"),
            PairingState::GeneratingCode => ("🔄", "Generating pairing code..."),
            PairingState::Broadcasting => ("📡", "Broadcasting pairing availability"),
            PairingState::Scanning => ("🔍", "Scanning for devices to pair with"),
            PairingState::Connecting => ("🔗", "Establishing secure connection"),
            PairingState::Authenticating => ("🔐", "Authenticating pairing code"),
            PairingState::ExchangingKeys => ("🔑", "Exchanging device information"),
            PairingState::AwaitingConfirmation => ("⏳", "Waiting for user confirmation"),
            PairingState::EstablishingSession => ("🛡️", "Establishing session keys"),
            PairingState::Completed => ("✅", "Pairing completed successfully"),
            PairingState::Failed(ref error) => ("❌", error.as_str()),
        };
        
        println!("{} {}", status, message.bright_white());
    }
    
    async fn show_pairing_error(&self, error: &NetworkError) {
        use colored::*;
        
        println!("{} Pairing failed: {}", "❌".red(), error.to_string().red());
    }
    
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        use colored::*;
        
        println!("\n🔑 Your Pairing Code");
        println!("==================");
        println!();
        println!("Share this code with the other device:");
        println!();
        println!("    {}", code.bright_cyan().bold());
        println!();
        println!("⏰ This code expires in {} seconds", expires_in_seconds.to_string().yellow());
        println!();
        println!("💡 The other device should enter these words to pair with you.");
    }
    
    async fn prompt_pairing_code(&self) -> Result<[String; 12]> {
        use dialoguer::Input;
        use colored::*;
        
        println!("\n🔑 Enter Pairing Code");
        println!("====================");
        println!("Enter the 12-word pairing code from the other device:");
        println!();
        
        let mut words = Vec::new();
        
        for i in 1..=12 {
            let word: String = Input::new()
                .with_prompt(&format!("Word {}/12", i))
                .interact()
                .map_err(|e| NetworkError::AuthenticationFailed(format!("Input failed: {}", e)))?;
            
            words.push(word.trim().to_lowercase());
        }
        
        // Validate we have exactly 12 words
        if words.len() != 12 {
            return Err(NetworkError::AuthenticationFailed(
                "Must provide exactly 12 words".to_string()
            ));
        }
        
        println!();
        println!("Entered code: {}", words.join(" ").bright_cyan());
        
        Ok([
            words[0].clone(),
            words[1].clone(),
            words[2].clone(),
            words[3].clone(),
            words[4].clone(),
            words[5].clone(),
            words[6].clone(),
            words[7].clone(),
            words[8].clone(),
            words[9].clone(),
            words[10].clone(),
            words[11].clone(),
        ])
    }
}

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