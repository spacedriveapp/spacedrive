//! Unified device model - no more node/device/instance confusion
//! 
//! A Device represents a machine running Spacedrive. This unifies the old
//! concepts of Node, Device, and Instance into one clear model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A device running Spacedrive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Unique identifier for this device
    pub id: Uuid,
    
    /// Human-readable name
    pub name: String,
    
    /// Operating system
    pub os: OperatingSystem,
    
    /// Hardware model (e.g., "MacBook Pro", "iPhone 15")
    pub hardware_model: Option<String>,
    
    /// Network addresses for P2P connections
    pub network_addresses: Vec<String>,
    
    /// Whether this device is currently online
    pub is_online: bool,
    
    /// Last time this device was seen
    pub last_seen_at: DateTime<Utc>,
    
    /// When this device was first added
    pub created_at: DateTime<Utc>,
    
    /// When this device info was last updated
    pub updated_at: DateTime<Utc>,
}

/// Operating system types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OperatingSystem {
    MacOS,
    Windows,
    Linux,
    IOs,
    Android,
    Other,
}

impl Device {
    /// Create a new device
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            os: detect_operating_system(),
            hardware_model: detect_hardware_model(),
            network_addresses: Vec::new(),
            is_online: true,
            last_seen_at: now,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Create the current device
    pub fn current() -> Self {
        Self::new(get_device_name())
    }
    
    /// Update network addresses
    pub fn update_network_addresses(&mut self, addresses: Vec<String>) {
        self.network_addresses = addresses;
        self.updated_at = Utc::now();
    }
    
    /// Mark device as online
    pub fn mark_online(&mut self) {
        self.is_online = true;
        self.last_seen_at = Utc::now();
        self.updated_at = Utc::now();
    }
    
    /// Mark device as offline
    pub fn mark_offline(&mut self) {
        self.is_online = false;
        self.updated_at = Utc::now();
    }
    
    /// Check if this is the current device
    pub fn is_current(&self) -> bool {
        self.id == crate::shared::types::get_current_device_id()
    }
}

/// Get the device name from the system
fn get_device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        return whoami::devicename();
    }
    
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        if let Ok(name) = hostname::get() {
            if let Ok(name_str) = name.into_string() {
                return name_str;
            }
        }
    }
    
    "Unknown Device".to_string()
}

/// Detect the operating system
fn detect_operating_system() -> OperatingSystem {
    #[cfg(target_os = "macos")]
    return OperatingSystem::MacOS;
    
    #[cfg(target_os = "windows")]
    return OperatingSystem::Windows;
    
    #[cfg(target_os = "linux")]
    return OperatingSystem::Linux;
    
    #[cfg(target_os = "ios")]
    return OperatingSystem::IOs;
    
    #[cfg(target_os = "android")]
    return OperatingSystem::Android;
    
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "ios",
        target_os = "android"
    )))]
    return OperatingSystem::Other;
}

/// Get hardware model information
fn detect_hardware_model() -> Option<String> {
    // This would use platform-specific APIs
    // For now, return None
    None
}

impl std::fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperatingSystem::MacOS => write!(f, "macOS"),
            OperatingSystem::Windows => write!(f, "Windows"),
            OperatingSystem::Linux => write!(f, "Linux"),
            OperatingSystem::IOs => write!(f, "iOS"),
            OperatingSystem::Android => write!(f, "Android"),
            OperatingSystem::Other => write!(f, "Other"),
        }
    }
}