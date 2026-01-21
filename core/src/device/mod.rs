//! Device management module
//!
//! Handles persistent device identification across Spacedrive installations

pub mod config;
mod id;
mod manager;

pub use config::DeviceConfig;
pub use id::{
	get_current_device_id, get_current_device_slug, set_current_device_id, set_current_device_slug,
	CURRENT_DEVICE_ID, CURRENT_DEVICE_SLUG,
};
pub use manager::{DeviceError, DeviceManager};

// Re-export domain types
pub use crate::domain::device::{Device, OperatingSystem};
