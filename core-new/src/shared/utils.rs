//! Shared utility functions

use std::sync::RwLock;
use uuid::Uuid;

/// Global reference to current device ID
/// This is set during Core initialization
pub static CURRENT_DEVICE_ID: once_cell::sync::Lazy<RwLock<Uuid>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Uuid::nil()));

/// Initialize the current device ID
pub fn set_current_device_id(id: Uuid) {
    if let Ok(mut device_id) = CURRENT_DEVICE_ID.write() {
        *device_id = id;
    }
}

/// Get the current device ID
pub fn get_current_device_id() -> Uuid {
    match CURRENT_DEVICE_ID.read() {
        Ok(guard) => *guard,
        Err(_) => Uuid::nil(),
    }
}