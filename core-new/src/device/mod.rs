//! Device management module
//! 
//! Handles persistent device identification across Spacedrive installations

mod config;
mod manager;

pub use config::DeviceConfig;
pub use manager::{DeviceManager, DeviceError};

// Re-export domain types
pub use crate::domain::device::{Device, OperatingSystem};