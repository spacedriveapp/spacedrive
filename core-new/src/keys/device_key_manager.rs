//! Master encryption key management using OS secure storage

use keyring::{Entry, Error as KeyringError};
use rand::{thread_rng, Rng};
use thiserror::Error;
use uuid::Uuid;

const KEYRING_SERVICE: &str = "Spacedrive";
const DEVICE_KEY_USERNAME: &str = "master_encryption_key";
const MASTER_KEY_LENGTH: usize = 32; // 256 bits

#[derive(Error, Debug)]
pub enum DeviceKeyError {
    #[error("Keyring error: {0}")]
    Keyring(#[from] KeyringError),
    
    #[error("Invalid key format")]
    InvalidKeyFormat,
    
    #[error("Key generation failed")]
    KeyGenerationFailed,
}

pub struct DeviceKeyManager {
    entry: Entry,
}

impl DeviceKeyManager {
    pub fn new() -> Result<Self, DeviceKeyError> {
        let entry = Entry::new(KEYRING_SERVICE, DEVICE_KEY_USERNAME)?;
        Ok(Self { entry })
    }

    #[cfg(test)]
    pub fn new_for_test(service: &str, username: &str) -> Result<Self, DeviceKeyError> {
        let entry = Entry::new(service, username)?;
        Ok(Self { entry })
    }

    pub fn get_or_create_master_key(&self) -> Result<[u8; MASTER_KEY_LENGTH], DeviceKeyError> {
        match self.entry.get_password() {
            Ok(key_hex) => {
                let key_bytes = hex::decode(key_hex)
                    .map_err(|_| DeviceKeyError::InvalidKeyFormat)?;
                
                if key_bytes.len() != MASTER_KEY_LENGTH {
                    return Err(DeviceKeyError::InvalidKeyFormat);
                }
                
                let mut key = [0u8; MASTER_KEY_LENGTH];
                key.copy_from_slice(&key_bytes);
                Ok(key)
            }
            Err(KeyringError::NoEntry) => {
                let key = self.generate_new_master_key()?;
                let key_hex = hex::encode(key);
                self.entry.set_password(&key_hex)?;
                Ok(key)
            }
            Err(e) => Err(DeviceKeyError::Keyring(e)),
        }
    }

    pub fn get_master_key(&self) -> Result<[u8; MASTER_KEY_LENGTH], DeviceKeyError> {
        let key_hex = self.entry.get_password()?;
        let key_bytes = hex::decode(key_hex)
            .map_err(|_| DeviceKeyError::InvalidKeyFormat)?;
        
        if key_bytes.len() != MASTER_KEY_LENGTH {
            return Err(DeviceKeyError::InvalidKeyFormat);
        }
        
        let mut key = [0u8; MASTER_KEY_LENGTH];
        key.copy_from_slice(&key_bytes);
        Ok(key)
    }

    pub fn get_master_key_hex(&self) -> Result<String, DeviceKeyError> {
        let key = self.get_master_key()?;
        Ok(hex::encode(key))
    }

    fn generate_new_master_key(&self) -> Result<[u8; MASTER_KEY_LENGTH], DeviceKeyError> {
        let mut key = [0u8; MASTER_KEY_LENGTH];
        thread_rng().fill(&mut key);
        Ok(key)
    }

    pub fn regenerate_master_key(&self) -> Result<[u8; MASTER_KEY_LENGTH], DeviceKeyError> {
        let key = self.generate_new_master_key()?;
        let key_hex = hex::encode(key);
        self.entry.set_password(&key_hex)?;
        Ok(key)
    }

    pub fn delete_master_key(&self) -> Result<(), DeviceKeyError> {
        self.entry.delete_credential()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring::Entry;

    const TEST_SERVICE: &str = "SpacedriveTest";
    const TEST_USERNAME: &str = "test_master_key";

    fn create_test_manager() -> DeviceKeyManager {
        let entry = Entry::new(TEST_SERVICE, TEST_USERNAME).unwrap();
        DeviceKeyManager { entry }
    }

    fn cleanup_test_key() {
        let entry = Entry::new(TEST_SERVICE, TEST_USERNAME).unwrap();
        let _ = entry.delete_credential();
    }

    #[test]
    fn test_generate_and_retrieve_master_key() {
        cleanup_test_key();
        let manager = create_test_manager();

        let key1 = manager.get_or_create_master_key().unwrap();
        let key2 = manager.get_master_key().unwrap();

        assert_eq!(key1, key2);
        assert_eq!(key1.len(), MASTER_KEY_LENGTH);

        cleanup_test_key();
    }

    #[test]
    fn test_master_key_persistence() {
        cleanup_test_key();
        let manager1 = create_test_manager();
        let key1 = manager1.get_or_create_master_key().unwrap();

        let manager2 = create_test_manager();
        let key2 = manager2.get_master_key().unwrap();

        assert_eq!(key1, key2);

        cleanup_test_key();
    }

    #[test]
    fn test_regenerate_master_key() {
        cleanup_test_key();
        let manager = create_test_manager();

        let key1 = manager.get_or_create_master_key().unwrap();
        let key2 = manager.regenerate_master_key().unwrap();

        assert_ne!(key1, key2);
        assert_eq!(key2.len(), MASTER_KEY_LENGTH);

        let key3 = manager.get_master_key().unwrap();
        assert_eq!(key2, key3);

        cleanup_test_key();
    }

    #[test]
    fn test_hex_representation() {
        cleanup_test_key();
        let manager = create_test_manager();

        let key = manager.get_or_create_master_key().unwrap();
        let hex_key = manager.get_master_key_hex().unwrap();

        assert_eq!(hex_key.len(), MASTER_KEY_LENGTH * 2);
        assert_eq!(hex::decode(&hex_key).unwrap(), key);

        cleanup_test_key();
    }
}