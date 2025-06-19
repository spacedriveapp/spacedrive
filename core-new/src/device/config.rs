//! Device configuration persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Device configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Unique device identifier
    pub id: Uuid,
    
    /// User-friendly device name
    pub name: String,
    
    /// When this device was first initialized
    pub created_at: DateTime<Utc>,
    
    /// Hardware model (if detectable)
    pub hardware_model: Option<String>,
    
    /// Operating system
    pub os: String,
    
    /// Spacedrive version that created this config
    pub version: String,
}

impl DeviceConfig {
    /// Create a new device configuration
    pub fn new(name: String, os: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            created_at: Utc::now(),
            hardware_model: None,
            os,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
    
    /// Get the configuration file path for the current platform
    pub fn config_path() -> Result<PathBuf, super::DeviceError> {
        let base_path = if cfg!(target_os = "macos") {
            dirs::data_dir()
                .ok_or(super::DeviceError::ConfigPathNotFound)?
                .join("com.spacedrive")
        } else if cfg!(target_os = "linux") {
            dirs::config_dir()
                .ok_or(super::DeviceError::ConfigPathNotFound)?
                .join("spacedrive")
        } else if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or(super::DeviceError::ConfigPathNotFound)?
                .join("Spacedrive")
        } else {
            return Err(super::DeviceError::UnsupportedPlatform);
        };
        
        Ok(base_path.join("device.json"))
    }
    
    /// Load configuration from disk
    pub fn load() -> Result<Self, super::DeviceError> {
        let path = Self::config_path()?;
        
        if !path.exists() {
            return Err(super::DeviceError::NotInitialized);
        }
        
        let content = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&content)?;
        
        Ok(config)
    }
    
    /// Save configuration to disk
    pub fn save(&self) -> Result<(), super::DeviceError> {
        let path = Self::config_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        
        Ok(())
    }
    
    /// Load configuration from a specific directory
    pub fn load_from(data_dir: &PathBuf) -> Result<Self, super::DeviceError> {
        let path = data_dir.join("device.json");
        
        if !path.exists() {
            return Err(super::DeviceError::NotInitialized);
        }
        
        let content = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&content)?;
        
        Ok(config)
    }
    
    /// Save configuration to a specific directory
    pub fn save_to(&self, data_dir: &PathBuf) -> Result<(), super::DeviceError> {
        // Ensure directory exists
        std::fs::create_dir_all(data_dir)?;
        
        let path = data_dir.join("device.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        
        Ok(())
    }
}