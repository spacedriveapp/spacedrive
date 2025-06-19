//! Application configuration

use super::{P2PConfig, Preferences, default_data_dir};
use crate::config::migration::Migrate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use std::fs;
use tracing::{info, warn};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Config schema version
    pub version: u32,
    
    /// Data directory path
    pub data_dir: PathBuf,
    
    /// Logging level
    pub log_level: String,
    
    /// Whether telemetry is enabled
    pub telemetry_enabled: bool,
    
    /// P2P configuration
    pub p2p: P2PConfig,
    
    /// User preferences
    pub preferences: Preferences,
}

impl AppConfig {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let data_dir = default_data_dir()?;
        Self::load_from(&data_dir)
    }
    
    /// Load configuration from a specific data directory
    pub fn load_from(data_dir: &PathBuf) -> Result<Self> {
        let config_path = data_dir.join("spacedrive.json");
        
        if config_path.exists() {
            info!("Loading config from {:?}", config_path);
            let json = fs::read_to_string(&config_path)?;
            let mut config: AppConfig = serde_json::from_str(&json)?;
            
            // Apply migrations if needed
            if config.version < Self::target_version() {
                info!("Migrating config from v{} to v{}", config.version, Self::target_version());
                config.migrate()?;
                config.save()?;
            }
            
            Ok(config)
        } else {
            warn!("No config found, creating default at {:?}", config_path);
            let config = Self::default_with_dir(data_dir.clone());
            config.save()?;
            Ok(config)
        }
    }
    
    /// Load or create configuration
    pub fn load_or_create(data_dir: &PathBuf) -> Result<Self> {
        Self::load_from(data_dir).or_else(|_| {
            let config = Self::default_with_dir(data_dir.clone());
            config.save()?;
            Ok(config)
        })
    }
    
    /// Create default configuration with specific data directory
    pub fn default_with_dir(data_dir: PathBuf) -> Self {
        Self {
            version: Self::target_version(),
            data_dir,
            log_level: "info".to_string(),
            telemetry_enabled: true,
            p2p: P2PConfig::default(),
            preferences: Preferences::default(),
        }
    }
    
    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        // Ensure directory exists
        fs::create_dir_all(&self.data_dir)?;
        
        let config_path = self.data_dir.join("spacedrive.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, json)?;
        info!("Saved config to {:?}", config_path);
        Ok(())
    }
    
    /// Get the path for logs directory
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
    
    /// Get the path for libraries directory
    pub fn libraries_dir(&self) -> PathBuf {
        self.data_dir.join("libraries")
    }
    
    /// Ensure all required directories exist
    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.libraries_dir())?;
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::default_with_dir(data_dir)
    }
}

impl Migrate for AppConfig {
    fn current_version(&self) -> u32 {
        self.version
    }
    
    fn target_version() -> u32 {
        1 // Current schema version
    }
    
    fn migrate(&mut self) -> Result<()> {
        match self.version {
            0 => {
                // Future migration from v0 to v1 would go here
                self.version = 1;
                Ok(())
            }
            1 => Ok(()), // Already at target version
            v => Err(anyhow!("Unknown config version: {}", v)),
        }
    }
}