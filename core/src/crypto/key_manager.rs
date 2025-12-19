//! Unified key management system
//!
//! Manages all encryption keys in Spacedrive:
//! - Device key: Stored in OS keychain (with file fallback)
//! - Library keys: Stored encrypted in redb database
//! - Cloud credentials: Stored encrypted in library database (not in key manager)

use chacha20poly1305::{
	aead::{Aead, KeyInit, OsRng},
	XChaCha20Poly1305, XNonce,
};
use keyring::{Entry, Error as KeyringError};
use rand::RngCore;
use redb::{Database, ReadableTable, TableDefinition};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

const KEYRING_SERVICE: &str = "Spacedrive";
const DEVICE_KEY_USERNAME: &str = "device_key";
const KEY_LENGTH: usize = 32; // 256 bits

// redb table for encrypted secrets
const SECRETS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("secrets");

#[derive(Error, Debug)]
pub enum KeyManagerError {
	#[error("Keyring error: {0}")]
	Keyring(#[from] KeyringError),

	#[error("Database error: {0}")]
	Database(#[from] redb::Error),

	#[error("Storage error: {0}")]
	StorageError(#[from] redb::StorageError),

	#[error("Table error: {0}")]
	TableError(#[from] redb::TableError),

	#[error("Transaction error: {0}")]
	TransactionError(#[from] redb::TransactionError),

	#[error("Commit error: {0}")]
	CommitError(#[from] redb::CommitError),

	#[error("Database error: {0}")]
	DatabaseError(#[from] redb::DatabaseError),

	#[error("Encryption error: {0}")]
	Encryption(String),

	#[error("Decryption error: {0}")]
	Decryption(String),

	#[error("Invalid key format")]
	InvalidKeyFormat,

	#[error("Key not found: {0}")]
	KeyNotFound(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
}

/// Unified key manager for all Spacedrive encryption keys
pub struct KeyManager {
	/// Path to encrypted secrets database
	db_path: PathBuf,

	/// redb database for encrypted secrets
	db: Arc<RwLock<Database>>,

	/// Device master key (cached in memory)
	device_key: Arc<RwLock<Option<[u8; KEY_LENGTH]>>>,

	/// File fallback path for device key (used in tests)
	device_key_fallback: Option<PathBuf>,
}

impl KeyManager {
	/// Create a new KeyManager
	pub fn new(data_dir: PathBuf) -> Result<Self, KeyManagerError> {
		Self::new_with_fallback(data_dir, None)
	}

	/// Create a new KeyManager with file fallback for device key (for tests)
	pub fn new_with_fallback(
		data_dir: PathBuf,
		device_key_fallback: Option<PathBuf>,
	) -> Result<Self, KeyManagerError> {
		let db_path = data_dir.join("secrets.redb");

		// Create or open the redb database
		let db = Database::create(&db_path)?;
		let db = Arc::new(RwLock::new(db));

		Ok(Self {
			db_path,
			db,
			device_key: Arc::new(RwLock::new(None)),
			device_key_fallback,
		})
	}

	/// Get or create the device master key
	pub async fn get_device_key(&self) -> Result<[u8; KEY_LENGTH], KeyManagerError> {
		// Check if already cached
		{
			let cached = self.device_key.read().await;
			if let Some(key) = *cached {
				return Ok(key);
			}
		}

		// Try to load from file fallback first (if specified and exists)
		if let Some(ref path) = self.device_key_fallback {
			if path.exists() {
				if let Ok(key_hex) = std::fs::read_to_string(path) {
					if let Ok(key_bytes) = hex::decode(key_hex.trim()) {
						if key_bytes.len() == KEY_LENGTH {
							let mut key = [0u8; KEY_LENGTH];
							key.copy_from_slice(&key_bytes);

							// Cache it
							*self.device_key.write().await = Some(key);
							return Ok(key);
						}
					}
				}
			}

			// File doesn't exist - generate new key and save to file
			let key = self.generate_key()?;
			let key_hex = hex::encode(key);

			if let Some(parent) = path.parent() {
				let _ = std::fs::create_dir_all(parent);
			}
			std::fs::write(path, &key_hex)?;

			// Cache it
			*self.device_key.write().await = Some(key);
			return Ok(key);
		}

		// No fallback - use keyring
		let entry = Entry::new(KEYRING_SERVICE, DEVICE_KEY_USERNAME)?;

		match entry.get_password() {
			Ok(key_hex) => {
				let key_bytes =
					hex::decode(key_hex).map_err(|_| KeyManagerError::InvalidKeyFormat)?;

				if key_bytes.len() != KEY_LENGTH {
					return Err(KeyManagerError::InvalidKeyFormat);
				}

				let mut key = [0u8; KEY_LENGTH];
				key.copy_from_slice(&key_bytes);

				// Cache it
				*self.device_key.write().await = Some(key);
				Ok(key)
			}
			Err(KeyringError::NoEntry) => {
				// Generate new device key
				let key = self.generate_key()?;
				let key_hex = hex::encode(key);
				entry.set_password(&key_hex)?;

				// Cache it
				*self.device_key.write().await = Some(key);
				Ok(key)
			}
			Err(e) => Err(KeyManagerError::Keyring(e)),
		}
	}

	/// Get a library encryption key (creates if doesn't exist)
	pub async fn get_library_key(
		&self,
		library_id: Uuid,
	) -> Result<[u8; KEY_LENGTH], KeyManagerError> {
		let key_id = format!("library_{}", library_id);

		// Try to load from encrypted storage
		let db = self.db.read().await;
		let read_txn = db.begin_read()?;

		// Handle case where table doesn't exist yet (first time)
		let table_result = read_txn.open_table(SECRETS_TABLE);

		if let Ok(table) = table_result {
			if let Some(encrypted_value) = table.get(key_id.as_str())? {
				drop(table);
				drop(read_txn);
				drop(db);

				// Decrypt the library key
				let device_key = self.get_device_key().await?;
				let decrypted = self.decrypt(&encrypted_value.value(), &device_key)?;

				if decrypted.len() != KEY_LENGTH {
					return Err(KeyManagerError::InvalidKeyFormat);
				}

				let mut key = [0u8; KEY_LENGTH];
				key.copy_from_slice(&decrypted);
				return Ok(key);
			}
		}

		drop(read_txn);
		drop(db);

		// Key doesn't exist - generate new one
		let key = self.generate_key()?;

		// Encrypt and store it
		let device_key = self.get_device_key().await?;
		let encrypted = self.encrypt(&key, &device_key)?;

		let db = self.db.write().await;
		let write_txn = db.begin_write()?;
		{
			let mut table = write_txn.open_table(SECRETS_TABLE)?;
			table.insert(key_id.as_str(), encrypted.as_slice())?;
		}
		write_txn.commit()?;

		Ok(key)
	}

	/// Store an encrypted secret in the KV store
	pub async fn set_secret(&self, key: &str, value: &[u8]) -> Result<(), KeyManagerError> {
		let device_key = self.get_device_key().await?;
		let encrypted = self.encrypt(value, &device_key)?;

		let db = self.db.write().await;
		let write_txn = db.begin_write()?;
		{
			let mut table = write_txn.open_table(SECRETS_TABLE)?;
			table.insert(key, encrypted.as_slice())?;
		}
		write_txn.commit()?;

		Ok(())
	}

	/// Get a decrypted secret from the KV store
	pub async fn get_secret(&self, key: &str) -> Result<Vec<u8>, KeyManagerError> {
		let db = self.db.read().await;
		let read_txn = db.begin_read()?;
		let table = read_txn.open_table(SECRETS_TABLE)?;

		let encrypted_value = table
			.get(key)?
			.ok_or_else(|| KeyManagerError::KeyNotFound(key.to_string()))?;

		let device_key = self.get_device_key().await?;
		let decrypted = self.decrypt(&encrypted_value.value(), &device_key)?;

		Ok(decrypted)
	}

	/// Delete a secret from the KV store
	pub async fn delete_secret(&self, key: &str) -> Result<(), KeyManagerError> {
		let db = self.db.write().await;
		let write_txn = db.begin_write()?;
		{
			let mut table = write_txn.open_table(SECRETS_TABLE)?;
			table.remove(key)?;
		}
		write_txn.commit()?;

		Ok(())
	}

	/// Close the database and release file locks
	/// This should be called before dropping KeyManager to ensure clean shutdown
	pub async fn close(&self) -> Result<(), KeyManagerError> {
		// Get a write lock and replace with an in-memory database to force file close
		let mut db_guard = self.db.write().await;
		// Drop the old database and replace with a dummy in-memory one
		drop(std::mem::replace(
			&mut *db_guard,
			Database::create(":memory:")?,
		));
		Ok(())
	}

	/// Generate a new random key
	fn generate_key(&self) -> Result<[u8; KEY_LENGTH], KeyManagerError> {
		let mut key = [0u8; KEY_LENGTH];
		OsRng.fill_bytes(&mut key);
		Ok(key)
	}

	/// Encrypt data with XChaCha20-Poly1305
	fn encrypt(&self, data: &[u8], key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>, KeyManagerError> {
		// Generate random nonce
		let mut nonce_bytes = [0u8; 24];
		OsRng.fill_bytes(&mut nonce_bytes);
		let nonce = XNonce::from_slice(&nonce_bytes);

		// Create cipher
		let cipher = XChaCha20Poly1305::new(key.into());

		// Encrypt
		let ciphertext = cipher
			.encrypt(nonce, data)
			.map_err(|e| KeyManagerError::Encryption(e.to_string()))?;

		// Prepend nonce to ciphertext
		let mut result = nonce.to_vec();
		result.extend_from_slice(&ciphertext);

		Ok(result)
	}

	/// Decrypt data with XChaCha20-Poly1305
	fn decrypt(
		&self,
		encrypted: &[u8],
		key: &[u8; KEY_LENGTH],
	) -> Result<Vec<u8>, KeyManagerError> {
		// Extract nonce (first 24 bytes)
		if encrypted.len() < 24 {
			return Err(KeyManagerError::Decryption(
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
			.map_err(|e| KeyManagerError::Decryption(e.to_string()))?;

		Ok(plaintext)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_device_key_persistence() {
		// Use a unique directory name to avoid database conflicts between test runs
		let temp_dir = TempDir::new().unwrap();
		let test_subdir = temp_dir
			.path()
			.join(format!("test_device_key_{}", uuid::Uuid::new_v4()));
		std::fs::create_dir_all(&test_subdir).unwrap();
		let fallback = test_subdir.join("device_key.txt");

		let manager1 =
			KeyManager::new_with_fallback(test_subdir.clone(), Some(fallback.clone())).unwrap();
		let key1 = manager1.get_device_key().await.unwrap();
		drop(manager1); // Explicitly drop to close the database

		let manager2 = KeyManager::new_with_fallback(test_subdir, Some(fallback)).unwrap();
		let key2 = manager2.get_device_key().await.unwrap();

		assert_eq!(key1, key2);
	}

	#[tokio::test]
	async fn test_library_key_storage() {
		let temp_dir = TempDir::new().unwrap();
		let manager = KeyManager::new_with_fallback(
			temp_dir.path().to_path_buf(),
			Some(temp_dir.path().join("device_key.txt")),
		)
		.unwrap();

		let library_id = Uuid::new_v4();
		let key1 = manager.get_library_key(library_id).await.unwrap();
		let key2 = manager.get_library_key(library_id).await.unwrap();

		assert_eq!(key1, key2);
	}

	#[tokio::test]
	async fn test_secret_storage() {
		let temp_dir = TempDir::new().unwrap();
		let manager = KeyManager::new_with_fallback(
			temp_dir.path().to_path_buf(),
			Some(temp_dir.path().join("device_key.txt")),
		)
		.unwrap();

		let secret = b"my secret data";
		manager.set_secret("test_key", secret).await.unwrap();

		let retrieved = manager.get_secret("test_key").await.unwrap();
		assert_eq!(secret, retrieved.as_slice());
	}
}
