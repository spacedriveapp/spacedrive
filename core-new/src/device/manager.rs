//! Device manager for handling device lifecycle

use super::config::DeviceConfig;
use crate::domain::device::{Device, OperatingSystem};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during device management
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not initialized")]
    NotInitialized,
    
    #[error("Config path not found")]
    ConfigPathNotFound,
    
    #[error("Unsupported platform")]
    UnsupportedPlatform,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Manages the current device state
pub struct DeviceManager {
    /// Current device configuration
    config: Arc<RwLock<DeviceConfig>>,
}

impl DeviceManager {
    /// Initialize the device manager
    /// 
    /// This will either load existing device configuration or create a new one
    pub fn init() -> Result<Self, DeviceError> {
        let config = match DeviceConfig::load() {
            Ok(config) => config,
            Err(DeviceError::NotInitialized) => {
                // Create new device configuration
                let os = detect_os();
                let name = get_device_name();
                let mut config = DeviceConfig::new(name, os);
                
                // Try to detect hardware model
                config.hardware_model = detect_hardware_model();
                
                // Save the new configuration
                config.save()?;
                config
            }
            Err(e) => return Err(e),
        };
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }
    
    /// Initialize the device manager with a custom data directory
    pub fn init_with_path(data_dir: &PathBuf) -> Result<Self, DeviceError> {
        let config = match DeviceConfig::load_from(data_dir) {
            Ok(config) => config,
            Err(DeviceError::NotInitialized) => {
                // Create new device configuration
                let os = detect_os();
                let name = get_device_name();
                let mut config = DeviceConfig::new(name, os);
                
                // Try to detect hardware model
                config.hardware_model = detect_hardware_model();
                
                // Save the new configuration
                config.save_to(data_dir)?;
                config
            }
            Err(e) => return Err(e),
        };
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }
    
    /// Get the current device ID
    pub fn device_id(&self) -> Result<Uuid, DeviceError> {
        self.config
            .read()
            .map(|c| c.id)
            .map_err(|_| DeviceError::LockPoisoned)
    }
    
    /// Get the current device configuration
    pub fn config(&self) -> Result<DeviceConfig, DeviceError> {
        self.config
            .read()
            .map(|c| c.clone())
            .map_err(|_| DeviceError::LockPoisoned)
    }
    
    /// Create a Device domain object from current configuration
    pub fn to_device(&self) -> Result<Device, DeviceError> {
        let config = self.config()?;
        
        // Create device with loaded configuration
        let mut device = Device::new(config.name.clone());
        device.id = config.id;
        device.os = parse_os(&config.os);
        device.hardware_model = config.hardware_model.clone();
        device.created_at = config.created_at;
        
        Ok(device)
    }
    
    /// Update device name
    pub fn set_name(&self, name: String) -> Result<(), DeviceError> {
        let mut config = self.config
            .write()
            .map_err(|_| DeviceError::LockPoisoned)?;
        
        config.name = name;
        config.save()?;
        
        Ok(())
    }
}

/// Get the device name from the system
fn get_device_name() -> String {
    whoami::devicename()
}

/// Detect the operating system
fn detect_os() -> String {
    if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else if cfg!(target_os = "ios") {
        "iOS".to_string()
    } else if cfg!(target_os = "android") {
        "Android".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Parse OS string back to enum
fn parse_os(os: &str) -> OperatingSystem {
    match os {
        "macOS" => OperatingSystem::MacOS,
        "Windows" => OperatingSystem::Windows,
        "Linux" => OperatingSystem::Linux,
        "iOS" => OperatingSystem::IOs,
        "Android" => OperatingSystem::Android,
        _ => OperatingSystem::Other,
    }
}

/// Try to detect hardware model
fn detect_hardware_model() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // Try to get model from system_profiler
        use std::process::Command;
        
        let output = Command::new("system_profiler")
            .args(&["SPHardwareDataType", "-json"])
            .output()
            .ok()?;
        
        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            // Simple extraction - in production would use proper JSON parsing
            if let Some(start) = json_str.find("\"machine_model\":") {
                let substr = &json_str[start + 17..];
                if let Some(end) = substr.find('"') {
                    return Some(substr[..end].to_string());
                }
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_device_config() {
        let config = DeviceConfig::new("Test Device".to_string(), "Linux".to_string());
        assert_eq!(config.name, "Test Device");
        assert_eq!(config.os, "Linux");
        assert!(config.hardware_model.is_none());
    }
}