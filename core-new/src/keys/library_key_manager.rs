//! Library encryption key management using OS secure storage
use keyring::{Entry, Error as KeyringError};
use rand::RngCore;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

const KEYRING_SERVICE: &str = "SpacedriveLibraryKeys";
const LIBRARY_KEY_LENGTH: usize = 32; // 256 bits

#[derive(Error, Debug)]
pub enum LibraryKeyError {
	#[error("Keyring error: {0}")]
	Keyring(#[from] KeyringError),
	#[error("Invalid key length: expected {LIBRARY_KEY_LENGTH} bytes, got {0}")]
	InvalidKeyLength(usize),
	#[error("Key not found for library: {0}")]
	KeyNotFound(Uuid),
	#[error("Failed to generate random key")]
	RandomKeyGenerationFailed,
	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),
}

pub struct LibraryKeyManager {
	keyring_entry: Entry,
}

impl LibraryKeyManager {
	pub fn new() -> Result<Self, LibraryKeyError> {
		let entry = Entry::new(KEYRING_SERVICE, "library_keys_store")?;
		Ok(Self {
			keyring_entry: entry,
		})
	}

	/// Get or create a library encryption key for a given library ID
	pub fn get_or_create_library_key(
		&self,
		library_id: Uuid,
	) -> Result<[u8; LIBRARY_KEY_LENGTH], LibraryKeyError> {
		let mut all_keys = self.load_all_library_keys()?;
		if let Some(key_hex) = all_keys.get(&library_id) {
			let key_bytes =
				hex::decode(key_hex).map_err(|_| LibraryKeyError::InvalidKeyLength(0))?; // Placeholder for actual error
			if key_bytes.len() != LIBRARY_KEY_LENGTH {
				return Err(LibraryKeyError::InvalidKeyLength(key_bytes.len()));
			}
			let mut key = [0u8; LIBRARY_KEY_LENGTH];
			key.copy_from_slice(&key_bytes);
			Ok(key)
		} else {
			let new_key = self.generate_new_library_key()?;
			all_keys.insert(library_id, hex::encode(new_key));
			self.save_all_library_keys(&all_keys)?;
			Ok(new_key)
		}
	}

	/// Get a library encryption key for a given library ID
	pub fn get_library_key(
		&self,
		library_id: Uuid,
	) -> Result<[u8; LIBRARY_KEY_LENGTH], LibraryKeyError> {
		let all_keys = self.load_all_library_keys()?;
		let key_hex = all_keys
			.get(&library_id)
			.ok_or(LibraryKeyError::KeyNotFound(library_id))?;
		let key_bytes = hex::decode(key_hex).map_err(|_| LibraryKeyError::InvalidKeyLength(0))?; // Placeholder for actual error
		if key_bytes.len() != LIBRARY_KEY_LENGTH {
			return Err(LibraryKeyError::InvalidKeyLength(key_bytes.len()));
		}
		let mut key = [0u8; LIBRARY_KEY_LENGTH];
		key.copy_from_slice(&key_bytes);
		Ok(key)
	}

	/// Store a library encryption key for a given library ID
	pub fn store_library_key(
		&self,
		library_id: Uuid,
		key: [u8; LIBRARY_KEY_LENGTH],
	) -> Result<(), LibraryKeyError> {
		let mut all_keys = self.load_all_library_keys()?;
		all_keys.insert(library_id, hex::encode(key));
		self.save_all_library_keys(&all_keys)?;
		Ok(())
	}

	/// Delete a library encryption key for a given library ID
	pub fn delete_library_key(&self, library_id: Uuid) -> Result<(), LibraryKeyError> {
		let mut all_keys = self.load_all_library_keys()?;
		all_keys.remove(&library_id);
		self.save_all_library_keys(&all_keys)?;
		Ok(())
	}

	fn generate_new_library_key(&self) -> Result<[u8; LIBRARY_KEY_LENGTH], LibraryKeyError> {
		use rand::RngCore;
		let mut key = [0u8; LIBRARY_KEY_LENGTH];
		rand::thread_rng()
			.try_fill_bytes(&mut key)
			.map_err(|_| LibraryKeyError::RandomKeyGenerationFailed)?;
		Ok(key)
	}

	fn load_all_library_keys(&self) -> Result<HashMap<Uuid, String>, LibraryKeyError> {
		match self.keyring_entry.get_password() {
			Ok(json_string) => serde_json::from_str(&json_string).map_err(LibraryKeyError::Json),
			Err(KeyringError::NoEntry) => Ok(HashMap::new()),
			Err(e) => Err(LibraryKeyError::Keyring(e)),
		}
	}

	fn save_all_library_keys(&self, keys: &HashMap<Uuid, String>) -> Result<(), LibraryKeyError> {
		let json_string = serde_json::to_string(keys).map_err(LibraryKeyError::Json)?;
		self.keyring_entry.set_password(&json_string)?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use uuid::Uuid;

	// Helper to clean up keyring entry after tests
	struct TestCleanup {
		manager: LibraryKeyManager,
	}

	impl Drop for TestCleanup {
		fn drop(&mut self) {
			let _ = self.manager.keyring_entry.delete_credential();
		}
	}

	fn create_test_manager() -> (LibraryKeyManager, TestCleanup) {
		let manager = LibraryKeyManager::new().unwrap();
		let cleanup = TestCleanup {
			manager: LibraryKeyManager::new().unwrap(),
		};
		// Ensure a clean state before test
		let _ = manager.keyring_entry.delete_credential();
		(manager, cleanup)
	}

	#[test]
	fn test_generate_and_retrieve_library_key() {
		let (manager, _cleanup) = create_test_manager();
		let library_id = Uuid::new_v4();

		let key1 = manager.get_or_create_library_key(library_id).unwrap();
		let key2 = manager.get_library_key(library_id).unwrap();

		assert_eq!(key1, key2);
		assert_eq!(key1.len(), LIBRARY_KEY_LENGTH);
	}

	#[test]
	fn test_library_key_persistence() {
		let (manager1, _cleanup) = create_test_manager();
		let library_id = Uuid::new_v4();

		let key1 = manager1.get_or_create_library_key(library_id).unwrap();
		drop(manager1); // Simulate application restart

		let (manager2, _cleanup2) = create_test_manager();
		let key2 = manager2.get_library_key(library_id).unwrap();

		assert_eq!(key1, key2);
	}

	#[test]
	fn test_store_and_delete_library_key() {
		let (manager, _cleanup) = create_test_manager();
		let library_id = Uuid::new_v4();
		let mut test_key = [0u8; LIBRARY_KEY_LENGTH];
		rand::thread_rng().fill_bytes(&mut test_key);

		manager.store_library_key(library_id, test_key).unwrap();
		let retrieved_key = manager.get_library_key(library_id).unwrap();
		assert_eq!(test_key, retrieved_key);

		manager.delete_library_key(library_id).unwrap();
		let result = manager.get_library_key(library_id);
		assert!(matches!(result, Err(LibraryKeyError::KeyNotFound(_))));
	}

	#[test]
	fn test_multiple_library_keys() {
		let (manager, _cleanup) = create_test_manager();
		let library_id1 = Uuid::new_v4();
		let library_id2 = Uuid::new_v4();

		let key1 = manager.get_or_create_library_key(library_id1).unwrap();
		let key2 = manager.get_or_create_library_key(library_id2).unwrap();

		assert_ne!(key1, key2);

		let retrieved_key1 = manager.get_library_key(library_id1).unwrap();
		let retrieved_key2 = manager.get_library_key(library_id2).unwrap();

		assert_eq!(key1, retrieved_key1);
		assert_eq!(key2, retrieved_key2);
	}
}
