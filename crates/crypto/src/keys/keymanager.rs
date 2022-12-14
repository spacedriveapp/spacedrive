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
use crate::primitives::{
	generate_master_key, generate_nonce, generate_passphrase, generate_salt, to_array, KEY_LEN,
};
use crate::{
	crypto::stream::Algorithm,
	primitives::{ENCRYPTED_KEY_LEN, SALT_LEN},
	Protected,
};
use crate::{Error, Result};

use dashmap::DashMap;
use uuid::Uuid;

#[cfg(feature = "serde")]
use serde_big_array::BigArray;

use super::hashing::HashingAlgorithm;

// The terminology in this file is very confusing.
// The `master_key` is specific to the `StoredKey`, and is just used internally for encryption.
// The `key` is what the user added/generated within their Spacedrive key manager.
// The `password` in this sense is the user's "master password", similar to a password manager's main password
// The `hashed_key` refers to the value you'd pass to PVM/MD decryption functions. It has been pre-hashed with the content salt.
// The content salt refers to the semi-universal salt that's used for metadata/preview media (unique to each key in the manager)

/// This is a stored key, and can be freely written to Prisma/another database.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct StoredKey {
	pub uuid: uuid::Uuid,     // uuid for identification. shared with mounted keys
	pub algorithm: Algorithm, // encryption algorithm for encrypting the master key. can be changed (requires a re-encryption though)
	pub hashing_algorithm: HashingAlgorithm, // hashing algorithm used for hashing the key with the content salt
	pub content_salt: [u8; SALT_LEN],
	#[cfg_attr(feature = "serde", serde(with = "BigArray"))] // salt used for file data
	pub master_key: [u8; ENCRYPTED_KEY_LEN], // this is for encrypting the `key`
	pub master_key_nonce: Vec<u8>, // nonce for encrypting the master key
	pub key_nonce: Vec<u8>,        // nonce used for encrypting the main key
	pub key: Vec<u8>, // encrypted. the key stored in spacedrive (e.g. generated 64 char key)
	pub memory_only: bool,
	pub automount: bool,
}

/// This is a mounted key, and needs to be kept somewhat hidden.
///
/// This contains the plaintext key, and the same key hashed with the content salt.
#[derive(Clone)]
pub struct MountedKey {
	pub uuid: Uuid, // used for identification. shared with stored keys
	pub hashed_key: Protected<[u8; KEY_LEN]>, // this is hashed with the content salt, for instant access
}

/// This is the key manager itself.
///
/// It contains the keystore, the keymount, the master password and the default key.
///
/// Use the associated functions to interact with it.
pub struct KeyManager {
	root_key: Mutex<Option<Protected<[u8; KEY_LEN]>>>, // the root key for the vault
	verification_key: Mutex<Option<StoredKey>>,
	keystore: DashMap<Uuid, StoredKey>,
	keymount: DashMap<Uuid, MountedKey>,
	default: Mutex<Option<Uuid>>,
}

// bundle returned during onboarding
// nil key should be stored within prisma
// secret key should be written down by the user (along with the master password)
/// This bundle is returned during onboarding.
///
/// The verification key should be written to the database, and only one nil-UUID key should exist at any given point for a library.
///
/// The secret key needs to be given to the user, and should be written down.
pub struct OnboardingBundle {
	pub verification_key: StoredKey, // nil UUID key that is only ever used for verifying the master password is correct
	pub master_password: Protected<String>,
	pub secret_key: Protected<String>, // hex encoded string that is required along with the master password
}

pub struct MasterPasswordChangeBundle {
	pub verification_key: StoredKey, // nil UUID key that is only ever used for verifying the master password is correct
	pub secret_key: Protected<String>, // hex encoded string that is required along with the master password
	                                   // pub updated_keystore: Vec<StoredKey>,
}

/// The `KeyManager` functions should be used for all key-related management.
impl KeyManager {
	fn format_secret_key(salt: &[u8; SALT_LEN]) -> Protected<String> {
		let hex_string: String = hex::encode_upper(salt)
			.chars()
			.enumerate()
			.map(|(i, c)| {
				if (i + 1) % 8 == 0 && i != 31 {
					c.to_string() + "-"
				} else {
					c.to_string()
				}
			})
			.into_iter()
			.collect();

		Protected::new(hex_string)
	}

	/// This should be used to generate everything for the user during onboarding.
	///
	/// This will create a master password (a 7-word diceware passphrase), and a secret key (16 bytes, hex encoded)
	///
	/// It will also generate a verification key, which should be written to the database.
	#[allow(clippy::needless_pass_by_value)]
	pub fn onboarding(
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
	) -> Result<OnboardingBundle> {
		let _master_password = generate_passphrase();
		let _salt = generate_salt();

		// BRXKEN128: REMOVE THIS ONCE ONBOARDING HAS BEEN DONE
		let master_password = Protected::new("password".to_string());
		let salt = *b"0000000000000000";

		// Hash the master password
		let hashed_password = hashing_algorithm.hash(
			Protected::new(master_password.expose().as_bytes().to_vec()),
			salt,
		)?;

		let uuid = uuid::Uuid::nil();

		// Generate items we'll need for encryption
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let root_key = generate_master_key();
		let root_key_nonce = generate_nonce(algorithm);

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
			hashed_password,
			&master_key_nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			&root_key_nonce,
			algorithm,
			root_key.expose(),
			&[],
		)?;

		let verification_key = StoredKey {
			uuid,
			algorithm,
			hashing_algorithm,
			content_salt: [0u8; SALT_LEN],
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce: root_key_nonce,
			key: encrypted_root_key,
			memory_only: false,
			automount: false,
		};

		let secret_key = Self::format_secret_key(&salt);

		let onboarding_bundle = OnboardingBundle {
			verification_key,
			master_password,
			secret_key,
		};

		Ok(onboarding_bundle)
	}

	/// Initialize the Key Manager with `StoredKeys` retrieved from Prisma
	pub fn new(stored_keys: Vec<StoredKey>) -> Result<Self> {
		let keystore = DashMap::new();

		let keymount: DashMap<Uuid, MountedKey> = DashMap::new();

		let keymanager = Self {
			root_key: Mutex::new(None),
			verification_key: Mutex::new(None),
			keystore,
			keymount,
			default: Mutex::new(None),
		};

		keymanager.populate_keystore(stored_keys)?;

		Ok(keymanager)
	}

	/// This function should be used to populate the keystore with multiple stored keys at a time.
	///
	/// It's suitable for when you created the key manager without populating it.
	///
	/// This also detects the nil-UUID master passphrase verification key
	pub fn populate_keystore(&self, stored_keys: Vec<StoredKey>) -> Result<()> {
		for key in stored_keys {
			if key.uuid.is_nil() {
				*self.verification_key.lock()? = Some(key);
			} else {
				self.keystore.insert(key.uuid, key);
			}
		}

		Ok(())
	}

	/// This function removes a key from the keystore, the keymount and it's unset as the default.
	pub fn remove_key(&self, uuid: Uuid) -> Result<()> {
		if self.keystore.contains_key(&uuid) {
			// if key is default, clear it
			// do this manually to prevent deadlocks
			let mut default = self.default.lock()?;
			if *default == Some(uuid) {
				*default = None;
			}
			drop(default);

			// unmount if mounted
			if self.keymount.contains_key(&uuid) {
				// use remove as unmount calls the checks that we just did
				self.keymount.remove(&uuid);
			}

			// remove from keystore
			self.keystore.remove(&uuid);
		}

		Ok(())
	}

	/// This allows you to set the default key
	pub fn set_default(&self, uuid: Uuid) -> Result<()> {
		if self.keystore.contains_key(&uuid) {
			*self.default.lock()? = Some(uuid);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This allows you to get the default key's ID
	pub fn get_default(&self) -> Result<Uuid> {
		if let Some(default) = *self.default.lock()? {
			Ok(default)
		} else {
			Err(Error::NoDefaultKeySet)
		}
	}

	/// This allows you to clear the default key
	pub fn clear_default(&self) -> Result<()> {
		let mut default = self.default.lock()?;

		if default.is_some() {
			*default = None;
			Ok(())
		} else {
			Err(Error::NoDefaultKeySet)
		}
	}

	/// This should ONLY be used internally.
	fn get_root_key(&self) -> Result<Protected<[u8; KEY_LEN]>> {
		match &*self.root_key.lock()? {
			Some(k) => Ok(k.clone()),
			None => Err(Error::NoMasterPassword),
		}
	}

	pub fn get_verification_key(&self) -> Result<StoredKey> {
		match &*self.verification_key.lock()? {
			Some(k) => Ok(k.clone()),
			None => Err(Error::NoMasterPassword),
		}
	}

	pub fn is_memory_only(&self, uuid: Uuid) -> Result<bool> {
		match self.keystore.get(&uuid) {
			Some(key) => Ok(key.memory_only),
			None => Err(Error::KeyNotFound),
		}
	}

	#[allow(clippy::needless_pass_by_value)]
	pub fn change_master_password(
		&self,
		master_password: Protected<String>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
	) -> Result<MasterPasswordChangeBundle> {
		let salt = generate_salt();

		let hashed_password = hashing_algorithm.hash(
			Protected::new(master_password.expose().as_bytes().to_vec()),
			salt,
		)?;

		let uuid = uuid::Uuid::nil();

		// Generate items we'll need for encryption
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let root_key = self.get_root_key()?;
		let root_key_nonce = generate_nonce(algorithm);

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
			hashed_password,
			&master_key_nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			&root_key_nonce,
			algorithm,
			root_key.expose(),
			&[],
		)?;

		let verification_key = StoredKey {
			uuid,
			algorithm,
			hashing_algorithm,
			content_salt: [0u8; SALT_LEN],
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce: root_key_nonce,
			key: encrypted_root_key,
			memory_only: false,
			automount: false,
		};

		*self.verification_key.lock()? = Some(verification_key.clone());

		let secret_key = Self::format_secret_key(&salt);

		let mp_change_bundle = MasterPasswordChangeBundle {
			verification_key,
			secret_key,
		};

		Ok(mp_change_bundle)
	}

	/// This is used to change a master password.
	///
	/// The entire keystore is re-encrypted with the new master password, and will require dumping and syncing with Prisma.
	// pub fn rotate_root_key(
	// 	&self,
	// 	master_password: Protected<String>,
	// 	algorithm: Algorithm,
	// 	hashing_algorithm: HashingAlgorithm,
	// ) -> Result<MasterPasswordChangeBundle> {
	// 	let new_root_key = generate_master_key();

	// 	// Iterate over the keystore - decrypt each master key, re-encrypt it with the same algorithm, and collect them into a vec
	// 	let updated_keystore: Result<Vec<StoredKey>> = self
	// 		.dump_keystore()
	// 		.iter()
	// 		.map(|stored_key| {
	// 			let mut stored_key = stored_key.clone();

	// 			let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
	// 				self.get_root_key()?,
	// 				&stored_key.master_key_nonce,
	// 				stored_key.algorithm,
	// 				&stored_key.master_key,
	// 				&[],
	// 			) {
	// 				Ok(Protected::new(to_array::<KEY_LEN>(
	// 					decrypted_master_key.expose().clone(),
	// 				)?))
	// 			} else {
	// 				Err(Error::IncorrectPassword)
	// 			}?;

	// 			let master_key_nonce = generate_nonce(stored_key.algorithm);

	// 			// Encrypt the master key with the us			let encrypted_master_key: [u8; ENCRYPTED_KEY_LEN] = to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
	// 				new_root_key.clone(),
	// 				&master_key_nonce,
	// 				stored_key.algorithm,
	// 				master_key.expose(),
	// 				&[],
	// 			)?)?;

	// 			stored_key.master_key = encrypted_master_key;
	// 			stored_key.master_key_nonce = master_key_nonce;

	// 			Ok(stored_key)
	// 		})
	// 		.collect();

	// 	// should use ? above
	// 	let updated_keystore = updated_keystore?;

	// 	// Clear the current keystore and update it with our re-encrypted keystore
	// 	self.empty_keystore();
	// 	self.populate_keystore(updated_keystore.clone())?;

	// 	// Create a new verification key
	// 	let uuid = uuid::Uuid::nil();
	// 	let master_key = generate_master_key();
	// 	let master_key_nonce = generate_nonce(algorithm);

	// 	// Encrypt the master key with the hashed master password
	// let encrypted_master_key: [u8; ENCRYPTED_KEY_LEN] = to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
	// 		hashed_password,
	// 		&master_key_nonce,
	// 		algorithm,
	// 		master_key.expose(),
	// 		&[],
	// 	)?)?;

	// 	let verification_key = StoredKey {
	// 		uuid,
	// 		algorithm,
	// 		hashing_algorithm,
	// 		content_salt: [0u8; SALT_LEN],
	// 		master_key: encrypted_master_key,
	// 		master_key_nonce,
	// 		key_nonce: Vec::new(),
	// 		key: Vec::new(),
	// 	};

	// let secret_key = Self::format_secret_key(&salt);

	// 	let mpc_bundle = MasterPasswordChangeBundle {
	// 		verification_key,
	// 		secret_key,
	// 		updated_keystore,
	// 	};

	// 	// Update the internal verification key, and then set the master password
	// 	*self.verification_key.lock()? = Some(mpc_bundle.verification_key.clone());
	// 	self.set_master_password(master_password, mpc_bundle.secret_key.clone())?;

	// 	// Return the verification key so it can be written to Prisma and return the secret key so it can be shown to the user
	// 	Ok(mpc_bundle)
	// }

	/// Used internally to convert from a hex-encoded `Protected<String>` to a `Protected<[u8; SALT_LEN]>` in a secretive manner.
	///
	/// If the secret key is wrong (not base64 or not the correct length), a filler secret key will be inserted secretly.
	#[allow(clippy::needless_pass_by_value)]
	fn convert_secret_key_string(secret_key: Protected<String>) -> Protected<[u8; SALT_LEN]> {
		let mut secret_key_clean = secret_key.expose().clone();
		secret_key_clean.retain(|c| c != '-' && !c.is_whitespace());

		let secret_key = if let Ok(secret_key) = hex::decode(secret_key_clean) {
			secret_key
		} else {
			Vec::new()
		};

		// we shouldn't be letting on to *what* failed so we use a random secret key here if it's still invalid
		// could maybe do this better (and make use of the subtle crate)
		if let Ok(secret_key) = to_array(secret_key) {
			Protected::new(secret_key)
		} else {
			Protected::new(generate_salt())
		}
	}

	/// This re-encrypts master keys so they can be imported from a key backup into the current key manager.
	///
	/// It returns a `Vec<StoredKey>` so they can be written to Prisma
	#[allow(clippy::needless_pass_by_value)]
	pub fn import_keystore_backup(
		&self,
		master_password: Protected<String>, // at the time of the backup
		secret_key: Protected<String>,      // at the time of the backup
		stored_keys: &[StoredKey],          // from the backup
	) -> Result<Vec<StoredKey>> {
		// this backup should contain a verification key, which will tell us the algorithm+hashing algorithm
		let master_password = Protected::new(master_password.expose().as_bytes().to_vec());
		let secret_key = Self::convert_secret_key_string(secret_key);

		let mut verification_key = None;

		let keys: Vec<StoredKey> = stored_keys
			.iter()
			.filter_map(|key| {
				if key.uuid.is_nil() {
					verification_key = Some(key.clone());
					None
				} else {
					Some(key.clone())
				}
			})
			.collect();

		let verification_key = if let Some(verification_key) = verification_key {
			verification_key
		} else {
			return Err(Error::NoVerificationKey);
		};

		let hashed_master_password = verification_key
			.hashing_algorithm
			.hash(master_password, *secret_key.expose())?;

		// decrypt the root key's KEK
		let master_key = StreamDecryption::decrypt_bytes(
			hashed_master_password,
			&verification_key.master_key_nonce,
			verification_key.algorithm,
			&verification_key.master_key,
			&[],
		)?;

		// get the root key from the backup
		let root_key = StreamDecryption::decrypt_bytes(
			Protected::new(to_array(master_key.expose().clone())?),
			&verification_key.key_nonce,
			verification_key.algorithm,
			&verification_key.key,
			&[],
		)?;

		let root_key = Protected::new(to_array(root_key.expose().clone())?);

		let mut reencrypted_keys = Vec::new();

		for key in keys {
			if self.keystore.contains_key(&key.uuid) {
				continue;
			}

			// could check the key material itself? if they match, attach the content salt

			// decrypt the key's master key
			let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
				root_key.clone(),
				&key.master_key_nonce,
				key.algorithm,
				&key.master_key,
				&[],
			) {
				Ok(Protected::new(to_array::<KEY_LEN>(
					decrypted_master_key.expose().clone(),
				)?))
			} else {
				Err(Error::IncorrectPassword)
			}?;

			// generate a new nonce
			let master_key_nonce = generate_nonce(key.algorithm);

			// encrypt the master key with the current root key
			let encrypted_master_key =
				to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
					self.get_root_key()?,
					&master_key_nonce,
					key.algorithm,
					master_key.expose(),
					&[],
				)?)?;

			let mut updated_key = key.clone();
			updated_key.master_key_nonce = master_key_nonce;
			updated_key.master_key = encrypted_master_key;

			reencrypted_keys.push(updated_key.clone());
			self.keystore.insert(updated_key.uuid, updated_key);
		}

		Ok(reencrypted_keys)
	}

	/// This requires both the master password and the secret key
	///
	/// The master password and secret key are hashed together.
	/// This minimises the risk of an attacker obtaining the master password, as both of these are required to unlock the vault (and both should be stored separately).
	///
	/// Both values need to be correct, otherwise this function will return a generic error.
	#[allow(clippy::needless_pass_by_value)]
	pub fn set_master_password(
		&self,
		master_password: Protected<String>,
		secret_key: Protected<String>,
	) -> Result<()> {
		let verification_key = match &*self.verification_key.lock()? {
			Some(k) => Ok(k.clone()),
			None => Err(Error::NoVerificationKey),
		}?;
		let master_password = Protected::new(master_password.expose().as_bytes().to_vec());
		let secret_key = Self::convert_secret_key_string(secret_key);

		let hashed_master_password = verification_key
			.hashing_algorithm
			.hash(master_password, *secret_key.expose())?;

		// Decrypt the StoredKey's master key using the user's hashed password
		if let Ok(master_key) = StreamDecryption::decrypt_bytes(
			hashed_master_password,
			&verification_key.master_key_nonce,
			verification_key.algorithm,
			&verification_key.master_key,
			&[],
		) {
			// decrypt the root key and set that as the master password
			*self.root_key.lock()? = Some(Protected::new(to_array(
				StreamDecryption::decrypt_bytes(
					Protected::new(to_array(master_key.expose().clone())?),
					&verification_key.key_nonce,
					verification_key.algorithm,
					&verification_key.key,
					&[],
				)?
				.expose()
				.clone(),
			)?));
			Ok(())
		} else {
			Err(Error::IncorrectKeymanagerDetails)
		}
	}

	/// This function is for removing a previously-added master password
	pub fn clear_root_key(&self) -> Result<()> {
		*self.root_key.lock()? = None;

		Ok(())
	}

	pub fn keystore_contains(&self, uuid: Uuid) -> bool {
		self.keystore.contains_key(&uuid)
	}

	pub fn keymount_contains(&self, uuid: Uuid) -> bool {
		self.keymount.contains_key(&uuid)
	}

	/// This function is used for seeing if the key manager has a master password.
	///
	/// Technically this checks for the root key, but it makes no difference to the front end.
	pub fn has_master_password(&self) -> Result<bool> {
		Ok(self.root_key.lock()?.is_some())
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
	pub fn compare_keystore(&self, supplied_keys: &[StoredKey]) -> Result<()> {
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
	pub fn unmount(&self, uuid: Uuid) -> Result<()> {
		if self.keymount.contains_key(&uuid) {
			self.keymount.remove(&uuid);
			Ok(())
		} else {
			Err(Error::KeyNotMounted)
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
	pub fn mount(&self, uuid: Uuid) -> Result<()> {
		if self.keymount.get(&uuid).is_some() {
			return Err(Error::KeyAlreadyMounted);
		}

		match self.keystore.get(&uuid) {
			Some(stored_key) => {
				// Decrypt the StoredKey's master key using the root key
				let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
					self.get_root_key()?,
					&stored_key.master_key_nonce,
					stored_key.algorithm,
					&stored_key.master_key,
					&[],
				) {
					Ok(Protected::new(to_array(
						decrypted_master_key.expose().clone(),
					)?))
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
					.hash(key, stored_key.content_salt)?;

				// Construct the MountedKey and insert it into the Keymount
				let mounted_key = MountedKey {
					uuid: stored_key.uuid,
					hashed_key,
				};

				self.keymount.insert(uuid, mounted_key);

				Ok(())
			}
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is used for getting the key itself, from a given UUID.
	///
	/// The master password/salt needs to be present, so we are able to decrypt the key itself from the stored key.
	pub fn get_key(&self, uuid: Uuid) -> Result<Protected<Vec<u8>>> {
		match self.keystore.get(&uuid) {
			Some(stored_key) => {
				// Decrypt the StoredKey's master key using the root key
				let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
					self.get_root_key()?,
					&stored_key.master_key_nonce,
					stored_key.algorithm,
					&stored_key.master_key,
					&[],
				) {
					Ok(Protected::new(to_array(
						decrypted_master_key.expose().clone(),
					)?))
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

				Ok(key)
			}
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing the internal keymount.
	///
	/// We could add a log to this, so that the user can view accesses
	pub fn access_keymount(&self, uuid: Uuid) -> Result<MountedKey> {
		match self.keymount.get(&uuid) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	/// This function is for accessing a `StoredKey`.
	pub fn access_keystore(&self, uuid: Uuid) -> Result<StoredKey> {
		match self.keystore.get(&uuid) {
			Some(key) => Ok(key.clone()),
			None => Err(Error::KeyNotFound),
		}
	}

	pub fn change_automount_status(&self, uuid: Uuid, status: bool) -> Result<()> {
		let updated_key = match self.keystore.get(&uuid) {
			Some(key) => {
				let mut updated_key = key.clone();
				updated_key.automount = status;
				Ok(updated_key)
			}
			None => Err(Error::KeyNotFound),
		}?;

		self.keystore.remove(&uuid);
		self.keystore.insert(uuid, updated_key);
		Ok(())
	}

	/// This function is for getting an entire collection of hashed keys.
	///
	/// These are ideal for passing over to decryption functions, as each decryption attempt is negligible, performance wise.
	///
	/// This means we don't need to keep super specific track of which key goes to which file, and we can just throw all of them at it.
	#[must_use]
	pub fn enumerate_hashed_keys(&self) -> Vec<Protected<[u8; KEY_LEN]>> {
		self.keymount
			.iter()
			.map(|mounted_key| mounted_key.hashed_key.clone())
			.collect::<Vec<Protected<[u8; KEY_LEN]>>>()
	}

	/// This function is for converting a memory-only key to a saved key which syncs to the library.
	///
	/// The returned value needs to be written to the database.
	pub fn save_to_database(&self, uuid: Uuid) -> Result<StoredKey> {
		if !self.is_memory_only(uuid)? {
			return Err(Error::KeyNotMemoryOnly);
		}

		let updated_key = match self.keystore.get(&uuid) {
			Some(key) => {
				let mut updated_key = key.clone();
				updated_key.memory_only = false;
				Ok(updated_key)
			}
			None => Err(Error::KeyNotFound),
		}?;

		self.keystore.remove(&uuid);
		self.keystore.insert(uuid, updated_key.clone());

		Ok(updated_key)
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
	///
	/// You may optionally provide a content salt, if not one will be generated.
	#[allow(clippy::needless_pass_by_value)]
	pub fn add_to_keystore(
		&self,
		key: Protected<Vec<u8>>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		memory_only: bool,
		automount: bool,
		content_salt: Option<[u8; SALT_LEN]>,
	) -> Result<Uuid> {
		let uuid = uuid::Uuid::new_v4();

		// Generate items we'll need for encryption
		let key_nonce = generate_nonce(algorithm);
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let content_salt = if let Some(content_salt) = content_salt {
			content_salt
		} else {
			generate_salt()
		};

		// Encrypt the master key with the user's hashed password
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(StreamEncryption::encrypt_bytes(
			self.get_root_key()?,
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
			content_salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce,
			key: encrypted_key,
			memory_only,
			automount,
		};

		// Insert it into the Keystore
		self.keystore.insert(stored_key.uuid, stored_key);

		// Return the ID so it can be identified
		Ok(uuid)
	}
}
