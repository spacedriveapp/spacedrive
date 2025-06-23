//! Device management module
//! 
//! Handles persistent device identification across Spacedrive installations

mod config;
mod manager;
mod master_key;

pub use config::DeviceConfig;
pub use manager::{DeviceManager, DeviceError};
pub use master_key::{MasterKeyManager, MasterKeyError};

// Re-export domain types
pub use crate::domain::device::{Device, OperatingSystem};