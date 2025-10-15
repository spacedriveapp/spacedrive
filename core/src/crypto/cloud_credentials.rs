//! Cloud storage credential management
//!
//! Provides secure storage for cloud service credentials, encrypted with library keys
//! and stored in the OS keyring.

use chacha20poly1305::{
	aead::{Aead, KeyInit, OsRng},
	XChaCha20Poly1305, XNonce,
};
use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use super::library_key_manager::LibraryKeyManager;
use std::sync::Arc;

const KEYRING_SERVICE: &str = "SpacedriveCloudCredentials";

#[derive(Error, Debug)]
pub enum CloudCredentialError {
	#[error("Keyring error: {0}")]
	Keyring(#[from] KeyringError),

	#[error("Encryption error: {0}")]
	Encryption(String),

	#[error("Decryption error: {0}")]
	Decryption(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Library key error: {0}")]
	LibraryKey(#[from] super::library_key_manager::LibraryKeyError),

	#[error("Credential not found: library={0}, volume={1}")]
	NotFound(Uuid, String),

	#[error("Invalid credential format")]
	InvalidFormat,
}

/// Manages cloud service credentials encrypted with library keys
pub struct CloudCredentialManager {
	library_key_manager: Arc<LibraryKeyManager>,
}

impl CloudCredentialManager {
	pub fn new(library_key_manager: Arc<LibraryKeyManager>) -> Self {
		Self {
			library_key_manager,
		}
	}

	/// Store cloud credentials for a volume, encrypted with the library key
	pub fn store_credential(
		&self,
		library_id: Uuid,
		volume_fingerprint: &str,
		credential: &CloudCredential,
	) -> Result<(), CloudCredentialError> {
		// Get or create library encryption key
		let library_key = self
			.library_key_manager
			.get_or_create_library_key(library_id)?;

		// Serialize credential
		let credential_json = serde_json::to_vec(credential)?;

		// Encrypt with XChaCha20-Poly1305
		let encrypted = self.encrypt_credential(&credential_json, &library_key)?;

		// Store in keyring
		let keyring_key = format!("cloud_{}_{}", library_id, volume_fingerprint);
		let entry = Entry::new(KEYRING_SERVICE, &keyring_key)?;
		entry.set_password(&hex::encode(encrypted))?;

		Ok(())
	}

	/// Retrieve cloud credentials for a volume, decrypted with the library key
	pub fn get_credential(
		&self,
		library_id: Uuid,
		volume_fingerprint: &str,
	) -> Result<CloudCredential, CloudCredentialError> {
		// Get from keyring
		let keyring_key = format!("cloud_{}_{}", library_id, volume_fingerprint);
		let entry = Entry::new(KEYRING_SERVICE, &keyring_key)?;

		let encrypted_hex = entry.get_password().map_err(|e| match e {
			KeyringError::NoEntry => {
				CloudCredentialError::NotFound(library_id, volume_fingerprint.to_string())
			}
			other => CloudCredentialError::Keyring(other),
		})?;

		// Decrypt
		let encrypted =
			hex::decode(&encrypted_hex).map_err(|_| CloudCredentialError::InvalidFormat)?;

		let library_key = self.library_key_manager.get_library_key(library_id)?;
		let decrypted = self.decrypt_credential(&encrypted, &library_key)?;

		// Deserialize
		let credential: CloudCredential = serde_json::from_slice(&decrypted)?;
		Ok(credential)
	}

	/// Delete cloud credentials for a volume
	pub fn delete_credential(
		&self,
		library_id: Uuid,
		volume_fingerprint: &str,
	) -> Result<(), CloudCredentialError> {
		let keyring_key = format!("cloud_{}_{}", library_id, volume_fingerprint);
		let entry = Entry::new(KEYRING_SERVICE, &keyring_key)?;
		entry.delete_credential()?;
		Ok(())
	}

	/// List all volume fingerprints that have stored credentials for a library
	pub fn list_credentials(&self, library_id: Uuid) -> Result<Vec<String>, CloudCredentialError> {
		// Note: This is a simple implementation that might not be efficient
		// In a real scenario, we might want to maintain an index of volume fingerprints
		Ok(Vec::new()) // TODO: Implement credential listing if needed
	}

	/// Encrypt credential data using library key
	fn encrypt_credential(
		&self,
		data: &[u8],
		key: &[u8; 32],
	) -> Result<Vec<u8>, CloudCredentialError> {
		use chacha20poly1305::aead::rand_core::RngCore;

		// Generate random nonce (192 bits for XChaCha20)
		let mut nonce_bytes = [0u8; 24];
		OsRng.fill_bytes(&mut nonce_bytes);
		let nonce = XNonce::from_slice(&nonce_bytes);

		// Create cipher
		let cipher = XChaCha20Poly1305::new(key.into());

		// Encrypt
		let ciphertext = cipher
			.encrypt(nonce, data)
			.map_err(|e| CloudCredentialError::Encryption(e.to_string()))?;

		// Prepend nonce to ciphertext
		let mut result = nonce.to_vec();
		result.extend_from_slice(&ciphertext);

		Ok(result)
	}

	/// Decrypt credential data using library key
	fn decrypt_credential(
		&self,
		encrypted: &[u8],
		key: &[u8; 32],
	) -> Result<Vec<u8>, CloudCredentialError> {
		// Extract nonce (first 24 bytes)
		if encrypted.len() < 24 {
			return Err(CloudCredentialError::Decryption(
				"Invalid ciphertext length".to_string(),
			));
		}

		let (nonce_bytes, ciphertext) = encrypted.split_at(24);
		let nonce = XNonce::from_slice(nonce_bytes);

		// Create cipher
		let cipher = XChaCha20Poly1305::new(key.into());

		// Decrypt
		let plaintext = cipher
			.decrypt(nonce, ciphertext)
			.map_err(|e| CloudCredentialError::Decryption(e.to_string()))?;

		Ok(plaintext)
	}
}

/// Cloud service credentials (stored encrypted in OS keyring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCredential {
	/// Service type
	pub service: crate::volume::CloudServiceType,

	/// Credential data
	pub data: CredentialData,

	/// When this credential was created
	pub created_at: chrono::DateTime<chrono::Utc>,

	/// When this credential expires (for OAuth tokens)
	pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Cloud credential data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CredentialData {
	/// Access key + secret (S3, compatible services)
	AccessKey {
		access_key_id: String,
		secret_access_key: String,
		session_token: Option<String>,
	},

	/// OAuth tokens (Google Drive, Dropbox, OneDrive)
	OAuth {
		access_token: String,
		refresh_token: String,
	},

	/// Simple API key
	ApiKey(String),

	/// Connection string (Azure, etc.)
	ConnectionString(String),
}

impl CloudCredential {
	/// Create a new access key credential
	pub fn new_access_key(
		service: crate::volume::CloudServiceType,
		access_key_id: String,
		secret_access_key: String,
		session_token: Option<String>,
	) -> Self {
		Self {
			service,
			data: CredentialData::AccessKey {
				access_key_id,
				secret_access_key,
				session_token,
			},
			created_at: chrono::Utc::now(),
			expires_at: None,
		}
	}

	/// Create a new OAuth credential
	pub fn new_oauth(
		service: crate::volume::CloudServiceType,
		access_token: String,
		refresh_token: String,
		expires_at: Option<chrono::DateTime<chrono::Utc>>,
	) -> Self {
		Self {
			service,
			data: CredentialData::OAuth {
				access_token,
				refresh_token,
			},
			created_at: chrono::Utc::now(),
			expires_at,
		}
	}

	/// Create a new API key credential
	pub fn new_api_key(service: crate::volume::CloudServiceType, api_key: String) -> Self {
		Self {
			service,
			data: CredentialData::ApiKey(api_key),
			created_at: chrono::Utc::now(),
			expires_at: None,
		}
	}

	/// Check if this credential is expired
	pub fn is_expired(&self) -> bool {
		if let Some(expires_at) = self.expires_at {
			chrono::Utc::now() > expires_at
		} else {
			false
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encrypt_decrypt_credential() {
		let library_key_manager = Arc::new(LibraryKeyManager::new().unwrap());
		let manager = CloudCredentialManager::new(library_key_manager);

		let library_id = Uuid::new_v4();
		let volume_fp = "test-volume-fingerprint";

		// Create test credential
		let credential = CloudCredential::new_access_key(
			crate::volume::CloudServiceType::S3,
			"AKIAIOSFODNN7EXAMPLE".to_string(),
			"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
			None,
		);

		// Store
		manager
			.store_credential(library_id, volume_fp, &credential)
			.unwrap();

		// Retrieve
		let retrieved = manager.get_credential(library_id, volume_fp).unwrap();

		match (&credential.data, &retrieved.data) {
			(
				CredentialData::AccessKey {
					access_key_id: a1,
					secret_access_key: s1,
					..
				},
				CredentialData::AccessKey {
					access_key_id: a2,
					secret_access_key: s2,
					..
				},
			) => {
				assert_eq!(a1, a2);
				assert_eq!(s1, s2);
			}
			_ => panic!("Credential type mismatch"),
		}

		// Cleanup
		manager.delete_credential(library_id, volume_fp).unwrap();
	}

	#[test]
	fn test_credential_expiry() {
		let future = chrono::Utc::now() + chrono::Duration::hours(1);
		let past = chrono::Utc::now() - chrono::Duration::hours(1);

		let credential_not_expired = CloudCredential::new_oauth(
			crate::volume::CloudServiceType::GoogleDrive,
			"access_token".to_string(),
			"refresh_token".to_string(),
			Some(future),
		);

		let credential_expired = CloudCredential::new_oauth(
			crate::volume::CloudServiceType::GoogleDrive,
			"access_token".to_string(),
			"refresh_token".to_string(),
			Some(past),
		);

		assert!(!credential_not_expired.is_expired());
		assert!(credential_expired.is_expired());
	}
}
