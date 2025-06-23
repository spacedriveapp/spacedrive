//! CLI-specific pairing user interface implementations
//!
//! This module contains CLI-specific implementations for pairing interactions,
//! providing console-based interactions for device pairing.

use async_trait::async_trait;
use crate::networking::{DeviceInfo, NetworkingError as NetworkError, Result};
use crate::networking::PairingState;

/// CLI-specific pairing user interface trait
#[async_trait]
pub trait PairingUserInterface: Send + Sync {
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool>;
    async fn show_pairing_progress(&self, state: PairingState);
    async fn show_pairing_error(&self, error: &NetworkError);
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32);
    async fn prompt_pairing_code(&self) -> Result<[String; 12]>;
}

/// Console-based pairing UI for CLI applications
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
            PairingState::WaitingForConnection => ("⏳", "Waiting for connection"),
            PairingState::Connecting => ("🔗", "Establishing secure connection"),
            PairingState::Authenticating => ("🔐", "Authenticating pairing code"),
            PairingState::ExchangingKeys => ("🔑", "Exchanging device information"),
            PairingState::AwaitingConfirmation => ("⏳", "Waiting for user confirmation"),
            PairingState::EstablishingSession => ("🛡️", "Establishing session keys"),
            PairingState::ChallengeReceived { .. } => ("🔐", "Processing challenge"),
            PairingState::ResponseSent => ("📤", "Response sent"),
            PairingState::Completed => ("✅", "Pairing completed successfully"),
            PairingState::Failed { ref reason } => ("❌", reason.as_str()),
            PairingState::ResponsePending { .. } => ("🔄", "Preparing challenge response"),
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

/// Simple pairing UI for daemon mode with configurable auto-accept
pub struct SimplePairingUI {
    auto_accept: bool,
    code_sender: Option<tokio::sync::oneshot::Sender<(String, u32)>>,
}

impl SimplePairingUI {
    pub fn new(auto_accept: bool) -> Self {
        Self {
            auto_accept,
            code_sender: None,
        }
    }
    
    pub fn with_code_sender(mut self, sender: tokio::sync::oneshot::Sender<(String, u32)>) -> Self {
        self.code_sender = Some(sender);
        self
    }
}

#[async_trait]
impl PairingUserInterface for SimplePairingUI {
    async fn show_pairing_error(&self, error: &NetworkError) {
        tracing::error!("Pairing error: {}", error);
    }

    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
        tracing::info!("Pairing code generated: {} (expires in {} seconds)", code, expires_in_seconds);
        
        // Send the code back to the waiting CLI
        if let Some(sender) = &self.code_sender {
            // We can't move out of self, so we'll log here and let the pairing method handle it differently
            // This is a limitation of the current UI interface design
        }
    }

    async fn prompt_pairing_code(&self) -> Result<[String; 12]> {
        // This should not be called in the CLI daemon context
        Err(NetworkError::AuthenticationFailed(
            "Interactive pairing code input not supported in daemon mode".to_string(),
        ))
    }

    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool> {
        if self.auto_accept {
            tracing::info!("Auto-accepting pairing with device: {}", remote_device.device_name);
            Ok(true)
        } else {
            tracing::info!("Pairing request from device: {} (manual confirmation required)", remote_device.device_name);
            // In daemon mode, we'll store the request and let the user decide via CLI
            Ok(false)
        }
    }

    async fn show_pairing_progress(&self, state: PairingState) {
        match state {
            PairingState::GeneratingCode => tracing::info!("Generating pairing code..."),
            PairingState::Broadcasting => tracing::info!("Broadcasting on DHT..."),
            PairingState::Scanning => tracing::info!("Scanning DHT for devices..."),
            PairingState::WaitingForConnection => tracing::info!("Waiting for connection..."),
            PairingState::Connecting => tracing::info!("Establishing connection..."),
            PairingState::Authenticating => tracing::info!("Authenticating..."),
            PairingState::ExchangingKeys => tracing::info!("Exchanging keys..."),
            PairingState::AwaitingConfirmation => tracing::info!("Awaiting confirmation..."),
            PairingState::EstablishingSession => tracing::info!("Establishing session..."),
            PairingState::ChallengeReceived { .. } => tracing::info!("Processing challenge..."),
            PairingState::ResponseSent => tracing::info!("Response sent..."),
            PairingState::Completed => tracing::info!("Pairing completed!"),
            PairingState::Failed { reason } => tracing::error!("Pairing failed: {}", reason),
            _ => {}
        }
    }
}

/// CLI-specific network logger that uses tracing
pub struct CliNetworkLogger;

#[async_trait]
impl crate::networking::NetworkLogger for CliNetworkLogger {
    async fn info(&self, message: &str) {
        tracing::info!("{}", message);
    }
    
    async fn error(&self, message: &str) {
        tracing::error!("{}", message);
    }
    
    async fn debug(&self, message: &str) {
        tracing::debug!("{}", message);
    }
    
    async fn warn(&self, message: &str) {
        tracing::warn!("{}", message);
    }
}