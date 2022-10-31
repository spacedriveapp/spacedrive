//! This module contains Spacedrive's key manager implementation.
//!
//! The key manager is used for keeping track of keys within memory, and mounting them on demand.
//!
//! The key manager is initialised, and added to a global state so it is accessible everywhere.
//! It is also populated with all keys from the Prisma database.
//!
//! # Examples
//!
//! ```rust
//! use sd_crypto::keys::keymanager::KeyManager;
//! use sd_crypto::Protected;
//! use sd_crypto::crypto::stream::Algorithm;
//! use sd_crypto::keys::hashing::{HashingAlgorithm, Params};
//!
//! let master_password = Protected::new(b"password".to_vec());
//!
//! // Initialise a `Keymanager` with no stored keys and no master password
//! let mut key_manager = KeyManager::new(vec![], None);
//!
//! // Set the master password
//! key_manager.set_master_password(master_password);
//!
//! let new_password = Protected::new(b"super secure".to_vec());
//!
//! // Register the new key with the key manager
//! let added_key = key_manager.add_to_keystore(new_password, Algorithm::XChaCha20Poly1305, HashingAlgorithm::Argon2id(Params::Standard)).unwrap();
//!
//! // Write the stored key to the database here (with `KeyManager::access_keystore()`)
//!
//! // Mount the key we just added (with the returned UUID)
//! key_manager.mount(added_key);
//!
//! // Retrieve all currently mounted, hashed keys to pass to a decryption function.
//! let keys = key_manager.enumerate_hashed_keys();
//! ```

use std::sync::Mutex;

use crate::crypto::stream::{StreamDecryption, StreamEncryption};
use crate::error::Error;
use crate::primitives::{
	generate_master_key, generate_nonce, generate_salt, to_array, MASTER_KEY_LEN,
};
use crate::{
	crypto::stream::Algorithm,
	primitives::{ENCRYPTED_MASTER_KEY_LEN, SALT_LEN},
	Protected,
};

use dashmap::DashMap;
use uuid::Uuid;

use super::hashing::HashingAlgorithm;

// The terminology in this file is very confusing.
// The `master_key` is specific to the `StoredKey`, and is just used internally for encryption.
// The `key` is what the user added/generated within their Spacedrive key manager.
// The `password` in this sense is the user's "master password", similar to a password manager's main password
// The `hashed_key` refers to the value you'd pass to PVM/MD decryption functions. It has been pre-hashed with the content salt.
// The content salt refers to the semi-universal salt that's used for metadata/preview media (unique to each key in the manager)

#[derive(Clone, PartialEq, Eq)]
pub struct StoredKey {
	pub uuid: uuid::Uuid,     // uuid for identification. shared with mounted keys
	pub algorithm: Algorithm, // encryption algorithm for encrypting the master key. can be changed (requires a re-encryption though)
	pub hashing_algorithm: HashingAlgorithm, // hashing algorithm to use for hashing everything related to this key. can't be changed once set.
	pub salt: [u8; SALT_LEN],                // salt to hash the master password with
	pub content_salt: [u8; SALT_LEN],        // salt used for file data
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is for encrypting the `key`
	pub master_key_nonce: Vec<u8>,           // nonce for encrypting the master key
	pub key_nonce: Vec<u8>,                  // nonce used for encrypting the main key
	pub key: Vec<u8>, // encrypted. the key stored in spacedrive (e.g. generated 64 char key)
}

pub struct KeyManager {
	master_password: Mutex<Option<Protected<Vec<u8>>>>, // the user's. we take ownership here to prevent other functions attempting to manage/pass it to us
	keystore: DashMap<Uuid, StoredKey>,
	keymount: DashMap<Uuid, MountedKey>,
	default: Mutex<Option<Uuid>>,
}

/// The `KeyManager` functions should be used for all key-related management.
impl KeyManager {
	/// Initialize the Key Manager with the user's master password, and `StoredKeys` retrieved from Prisma
	#[must_use]
	pub fn new(stored_keys: Vec<StoredKey>, master_password: Option<Protected<Vec<u8>>>) -> Self {
		let keystore = DashMap::new();
		for key in stored_keys {
			keystore.insert(key.uuid, key);
		}

		let keymount: DashMap<Uuid, MountedKey> = DashMap::new();

		Self {
			master_password: Mutex::new(master_password),
			keystore,
			keymount,
			default: Mutex::new(None),
		}
	}

	/// This function should be used to populate the keystore with multiple stored keys at a time.
	///
	/// It's suitable for when you created the key manager without populating it.
	pub fn populate_keystore(&self, stored_keys: Vec<StoredKey>) -> Result<(), Error> {
		for key in stored_keys {
			self.keystore.insert(key.uuid, key);
		}

		Ok(())
	}
	pub fn remove_key(&self, uuid: Uuid) -> Result<(), Error> {
		if self.keystore.contains_key(&uuid) {
			// unmount if mounted
			if self.keymount.contains_key(&uuid) {
				self.unmount(uuid).unwrap();
			}

			// remove from keystore
			self.keystore.remove(&uuid);

			// unset as default (if it is the default)
			if let Some(default) = *self.default.lock().unwrap() {
				if default == uuid {
					*self.default.lock().unwrap() = None;
				}
			}
		}

		Ok(())
	}

	/// This allows you to set the default key
	pub fn set_default(&self, uuid: Uuid) -> Result<(), Error> {
		if self.keystore.contains_key(&uuid) {
			*self.default.lock().unwrap() = Some(uuid);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This allows you to get the default key's ID
	pub fn get_default(&self) -> Result<Uuid, Error> {
		if let Some(default) = *self.default.lock().unwrap() {
			Ok(default)
		} else {
			Err(Error::NoDefaultKeySet)
		}
	}

	/// This should ONLY be used internally.
	fn get_master_password(&self) -> Result<Protected<Vec<u8>>, Error> {
		match &*self.master_password.lock().unwrap() {
			Some(k) => Ok(k.clone()),
			None => Err(Error::NoMasterPassword),
		}
	}

	pub fn set_master_password(&self, master_password: Protected<Vec<u8>>) -> Result<(), Error> {
		// this returns a result, so we can potentially implement password checking functionality
		*self.master_password.lock().unwrap() = Some(master_password);
		Ok(())
	}

	/// This function is for removing a previously-added master password
	pub fn clear_master_password(&self) {
		*self.master_password.lock().unwrap() = None;
	}

	#[must_use]
	pub fn has_master_password(&self) -> bool {
		self.master_password.lock().unwrap().is_some()
	}

	/// This function is used for emptying the entire keystore.
	pub fn empty_keystore(&self) {
		self.keystore.clear();
	}

	/// This function is used for unmounting all keys at once.
	pub fn empty_keymount(&self) {
		// i'm unsure whether or not `.clear()` also calls drop
		// if it doesn't, we're going to need to find another way to call drop on these values
		// that way they will be zeroized and removed from memory fully
		self.keymount.clear();
	}

	/// This function can be used for comparing an array of `StoredKeys` to the currently loaded keystore.
	pub fn compare_keystore(&self, supplied_keys: &[StoredKey]) -> Result<(), Error> {
		if supplied_keys.len() != self.keystore.len() {
			return Err(Error::KeystoreMismatch);
		}

		for key in supplied_keys {
			let keystore_key = match self.keystore.get(&key.uuid) {
				Some(key) => key.clone(),
				None => return Err(Error::KeystoreMismatch),
			};

			if *key != keystore_key {
				return Err(Error::KeystoreMismatch);
			}
		}

		Ok(())
	}

	/// This function is for unmounting a key from the key manager
	///
	/// This does not remove the key from the key store
	pub fn unmount(&self, uuid: Uuid) -> Result<(), Error> {
		if self.keymount.contains_key(&uuid) {
			self.keymount.remove(&uuid);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This function returns a Vec of `StoredKey`s, so you can write them somewhere/update the database with them/etc
	///
	/// The database and keystore should be in sync at ALL times (unless the user chose an in-memory only key)
	#[must_use]
	pub fn dump_keystore(&self) -> Vec<StoredKey> {
		self.keystore.iter().map(|key| key.clone()).collect()
	}

	#[must_use]
	pub fn get_mounted_uuids(&self) -> Vec<Uuid> {
		self.keymount.iter().map(|key| key.uuid).collect()
	}

	/// This function does not return a value by design.
	///
	/// Once a key is mounted, access it with `KeyManager::access()`
	///
	/// This is to ensure that only functions which require access to the mounted key receive it.
	///
	/// We could add a log to this, so that the user can view mounts
	pub fn mount(&self, uuid: Uuid) -> Result<(), Error> {
		match self.keystore.get(&uuid) {
			Some(stored_key) => {
				let master_password = self.get_master_password()?;

				let hashed_password = stored_key
					.hashing_algorithm
					.hash(master_password, stored_key.salt)?;

				let mut master_key = [0u8; MASTER_KEY_LEN];

				// Decrypt the StoredKey's master key using the user's hashed password
				let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
					hashed_password,
					&stored_key.master_key_nonce,
					stored_key.algorithm,
					&stored_key.master_key,
					&[],
				) {
					master_key.copy_from_slice(&decrypted_master_key);
					Ok(Protected::new(master_key))
				} else {
					Err(Error::IncorrectPassword)
				}?;

				// Decrypt the StoredKey using the decrypted master key
				let key = StreamDecryption::decrypt_bytes(
					master_key,
					&stored_key.key_nonce,
					stored_key.algorithm,
					&stored_key.key,
					&[],
				)?;

				// Hash the key once with the parameters/algorithm the user selected during first mount
				let hashed_key = stored_key
					.hashing_algorithm
					.hash(key.clone(), stored_key.content_salt)?;

				// Construct the MountedKey and insert it into the Keymount
				let mounted_key = MountedKey {
					uuid: stored_key.uuid,
					key,
					content_salt: stored_key.content_salt,
					hashed_key,
				};

				self.keymount.insert(uuid, mounted_key);

				Ok(())
			}
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing the internal keymount.
	///
	/// We could add a log to this, so that the user can view accesses
	pub fn access_keymount(&self, uuid: Uuid) -> Result<MountedKey, Error> {
		match self.keymount.get(&uuid) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing a `StoredKey`.
	pub fn access_keystore(&self, uuid: Uuid) -> Result<StoredKey, Error> {
		match self.keystore.get(&uuid) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for getting an entire collection of hashed keys.
	///
	/// These are ideal for passing over to decryption functions, as each decryption attempt is negligible, performance wise.
	///
	/// This means we don't need to keep super specific track of which key goes to which file, and we can just throw all of them at it.
	#[must_use]
	pub fn enumerate_hashed_keys(&self) -> Vec<Protected<[u8; 32]>> {
		self.keymount
			.iter()
			.map(|mounted_key| mounted_key.hashed_key.clone())
			.collect::<Vec<Protected<[u8; 32]>>>()
	}

	/// This function is used to add a new key/password to the keystore.
	///
	/// You should use this when a new key is added, as it will generate salts/nonces/etc.
	///
	/// It does not mount the key, it just registers it.
	///
	/// Once added, you will need to use `KeyManager::access_keystore()` to retrieve it and add it to Prisma.
	///
	/// You may use the returned ID to identify this key.
	#[allow(clippy::needless_pass_by_value)]
	pub fn add_to_keystore(
		&self,
		key: Protected<Vec<u8>>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
	) -> Result<Uuid, Error> {
		let master_password = self.get_master_password()?;

		let uuid = uuid::Uuid::new_v4();

		// Generate items we'll need for encryption
		let key_nonce = generate_nonce(algorithm);
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);
		let salt = generate_salt();
		let content_salt = generate_salt(); // for PVM/MD

		// Hash the user's master password
		let hashed_password = hashing_algorithm.hash(master_password, salt)?;

		// Encrypted the master key with the user's hashed password
		let encrypted_master_key: [u8; 48] = to_array(StreamEncryption::encrypt_bytes(
			hashed_password,
			&master_key_nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		// Encrypt the actual key (e.g. user-added/autogenerated, text-encodable)
		let encrypted_key =
			StreamEncryption::encrypt_bytes(master_key, &key_nonce, algorithm, &key, &[])?;

		// Construct the StoredKey
		let stored_key = StoredKey {
			uuid,
			algorithm,
			hashing_algorithm,
			salt,
			content_salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce,
			key: encrypted_key,
		};

		// Insert it into the Keystore
		self.keystore.insert(stored_key.uuid, stored_key);

		// Return the ID so it can be identified
		Ok(uuid)
	}
}

/// This is a mounted key, and needs to be kept somewhat hidden.
///
/// This contains the plaintext key, and the same key hashed with the content salt.
#[derive(Clone)]
pub struct MountedKey {
	pub uuid: Uuid,                   // used for identification. shared with stored keys
	pub key: Protected<Vec<u8>>, // the actual key itself, text format encodable (so it can be viewed with an UI)
	pub content_salt: [u8; SALT_LEN], // the salt used for file data
	pub hashed_key: Protected<[u8; 32]>, // this is hashed with the content salt, for instant access
}
