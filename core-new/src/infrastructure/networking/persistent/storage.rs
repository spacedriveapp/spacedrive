//! Encrypted storage utilities for persistent device connections
//!
//! Provides secure storage of device relationships, session keys, and connection metadata
//! using industry-standard encryption with password-derived keys.

use chrono::{DateTime, Utc};
use ring::{aead, pbkdf2, rand::{SystemRandom, SecureRandom}};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::networking::{NetworkError, Result};

/// Number of PBKDF2 iterations for key derivation
const PBKDF2_ITERATIONS: u32 = 100_000;

/// Salt length for key derivation
const SALT_LENGTH: usize = 32;

/// Nonce length for AES-256-GCM
const NONCE_LENGTH: usize = 12;

/// AES-256-GCM key length
const KEY_LENGTH: usize = 32;

/// Encrypted data container with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Encrypted payload
    pub ciphertext: Vec<u8>,
    /// Salt for key derivation
    pub salt: [u8; SALT_LENGTH],
    /// Nonce for encryption
    pub nonce: [u8; NONCE_LENGTH],
    /// When this data was encrypted
    pub encrypted_at: DateTime<Utc>,
    /// Version for future compatibility
    pub version: u32,
}

/// Secure storage for encrypted data with atomic operations
pub struct SecureStorage {
    /// Base directory for storage
    base_path: PathBuf,
}

impl SecureStorage {
    /// Create new secure storage at the given path
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Encrypt data with password-derived key
    pub fn encrypt_data(&self, data: &[u8], password: &str) -> Result<EncryptedData> {
        let rng = SystemRandom::new();
        
        // Generate salt and nonce
        let mut salt = [0u8; SALT_LENGTH];
        let mut nonce = [0u8; NONCE_LENGTH];
        
        rng.fill(&mut salt)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate salt: {:?}", e)))?;
        rng.fill(&mut nonce)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate nonce: {:?}", e)))?;

        // Derive key from password
        let mut key = [0u8; KEY_LENGTH];
        let iterations = NonZeroU32::new(PBKDF2_ITERATIONS)
            .ok_or_else(|| NetworkError::EncryptionError("Invalid iteration count".to_string()))?;
        
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &salt,
            password.as_bytes(),
            &mut key,
        );

        // Encrypt data
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create key: {:?}", e)))?;
        let sealing_key = aead::LessSafeKey::new(unbound_key);

        let mut ciphertext = data.to_vec();
        sealing_key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::empty(),
                &mut ciphertext,
            )
            .map_err(|e| NetworkError::EncryptionError(format!("Encryption failed: {:?}", e)))?;

        Ok(EncryptedData {
            ciphertext,
            salt,
            nonce,
            encrypted_at: Utc::now(),
            version: 1,
        })
    }

    /// Decrypt data with password-derived key
    pub fn decrypt_data(&self, encrypted: &EncryptedData, password: &str) -> Result<Vec<u8>> {
        // Derive key from password
        let mut key = [0u8; KEY_LENGTH];
        let iterations = NonZeroU32::new(PBKDF2_ITERATIONS)
            .ok_or_else(|| NetworkError::EncryptionError("Invalid iteration count".to_string()))?;
        
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &encrypted.salt,
            password.as_bytes(),
            &mut key,
        );

        // Decrypt data
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create key: {:?}", e)))?;
        let opening_key = aead::LessSafeKey::new(unbound_key);

        let mut ciphertext = encrypted.ciphertext.clone();
        let plaintext = opening_key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(encrypted.nonce),
                aead::Aad::empty(),
                &mut ciphertext,
            )
            .map_err(|e| NetworkError::EncryptionError(format!("Decryption failed: {:?}", e)))?;

        Ok(plaintext.to_vec())
    }

    /// Store encrypted data at the given path
    pub async fn store<T: Serialize>(&self, path: &PathBuf, data: &T, password: &str) -> Result<()> {
        // Serialize data
        let json_data = serde_json::to_vec(data)
            .map_err(|e| NetworkError::SerializationError(format!("Serialization failed: {}", e)))?;

        // Encrypt data
        let encrypted = self.encrypt_data(&json_data, password)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| NetworkError::IoError(e.to_string()))?;
        }

        // Atomic write using temporary file
        let temp_path = path.with_extension("tmp");
        let encrypted_json = serde_json::to_vec_pretty(&encrypted)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to serialize encrypted data: {}", e)))?;

        fs::write(&temp_path, encrypted_json).await
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        
        fs::rename(&temp_path, path).await
            .map_err(|e| NetworkError::IoError(e.to_string()))?;

        tracing::debug!("Stored encrypted data at {:?}", path);
        Ok(())
    }

    /// Load and decrypt data from the given path
    pub async fn load<T: for<'de> Deserialize<'de>>(&self, path: &PathBuf, password: &str) -> Result<Option<T>> {
        if !path.exists() {
            return Ok(None);
        }

        // Read encrypted data
        let encrypted_json = fs::read(path).await
            .map_err(|e| NetworkError::IoError(e.to_string()))?;

        let encrypted: EncryptedData = serde_json::from_slice(&encrypted_json)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to parse encrypted data: {}", e)))?;

        // Decrypt data
        let decrypted_data = self.decrypt_data(&encrypted, password)?;

        // Deserialize data
        let data: T = serde_json::from_slice(&decrypted_data)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to deserialize decrypted data: {}", e)))?;

        tracing::debug!("Loaded encrypted data from {:?}", path);
        Ok(Some(data))
    }

    /// Delete stored data
    pub async fn delete(&self, path: &PathBuf) -> Result<bool> {
        if path.exists() {
            fs::remove_file(path).await
                .map_err(|e| NetworkError::IoError(e.to_string()))?;
            tracing::debug!("Deleted stored data at {:?}", path);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all files in a directory
    pub async fn list_files(&self, dir: &PathBuf) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(dir).await
            .map_err(|e| NetworkError::IoError(e.to_string()))?;

        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| NetworkError::IoError(e.to_string()))? {
            
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }

        Ok(files)
    }

    /// Get storage path for a device's persistent identity
    pub fn device_identity_path(&self, device_id: &Uuid) -> PathBuf {
        self.base_path.join("devices").join(format!("{}.json", device_id))
    }

    /// Get storage path for device connection data
    pub fn device_connection_path(&self, device_id: &Uuid, peer_device_id: &Uuid) -> PathBuf {
        self.base_path
            .join("connections")
            .join(device_id.to_string())
            .join(format!("{}.json", peer_device_id))
    }

    /// Get storage path for connection history
    pub fn connection_history_path(&self, device_id: &Uuid) -> PathBuf {
        self.base_path
            .join("history")
            .join(format!("{}.json", device_id))
    }

    /// Clean up old encrypted data based on age
    pub async fn cleanup_old_data(&self, max_age_days: u32) -> Result<usize> {
        let cutoff_time = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let mut cleaned_count = 0;

        // Cleanup connection history
        let history_dir = self.base_path.join("history");
        if history_dir.exists() {
            let files = self.list_files(&history_dir).await?;
            for file in files {
                if let Ok(metadata) = fs::metadata(&file).await {
                    if let Ok(modified) = metadata.modified() {
                        let modified_dt = DateTime::<Utc>::from(modified);
                        if modified_dt < cutoff_time {
                            if self.delete(&file).await? {
                                cleaned_count += 1;
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("Cleaned up {} old encrypted files", cleaned_count);
        Ok(cleaned_count)
    }
}

/// Test helper for storage operations
#[cfg(test)]
impl SecureStorage {
    /// Create temporary storage for testing
    pub fn temp() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("spacedrive-test-{}", Uuid::new_v4()));
        Self::new(temp_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        message: String,
        number: i32,
    }

    #[tokio::test]
    async fn test_encrypt_decrypt() {
        let storage = SecureStorage::temp();
        let password = "test-password-123";
        
        let original_data = TestData {
            message: "Hello, World!".to_string(),
            number: 42,
        };

        // Test encryption/decryption
        let json_data = serde_json::to_vec(&original_data).unwrap();
        let encrypted = storage.encrypt_data(&json_data, password).unwrap();
        let decrypted_data = storage.decrypt_data(&encrypted, password).unwrap();
        let recovered_data: TestData = serde_json::from_slice(&decrypted_data).unwrap();

        assert_eq!(original_data, recovered_data);
    }

    #[tokio::test]
    async fn test_store_load() {
        let storage = SecureStorage::temp();
        let password = "test-password-456";
        let test_path = storage.base_path.join("test.json");
        
        let original_data = TestData {
            message: "Store and load test".to_string(),
            number: 123,
        };

        // Store and load
        storage.store(&test_path, &original_data, password).await.unwrap();
        let loaded_data: Option<TestData> = storage.load(&test_path, password).await.unwrap();
        
        assert_eq!(Some(original_data), loaded_data);

        // Test loading non-existent file
        let missing_path = storage.base_path.join("missing.json");
        let missing_data: Option<TestData> = storage.load(&missing_path, password).await.unwrap();
        assert_eq!(None, missing_data);
    }

    #[tokio::test]
    async fn test_wrong_password() {
        let storage = SecureStorage::temp();
        let password = "correct-password";
        let wrong_password = "wrong-password";
        let test_path = storage.base_path.join("test.json");
        
        let original_data = TestData {
            message: "Password test".to_string(),
            number: 789,
        };

        // Store with correct password
        storage.store(&test_path, &original_data, password).await.unwrap();
        
        // Try to load with wrong password
        let result: Result<Option<TestData>> = storage.load(&test_path, wrong_password).await;
        assert!(result.is_err());
    }
}