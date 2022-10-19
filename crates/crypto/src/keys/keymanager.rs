use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

use uuid::Uuid;

use serde_big_array::BigArray;

use super::hashing::{HashingAlgorithm, HASHING_ALGORITHM_LIST};

// The terminology in this file is very confusing.
// The `master_key` is specific to the `StoredKey`, and is just used internally for encryption.
// The `key` is what the user added/generated within their Spacedrive key manager.
// The `password` in this sense is the user's "master password", similar to a password manager's main password
// The `hashed_key` refers to the value you'd pass to PVM/MD decryption functions. It has been pre-hashed with the content salt.
// The content salt refers to the semi-universal salt that's used for metadata/preview media (unique to each key in the manager)

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredKey {
	pub id: uuid::Uuid,                      // uuid for identification
	pub algorithm: Algorithm, // encryption algorithm for encrypting the master key. can be changed (requires a re-encryption though)
	pub hashing_algorithm: HashingAlgorithm, // hashing algorithm to use for hashing everything related to this key. can't be changed once set.
	pub salt: [u8; SALT_LEN],                // salt to hash the master password with
	pub content_salt: [u8; SALT_LEN],        // salt used for file data
	#[serde(with = "BigArray")]
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is for encrypting the `key`
	pub master_key_nonce: Vec<u8>,           // nonce for encrypting the master key
	pub key_nonce: Vec<u8>,                  // nonce used for encrypting the main key
	pub key: Vec<u8>, // encrypted. the key stored in spacedrive (e.g. generated 64 char key)
}

pub struct KeyManager {
	master_password: Option<Protected<Vec<u8>>>, // the user's. we take ownership here to prevent other functions attempting to manage/pass it to us
	keystore: HashMap<Uuid, StoredKey>,
	keymount: HashMap<Uuid, MountedKey>,
	default: Option<Uuid>,
}

/// The `KeyManager` functions should be used for all key-related management.
impl KeyManager {
	/// Initialize the Key Manager with the user's master password, and `StoredKeys` retrieved from Prisma
	#[must_use]
	pub fn new(stored_keys: Vec<StoredKey>, master_password: Option<Protected<Vec<u8>>>) -> Self {
		let mut keystore = HashMap::new();
		for key in stored_keys {
			keystore.insert(key.id, key);
		}

		let keymount: HashMap<Uuid, MountedKey> = HashMap::new();

		Self {
			master_password,
			keystore,
			keymount,
			default: None,
		}
	}

	/// This allows you to set the default key
	pub fn set_default(&mut self, id: Uuid) -> Result<(), Error> {
		if self.keystore.contains_key(&id) {
			self.default = Some(id);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This allows you to get the default key's ID
	pub fn get_default(&self) -> Result<Uuid, Error> {
		if let Some(default) = self.default {
			Ok(default)
		} else {
			Err(Error::NoDefaultKeySet)
		}
	}

	/// This should ONLY be used within the key manager
	fn get_master_password(&self) -> Result<Protected<Vec<u8>>, Error> {
		match self.master_password.clone() {
			Some(k) => Ok(k),
			None => Err(Error::NoMasterPassword),
		}
	}

	/// This function does not return a value by design.
	/// Once a key is mounted, access it with `KeyManager::access()`
	/// This is to ensure that only functions which require access to the mounted key receive it.
	/// We could add a log to this, so that the user can view mounts
	pub fn mount(&mut self, id: Uuid) -> Result<(), Error> {
		match self.keystore.get(&id) {
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

				let mut hashed_keys = Vec::<Protected<[u8; 32]>>::new();

				// Hash the StoredKey using each available password hashing parameter, so all content is accessible no matter the settings.
				// It makes key mounting more expensive, but it allows for greater UX and customizability.
				for hashing_algorithm in HASHING_ALGORITHM_LIST {
					hashed_keys.push(hashing_algorithm.hash(key.clone(), stored_key.content_salt)?);
				}

				// Construct the MountedKey and insert it into the Keymount
				let mounted_key = MountedKey {
					key,
					content_salt: stored_key.content_salt,
					hashed_keys,
				};

				self.keymount.insert(id, mounted_key);

				Ok(())
			}
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing the internal keymount.
	/// We could add a log to this, so that the user can view accesses
	pub fn access_mount(&self, id: Uuid) -> Result<MountedKey, Error> {
		match self.keymount.get(&id) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing a `StoredKey` from an ID.
	pub fn access_store(&self, id: Uuid) -> Result<StoredKey, Error> {
		match self.keystore.get(&id) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is used to add a new key/password to the keystore.
	/// It does not mount the key, it just registers it.
	/// Once added, you will need to use `KeyManager::access_store()` to retrieve it and add it to Prisma.
	/// You may use the returned ID to identify this key.
	pub fn add_to_keystore(
		&mut self,
		key: &Protected<Vec<u8>>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
	) -> Result<Uuid, Error> {
		let master_password = self.get_master_password()?;

		let id = uuid::Uuid::new_v4();

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
			StreamEncryption::encrypt_bytes(master_key, &key_nonce, algorithm, key, &[])?;

		// Construct the StoredKey
		let stored_key = StoredKey {
			id,
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
		self.keystore.insert(stored_key.id, stored_key);

		// Return the ID so it can be identified
		Ok(id)
	}
}

// derive explicit CLONES only
#[derive(Clone)]
pub struct MountedKey {
	pub key: Protected<Vec<u8>>, // the actual key itself, text format encodable (so it can be viewed with an UI)
	pub content_salt: [u8; SALT_LEN], // the salt used for file data
	pub hashed_keys: Vec<Protected<[u8; 32]>>, // this is hashed with the content salt, for instant access
}
