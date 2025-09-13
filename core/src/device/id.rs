//! Global Device ID Management
//!
//! This module manages the global device ID that is accessible throughout the application.
//! The device ID is stored globally for performance and convenience reasons.
//!
//! ## Why Global?
//! - **Performance**: Device ID is accessed frequently across the codebase. Global access
//!   avoids the overhead of Arc/RwLock on every access.
//! - **Convenience**: No need to pass CoreContext everywhere just to get the device ID.
//! - **Simplicity**: Device ID is immutable once set and doesn't need complex lifecycle management.
//! - **Thread Safety**: Works seamlessly in both sync and async contexts.
//!
//! ## Why Not Part of DeviceManager?
//! While the DeviceManager handles device configuration and lifecycle, the global device ID
//! serves a different purpose - it's a runtime cache for quick access. The DeviceManager
//! deals with device state (keys, config, etc.) while this is just the device identifier.
//!
//! ## Architectural Trade-offs
//! **Pros:**
//! - Fast access without context passing
//! - Simple API (`get_current_device_id()`)
//! - No error handling needed
//! - Works anywhere in the codebase
//!
//! **Cons:**
//! - Not "pure" dependency injection
//! - Global mutable state (though read-only after initialization)
//! - Harder to test in isolation
//!
//! ## Module Organization
//! Originally in `common/utils.rs`, moved to `device/id.rs` for better module organization.
//! Device ID management belongs with other device-related code, even though it's
//! implemented as global functions rather than DeviceManager methods.
//!
//! ## Usage Pattern
//! ```rust
//! // Set once during initialization
//! set_current_device_id(device_manager.device_id()?);
//!
//! // Use anywhere in the codebase
//! let device_id = get_current_device_id();
//! ```

use once_cell::sync::Lazy;
use std::sync::RwLock;
use uuid::Uuid;

/// Global reference to current device ID
/// This is set during Core initialization
pub static CURRENT_DEVICE_ID: Lazy<RwLock<Uuid>> = Lazy::new(|| RwLock::new(Uuid::nil()));

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_device_id_management() {
		let test_id = Uuid::new_v4();

		// Initially should be nil
		assert!(get_current_device_id().is_nil());

		// Set device ID
		set_current_device_id(test_id);

		// Should return the set ID
		assert_eq!(get_current_device_id(), test_id);

		// Reset for other tests
		set_current_device_id(Uuid::nil());
		assert!(get_current_device_id().is_nil());
	}
}
