//! Device management module
//! 
//! Handles persistent device identification across Spacedrive installations

mod config;
mod manager;
pub use config::DeviceConfig;
pub use manager::{DeviceManager, DeviceError};
pub use crate::keys::device_key_manager::{DeviceKeyManager, DeviceKeyError};

// Re-export domain types
pub use crate::domain::device::{Device, OperatingSystem};