//! Unified device model - no more node/device/instance confusion
//! 
//! A Device represents a machine running Spacedrive. This unifies the old
//! concepts of Node, Device, and Instance into one clear model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    
    /// Sync leadership status per library
    pub sync_leadership: HashMap<Uuid, SyncRole>,
    
    /// Last time this device was seen
    pub last_seen_at: DateTime<Utc>,
    
    /// When this device was first added
    pub created_at: DateTime<Utc>,
    
    /// When this device info was last updated
    pub updated_at: DateTime<Utc>,
}

/// Sync role for a device in a specific library
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SyncRole {
    /// This device maintains the sync log for the library
    Leader,
    
    /// This device syncs from the leader
    Follower,
    
    /// This device doesn't participate in sync for this library
    Inactive,
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
            sync_leadership: HashMap::new(),
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
    
    /// Set sync role for a library
    pub fn set_sync_role(&mut self, library_id: Uuid, role: SyncRole) {
        self.sync_leadership.insert(library_id, role);
        self.updated_at = Utc::now();
    }
    
    /// Get sync role for a library
    pub fn sync_role(&self, library_id: &Uuid) -> SyncRole {
        self.sync_leadership.get(library_id).copied().unwrap_or(SyncRole::Inactive)
    }
    
    /// Check if this device is the sync leader for a library
    pub fn is_sync_leader(&self, library_id: &Uuid) -> bool {
        matches!(self.sync_role(library_id), SyncRole::Leader)
    }
    
    /// Get all libraries where this device is the leader
    pub fn leader_libraries(&self) -> Vec<Uuid> {
        self.sync_leadership
            .iter()
            .filter_map(|(lib_id, role)| {
                if *role == SyncRole::Leader {
                    Some(*lib_id)
                } else {
                    None
                }
            })
            .collect()
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

// Conversion implementations for database entities
use crate::infrastructure::database::entities;
use sea_orm::ActiveValue;

impl From<Device> for entities::device::ActiveModel {
    fn from(device: Device) -> Self {
        use sea_orm::ActiveValue::*;
        
        entities::device::ActiveModel {
            id: NotSet, // Auto-increment
            uuid: Set(device.id),
            name: Set(device.name),
            os: Set(device.os.to_string()),
            os_version: Set(None), // TODO: Add to domain model if needed
            hardware_model: Set(device.hardware_model),
            network_addresses: Set(serde_json::json!(device.network_addresses)),
            is_online: Set(device.is_online),
            last_seen_at: Set(device.last_seen_at),
            capabilities: Set(serde_json::json!({
                "indexing": true,
                "p2p": true,
                "volume_detection": true
            })),
            sync_leadership: Set(serde_json::json!(device.sync_leadership)),
            created_at: Set(device.created_at),
            updated_at: Set(device.updated_at),
        }
    }
}

impl TryFrom<entities::device::Model> for Device {
    type Error = serde_json::Error;
    
    fn try_from(model: entities::device::Model) -> Result<Self, Self::Error> {
        let network_addresses: Vec<String> = serde_json::from_value(model.network_addresses)?;
        let sync_leadership: HashMap<Uuid, SyncRole> = serde_json::from_value(model.sync_leadership)?;
        
        Ok(Device {
            id: model.uuid,
            name: model.name,
            os: parse_operating_system(&model.os),
            hardware_model: model.hardware_model,
            network_addresses,
            is_online: model.is_online,
            sync_leadership,
            last_seen_at: model.last_seen_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// Parse OS string to enum
fn parse_operating_system(os_str: &str) -> OperatingSystem {
    match os_str {
        "macOS" => OperatingSystem::MacOS,
        "Windows" => OperatingSystem::Windows,
        "Linux" => OperatingSystem::Linux,
        "iOS" => OperatingSystem::IOs,
        "Android" => OperatingSystem::Android,
        _ => OperatingSystem::Other,
    }
}