//! Cloud storage credential management
//!
//! Provides secure storage for cloud service credentials, encrypted with library keys
//! and stored in the library database.

use chacha20poly1305::{
	aead::{Aead, KeyInit, OsRng},
	XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use super::key_manager::KeyManager;
use crate::infra::db::Database;

#[derive(Error, Debug)]
pub enum CloudCredentialError {
	#[error("Database error: {0}")]
	Database(String),

	#[error("Encryption error: {0}")]
	Encryption(String),

	#[error("Decryption error: {0}")]
	Decryption(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Library key error: {0}")]
	LibraryKey(#[from] super::key_manager::KeyManagerError),

	#[error("Credential not found: library={0}, volume={1}")]
	NotFound(Uuid, String),

	#[error("Invalid credential format")]
	InvalidFormat,
}

/// Manages cloud service credentials encrypted with library keys
pub struct CloudCredentialManager {
	key_manager: Arc<KeyManager>,
	db: Arc<Database>,
	library_id: Uuid,
}

impl CloudCredentialManager {
	pub fn new(key_manager: Arc<KeyManager>, db: Arc<Database>, library_id: Uuid) -> Self {
		Self {
			key_manager,
			db,
			library_id,
		}
	}

	/// Store cloud credentials for a volume, encrypted with the library key
	pub async fn store_credential(
		&self,
		library_id: Uuid,
		volume_fingerprint: &str,
		credential: &CloudCredential,
	) -> Result<(), CloudCredentialError> {
		// Get or create library encryption key
		let library_key = self.key_manager.get_library_key(library_id).await?;

		// Serialize credential
		let credential_json = serde_json::to_vec(credential)?;

		// Encrypt with XChaCha20-Poly1305
		let encrypted = self.encrypt_credential(&credential_json, &library_key)?;

		// Store in database
		use crate::infra::db::entities::cloud_credential;
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

		let db = self.db.conn();

		// Check if credential already exists
		let existing = cloud_credential::Entity::find()
			.filter(cloud_credential::Column::VolumeFingerprint.eq(volume_fingerprint))
			.one(db)
			.await
			.map_err(|e| CloudCredentialError::Database(e.to_string()))?;

		if let Some(existing) = existing {
			// Update existing credential
			let mut active: cloud_credential::ActiveModel = existing.into();
			active.encrypted_credential = Set(encrypted);
			active.service_type = Set(format!("{:?}", credential.service));
			active.updated_at = Set(chrono::Utc::now().into());

			active
				.update(db)
				.await
				.map_err(|e| CloudCredentialError::Database(e.to_string()))?;
		} else {
			// Insert new credential
			let active = cloud_credential::ActiveModel {
				id: sea_orm::ActiveValue::NotSet,
				volume_fingerprint: Set(volume_fingerprint.to_string()),
				encrypted_credential: Set(encrypted),
				service_type: Set(format!("{:?}", credential.service)),
				created_at: Set(chrono::Utc::now().into()),
				updated_at: Set(chrono::Utc::now().into()),
			};

			active
				.insert(db)
				.await
				.map_err(|e| CloudCredentialError::Database(e.to_string()))?;
		}

		Ok(())
	}

	/// Retrieve cloud credentials for a volume, decrypted with the library key
	pub async fn get_credential(
		&self,
		library_id: Uuid,
		volume_fingerprint: &str,
	) -> Result<CloudCredential, CloudCredentialError> {
		// Get from database
		use crate::infra::db::entities::cloud_credential;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let db = self.db.conn();

		let credential_model = cloud_credential::Entity::find()
			.filter(cloud_credential::Column::VolumeFingerprint.eq(volume_fingerprint))
			.one(db)
			.await
			.map_err(|e| CloudCredentialError::Database(e.to_string()))?
			.ok_or_else(|| {
				CloudCredentialError::NotFound(library_id, volume_fingerprint.to_string())
			})?;

		// Decrypt
		let library_key = self.key_manager.get_library_key(library_id).await?;
		let decrypted =
			self.decrypt_credential(&credential_model.encrypted_credential, &library_key)?;

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
		// Delete from database
		use crate::infra::db::entities::cloud_credential;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let db = self.db.conn();

		// Use blocking call since this method is not async
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async {
				cloud_credential::Entity::delete_many()
					.filter(cloud_credential::Column::VolumeFingerprint.eq(volume_fingerprint))
					.exec(db)
					.await
					.map_err(|e| CloudCredentialError::Database(e.to_string()))?;

				Ok(())
			})
		})
	}

	/// List all volume fingerprints that have stored credentials for this library
	pub async fn list_credentials(&self) -> Result<Vec<String>, CloudCredentialError> {
		use crate::infra::db::entities::cloud_credential;
		use sea_orm::EntityTrait;

		let db = self.db.conn();

		let credentials = cloud_credential::Entity::find()
			.all(db)
			.await
			.map_err(|e| CloudCredentialError::Database(e.to_string()))?;

		Ok(credentials
			.into_iter()
			.map(|c| c.volume_fingerprint)
			.collect())
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

/// Cloud service credentials (stored encrypted in library database)
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

	#[tokio::test]
	async fn test_encrypt_decrypt_credential() {
		let temp_dir = tempfile::tempdir().unwrap();
		let db_path = temp_dir.path().join("test.db");
		let key_manager = Arc::new(
			KeyManager::new_with_fallback(
				temp_dir.path().to_path_buf(),
				Some(temp_dir.path().join("device_key")),
			)
			.unwrap(),
		);

		// Create database
		let db = Arc::new(Database::create(&db_path).await.unwrap());
		db.migrate().await.unwrap();

		let library_id = Uuid::new_v4();
		let manager = CloudCredentialManager::new(key_manager, db, library_id);

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
			.await
			.unwrap();

		// Retrieve
		let retrieved = manager.get_credential(library_id, volume_fp).await.unwrap();

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

		// List credentials
		let credentials = manager.list_credentials().await.unwrap();
		assert_eq!(credentials.len(), 1);
		assert_eq!(credentials[0], volume_fp);

		// Cleanup
		manager.delete_credential(library_id, volume_fp).unwrap();

		// Verify deleted
		let result = manager.get_credential(library_id, volume_fp).await;
		assert!(result.is_err());
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
