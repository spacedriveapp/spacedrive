//! Device management module
//!
//! Handles persistent device identification across Spacedrive installations

mod config;
mod id;
mod manager;

pub use crate::crypto::device_key_manager::{DeviceKeyError, DeviceKeyManager};
pub use config::DeviceConfig;
pub use id::{get_current_device_id, set_current_device_id, CURRENT_DEVICE_ID};
pub use manager::{DeviceError, DeviceManager};

// Re-export domain types
pub use crate::domain::device::{Device, OperatingSystem};
