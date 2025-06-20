//! Common test utilities for networking module

#[cfg(test)]
pub mod test_helpers {
    use crate::networking::{DeviceInfo, identity::PublicKey};
    use uuid::Uuid;

    /// Create a test device info for unit tests
    pub fn create_test_device_info() -> DeviceInfo {
        DeviceInfo::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
        )
    }

    /// Create a test device info with custom name
    pub fn create_test_device_info_with_name(name: &str) -> DeviceInfo {
        DeviceInfo::new(
            Uuid::new_v4(),
            name.to_string(),
            PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
        )
    }
}