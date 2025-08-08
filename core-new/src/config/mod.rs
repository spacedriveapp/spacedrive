//! Application configuration management

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use anyhow::{anyhow, Result};
use std::fs;

pub mod app_config;
pub mod migration;

pub use app_config::{AppConfig, JobLoggingConfig};
pub use migration::Migrate;

/// Platform-specific data directory resolution
pub fn default_data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow!("Could not determine data directory"))?
        .join("spacedrive");
    
    #[cfg(target_os = "windows")]
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow!("Could not determine data directory"))?
        .join("Spacedrive");
    
    #[cfg(target_os = "linux")]
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("Could not determine data directory"))?
        .join("spacedrive");
    
    // Create directory if it doesn't exist
    fs::create_dir_all(&dir)?;
    
    Ok(dir)
}

/// P2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    pub enabled: bool,
    pub discovery: String, // "local", "global", "disabled"
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            discovery: "local".to_string(),
        }
    }
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: String,     // "light", "dark", "system"
    pub language: String,  // ISO 639-1 code
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "en".to_string(),
        }
    }
}