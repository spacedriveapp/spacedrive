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

use std::sync::Arc;

use tokio::sync::Mutex;

// use crate::primitives::{
// 	derive_key, generate_master_key, generate_nonce, generate_salt, to_array, EncryptedKey, Key,
// 	OnboardingConfig, Salt, KEY_LEN, LATEST_STORED_KEY, MASTER_PASSWORD_CONTEXT, ROOT_KEY_CONTEXT,
// };
use crate::{
	crypto::stream::{Algorithm, StreamDecryption, StreamEncryption},
	primitives::{
		derive_key, generate_master_key, generate_nonce, generate_salt, generate_secret_key,
		to_array, EncryptedKey, Key, OnboardingConfig, Salt, SecretKey, APP_IDENTIFIER,
		ENCRYPTED_KEY_LEN, KEY_LEN, LATEST_STORED_KEY, MASTER_PASSWORD_CONTEXT, ROOT_KEY_CONTEXT,
		SECRET_KEY_IDENTIFIER,
	},
	Error, Protected, Result,
};

use dashmap::{DashMap, DashSet};
use uuid::Uuid;

#[cfg(feature = "serde")]
use serde_big_array::BigArray;

use super::{
	hashing::HashingAlgorithm,
	keyring::{Identifier, KeyringInterface},
};

/// This is a stored key, and can be freely written to Prisma/another database.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct StoredKey {
	pub uuid: Uuid, // uuid for identification. shared with mounted keys
	pub version: StoredKeyVersion,
	pub key_type: StoredKeyType,
	pub algorithm: Algorithm, // encryption algorithm for encrypting the master key. can be changed (requires a re-encryption though)
	pub hashing_algorithm: HashingAlgorithm, // hashing algorithm used for hashing the key with the content salt
	pub content_salt: Salt,
	#[cfg_attr(feature = "serde", serde(with = "BigArray"))] // salt used for file data
	pub master_key: EncryptedKey, // this is for encrypting the `key`
	pub master_key_nonce: Vec<u8>, // nonce for encrypting the master key
	pub key_nonce: Vec<u8>,        // nonce used for encrypting the main key
	pub key: Vec<u8>, // encrypted. the key stored in spacedrive (e.g. generated 64 char key)
	pub salt: Salt,
	pub memory_only: bool,
	pub automount: bool,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub enum StoredKeyType {
	User,
	Root,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub enum StoredKeyVersion {
	V1,
}

/// This is a mounted key, and needs to be kept somewhat hidden.
///
/// This contains the plaintext key, and the same key hashed with the content salt.
#[derive(Clone)]
pub struct MountedKey {
	pub uuid: Uuid,                 // used for identification. shared with stored keys
	pub hashed_key: Protected<Key>, // this is hashed with the content salt, for instant access
}

/// This is the key manager itself.
///
/// It contains the keystore, the keymount, the master password and the default key.
///
/// Use the associated functions to interact with it.
pub struct KeyManager {
	root_key: Mutex<Option<Protected<Key>>>, // the root key for the vault
	verification_key: Mutex<Option<StoredKey>>,
	keystore: DashMap<Uuid, StoredKey>,
	keymount: DashMap<Uuid, MountedKey>,
	default: Mutex<Option<Uuid>>,
	mounting_queue: DashSet<Uuid>,
	keyring: Option<Arc<Mutex<KeyringInterface>>>,
}

/// The `KeyManager` functions should be used for all key-related management.
impl KeyManager {
	/// Initialize the Key Manager with `StoredKeys` retrieved from Prisma
	pub async fn new(stored_keys: Vec<StoredKey>) -> Result<Self> {
		let keyring = KeyringInterface::new()
			.map(|k| Arc::new(Mutex::new(k)))
			.ok();

		let keymanager = Self {
			root_key: Mutex::new(None),
			verification_key: Mutex::new(None),
			keystore: DashMap::new(),
			keymount: DashMap::new(),
			default: Mutex::new(None),
			mounting_queue: DashSet::new(),
			keyring,
		};

		keymanager.populate_keystore(stored_keys).await?;

		Ok(keymanager)
	}

	#[allow(clippy::needless_pass_by_value)]
	fn format_secret_key(secret_key: Protected<SecretKey>) -> Protected<String> {
		let hex_string: String = hex::encode_upper(secret_key.expose())
			.chars()
			.enumerate()
			.map(|(i, c)| {
				if (i + 1) % 6 == 0 && i != 35 {
					c.to_string() + "-"
				} else {
					c.to_string()
				}
			})
			.into_iter()
			.collect();

		Protected::new(hex_string)
	}

	// A returned error here should be treated as `false`
	pub async fn keyring_contains(&self, library_uuid: Uuid, usage: String) -> Result<()> {
		self.get_keyring()?.lock().await.retrieve(Identifier {
			application: APP_IDENTIFIER,
			library_uuid: &library_uuid.to_string(),
			usage: &usage,
		})?;

		Ok(())
	}

	pub async fn keyring_retrieve(
		&self,
		library_uuid: Uuid,
		usage: String,
	) -> Result<Protected<String>> {
		let value = self.get_keyring()?.lock().await.retrieve(Identifier {
			application: APP_IDENTIFIER,
			library_uuid: &library_uuid.to_string(),
			usage: &usage,
		})?;

		Ok(Protected::new(String::from_utf8(value.expose().clone())?))
	}

	/// This checks to see if the keyring is active, and if the keyring has a valid secret key.
	///
	/// For a secret key to be considered valid, it must be 18 bytes encoded in hex.
	///
	/// We can use this to detect if a secret key is technically present in the keyring, but not valid/has been tampered with.
	pub async fn keyring_contains_valid_secret_key(&self, library_uuid: Uuid) -> Result<()> {
		let secret_key = self
			.keyring_retrieve(library_uuid, SECRET_KEY_IDENTIFIER.to_string())
			.await?;

		let mut secret_key_sanitized = secret_key.expose().clone();
		secret_key_sanitized.retain(|c| c != '-' && !c.is_whitespace());

		if hex::decode(secret_key_sanitized)
			.map_err(|_| Error::IncorrectPassword)?
			.len() != 18
		{
			return Err(Error::IncorrectPassword);
		}

		Ok(())
	}

	async fn keyring_insert(
		&self,
		library_uuid: Uuid,
		usage: String,
		value: Protected<String>,
	) -> Result<()> {
		self.get_keyring()?.lock().await.insert(
			Identifier {
				application: APP_IDENTIFIER,
				library_uuid: &library_uuid.to_string(),
				usage: &usage,
			},
			value,
		)?;

		Ok(())
	}

	fn get_keyring(&self) -> Result<Arc<Mutex<KeyringInterface>>> {
		self.keyring
			.as_ref()
			.map_or(Err(Error::KeyringNotSupported), |k| Ok(k.clone()))
	}

	/// This should be used to generate everything for the user during onboarding.
	///
	/// This will create a master password (a 7-word diceware passphrase), and a secret key (16 bytes, hex encoded)
	///
	/// It will also generate a verification key, which should be written to the database.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn onboarding(config: OnboardingConfig, library_uuid: Uuid) -> Result<StoredKey> {
		let content_salt = generate_salt();
		let secret_key = generate_secret_key();

		dbg!(Self::format_secret_key(secret_key.clone()).expose());

		let algorithm = config.algorithm;
		let hashing_algorithm = config.hashing_algorithm;

		// Hash the master password
		let hashed_password = hashing_algorithm.hash(
			Protected::new(config.password.expose().as_bytes().to_vec()),
			content_salt,
			Some(secret_key.clone()),
		)?;

		let salt = generate_salt();

		// Generate items we'll need for encryption
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let root_key = generate_master_key();
		let root_key_nonce = generate_nonce(algorithm);

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(
			StreamEncryption::encrypt_bytes(
				derive_key(hashed_password, salt, MASTER_PASSWORD_CONTEXT),
				&master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			&root_key_nonce,
			algorithm,
			root_key.expose(),
			&[],
		)
		.await?;

		// attempt to insert into the OS keyring
		// can ignore false here as we want to silently error
		if let Ok(keyring) = KeyringInterface::new() {
			let identifier = Identifier {
				application: APP_IDENTIFIER,
				library_uuid: &library_uuid.to_string(),
				usage: SECRET_KEY_IDENTIFIER,
			};

			keyring
				.insert(identifier, Self::format_secret_key(secret_key))
				.ok();
		}

		let verification_key = StoredKey {
			uuid: Uuid::new_v4(),
			version: LATEST_STORED_KEY,
			key_type: StoredKeyType::Root,
			algorithm,
			hashing_algorithm,
			content_salt, // salt used for hashing
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce: root_key_nonce,
			key: encrypted_root_key,
			salt, // salt used for key derivation
			memory_only: false,
			automount: false,
		};

		Ok(verification_key)
	}

	/// This function should be used to populate the keystore with multiple stored keys at a time.
	///
	/// It's suitable for when you created the key manager without populating it.
	///
	/// This also detects the nil-UUID master passphrase verification key
	pub async fn populate_keystore(&self, stored_keys: Vec<StoredKey>) -> Result<()> {
		for key in stored_keys {
			if self.keystore.contains_key(&key.uuid) {
				continue;
			}

			if key.key_type == StoredKeyType::Root {
				*self.verification_key.lock().await = Some(key);
			} else {
				self.keystore.insert(key.uuid, key);
			}
		}

		Ok(())
	}

	/// This function removes a key from the keystore, the keymount and it's unset as the default.
	pub async fn remove_key(&self, uuid: Uuid) -> Result<()> {
		if self.keystore.contains_key(&uuid) {
			// if key is default, clear it
			// do this manually to prevent deadlocks
			let mut default = self.default.lock().await;
			if *default == Some(uuid) {
				*default = None;
			}
			drop(default);

			// unmount if mounted
			self.keymount
				.contains_key(&uuid)
				.then(|| self.keymount.remove(&uuid));

			// remove from keystore
			self.keystore.remove(&uuid);
		}

		Ok(())
	}

	#[allow(clippy::needless_pass_by_value)]
	pub async fn change_master_password(
		&self,
		master_password: Protected<String>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		library_uuid: Uuid,
	) -> Result<StoredKey> {
		let secret_key = generate_secret_key();
		let content_salt = generate_salt();

		dbg!(Self::format_secret_key(secret_key.clone()).expose());

		let hashed_password = hashing_algorithm.hash(
			Protected::new(master_password.expose().as_bytes().to_vec()),
			content_salt,
			Some(secret_key.clone()),
		)?;

		// Generate items we'll need for encryption
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let root_key = self.get_root_key().await?;
		let root_key_nonce = generate_nonce(algorithm);

		let salt = generate_salt();

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(
			StreamEncryption::encrypt_bytes(
				derive_key(hashed_password, salt, MASTER_PASSWORD_CONTEXT),
				&master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			&root_key_nonce,
			algorithm,
			root_key.expose(),
			&[],
		)
		.await?;

		// will update if it's already present
		self.keyring_insert(
			library_uuid,
			SECRET_KEY_IDENTIFIER.to_string(),
			Self::format_secret_key(secret_key),
		)
		.await
		.ok();

		let verification_key = StoredKey {
			uuid: Uuid::new_v4(),
			version: LATEST_STORED_KEY,
			key_type: StoredKeyType::Root,
			algorithm,
			hashing_algorithm,
			content_salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce: root_key_nonce,
			key: encrypted_root_key,
			salt,
			memory_only: false,
			automount: false,
		};

		*self.verification_key.lock().await = Some(verification_key.clone());

		Ok(verification_key)
	}

	/// This re-encrypts master keys so they can be imported from a key backup into the current key manager.
	///
	/// It returns a `Vec<StoredKey>` so they can be written to Prisma
	#[allow(clippy::needless_pass_by_value)]
	pub async fn import_keystore_backup(
		&self,
		master_password: Protected<String>, // at the time of the backup
		secret_key: Protected<String>,      // at the time of the backup
		stored_keys: &[StoredKey],          // from the backup
	) -> Result<Vec<StoredKey>> {
		// this backup should contain a verification key, which will tell us the algorithm+hashing algorithm
		let secret_key = Self::convert_secret_key_string(secret_key);

		let mut old_verification_key = None;

		let keys: Vec<StoredKey> = stored_keys
			.iter()
			.filter_map(|key| {
				if key.key_type == StoredKeyType::Root {
					old_verification_key = Some(key.clone());
					None
				} else {
					Some(key.clone())
				}
			})
			.collect();

		let old_verification_key = old_verification_key.ok_or(Error::NoVerificationKey)?;

		let old_root_key = match old_verification_key.version {
			StoredKeyVersion::V1 => {
				let hashed_password = old_verification_key.hashing_algorithm.hash(
					Protected::new(master_password.expose().as_bytes().to_vec()),
					old_verification_key.content_salt,
					Some(secret_key),
				)?;

				// decrypt the root key's KEK
				let master_key = StreamDecryption::decrypt_bytes(
					derive_key(
						hashed_password,
						old_verification_key.salt,
						MASTER_PASSWORD_CONTEXT,
					),
					&old_verification_key.master_key_nonce,
					old_verification_key.algorithm,
					&old_verification_key.master_key,
					&[],
				)
				.await?;

				// get the root key from the backup
				let old_root_key = StreamDecryption::decrypt_bytes(
					Protected::new(to_array(master_key.into_inner())?),
					&old_verification_key.key_nonce,
					old_verification_key.algorithm,
					&old_verification_key.key,
					&[],
				)
				.await?;

				Protected::new(to_array(old_root_key.into_inner())?)
			}
		};

		let mut reencrypted_keys = Vec::new();

		for key in keys {
			if self.keystore.contains_key(&key.uuid) {
				continue;
			}

			match key.version {
				StoredKeyVersion::V1 => {
					// decrypt the key's master key
					let master_key = StreamDecryption::decrypt_bytes(
						derive_key(old_root_key.clone(), key.salt, ROOT_KEY_CONTEXT),
						&key.master_key_nonce,
						key.algorithm,
						&key.master_key,
						&[],
					)
					.await
					.map_or(Err(Error::IncorrectPassword), |v| {
						Ok(Protected::new(to_array::<KEY_LEN>(v.into_inner())?))
					})?;

					// generate a new nonce
					let master_key_nonce = generate_nonce(key.algorithm);

					let salt = generate_salt();

					// encrypt the master key with the current root key
					let encrypted_master_key = to_array(
						StreamEncryption::encrypt_bytes(
							derive_key(self.get_root_key().await?, salt, ROOT_KEY_CONTEXT),
							&master_key_nonce,
							key.algorithm,
							master_key.expose(),
							&[],
						)
						.await?,
					)?;

					let mut updated_key = key.clone();
					updated_key.master_key_nonce = master_key_nonce;
					updated_key.master_key = encrypted_master_key;
					updated_key.salt = salt;

					reencrypted_keys.push(updated_key.clone());
					self.keystore.insert(updated_key.uuid, updated_key);
				}
			}
		}

		Ok(reencrypted_keys)
	}

	/// This is used for unlocking the key manager, and requires both the master password and the secret key.
	///
	/// The master password and secret key are hashed together.
	///
	/// Only provide the secret key if it should not/can not be sourced from an OS keychain (e.g. web, OS keychains not enabled/available, etc).
	///
	/// This minimises the risk of an attacker obtaining the master password, as both of these are required to unlock the vault (and both should be stored separately).
	///
	/// Both values need to be correct, otherwise this function will return a generic error.
	///
	/// The invalidate function is to handle query invalidation, so that the UI updates correctly. Leave it blank if this isn't required.
	///
	/// Note: The invalidation function is ran after updating the queue both times, so it isn't required externally.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn unlock<F>(
		&self,
		master_password: Protected<String>,
		provided_secret_key: Option<Protected<String>>,
		library_uuid: Uuid,
		invalidate: F,
	) -> Result<()>
	where
		F: Fn() + Send,
	{
		let verification_key = (*self.verification_key.lock().await)
			.as_ref()
			.map_or(Err(Error::NoVerificationKey), |k| Ok(k.clone()))?;

		if self.is_unlocked().await? {
			return Err(Error::KeyAlreadyMounted);
		} else if self.is_queued(verification_key.uuid) {
			return Err(Error::KeyAlreadyQueued);
		}

		let secret_key = if let Some(secret_key) = provided_secret_key.clone() {
			Self::convert_secret_key_string(secret_key)
		} else {
			self.get_keyring()?
				.lock()
				.await
				.retrieve(Identifier {
					application: APP_IDENTIFIER,
					library_uuid: &library_uuid.to_string(),
					usage: SECRET_KEY_IDENTIFIER,
				})
				.map(|x| Protected::new(String::from_utf8(x.expose().clone()).unwrap()))
				.map(Self::convert_secret_key_string)?
		};

		self.mounting_queue.insert(verification_key.uuid);
		invalidate();

		match verification_key.version {
			StoredKeyVersion::V1 => {
				let hashed_password = verification_key
					.hashing_algorithm
					.hash(
						Protected::new(master_password.expose().as_bytes().to_vec()),
						verification_key.content_salt,
						Some(secret_key),
					)
					.map_err(|e| {
						self.remove_from_queue(verification_key.uuid).ok();
						e
					})?;

				let master_key = StreamDecryption::decrypt_bytes(
					derive_key(
						hashed_password,
						verification_key.salt,
						MASTER_PASSWORD_CONTEXT,
					),
					&verification_key.master_key_nonce,
					verification_key.algorithm,
					&verification_key.master_key,
					&[],
				)
				.await
				.map_err(|_| {
					self.remove_from_queue(verification_key.uuid).ok();
					Error::IncorrectKeymanagerDetails
				})?;

				*self.root_key.lock().await = Some(Protected::new(
					to_array(
						StreamDecryption::decrypt_bytes(
							Protected::new(to_array(master_key.into_inner())?),
							&verification_key.key_nonce,
							verification_key.algorithm,
							&verification_key.key,
							&[],
						)
						.await?
						.expose()
						.clone(),
					)
					.map_err(|e| {
						self.remove_from_queue(verification_key.uuid).ok();
						e
					})?,
				));

				self.remove_from_queue(verification_key.uuid)?;
			}
		}

		if let Some(secret_key) = provided_secret_key {
			// converting twice ensures it's formatted correctly
			self.keyring_insert(
				library_uuid,
				SECRET_KEY_IDENTIFIER.to_string(),
				Self::format_secret_key(Self::convert_secret_key_string(secret_key)),
			)
			.await
			.ok();
		}

		invalidate();

		Ok(())
	}

	/// This function does not return a value by design.
	///
	/// Once a key is mounted, access it with `KeyManager::access()`
	///
	/// This is to ensure that only functions which require access to the mounted key receive it.
	///
	/// We could add a log to this, so that the user can view mounts
	pub async fn mount(&self, uuid: Uuid) -> Result<()> {
		if self.keymount.get(&uuid).is_some() {
			return Err(Error::KeyAlreadyMounted);
		} else if self.is_queued(uuid) {
			return Err(Error::KeyAlreadyQueued);
		}

		if let Some(stored_key) = self.keystore.get(&uuid) {
			match stored_key.version {
				StoredKeyVersion::V1 => {
					self.mounting_queue.insert(uuid);

					let master_key = StreamDecryption::decrypt_bytes(
						derive_key(
							self.get_root_key().await?,
							stored_key.salt,
							ROOT_KEY_CONTEXT,
						),
						&stored_key.master_key_nonce,
						stored_key.algorithm,
						&stored_key.master_key,
						&[],
					)
					.await
					.map_or_else(
						|_| {
							self.remove_from_queue(uuid).ok();
							Err(Error::IncorrectPassword)
						},
						|v| Ok(Protected::new(to_array(v.into_inner())?)),
					)?;
					// Decrypt the StoredKey using the decrypted master key
					let key = StreamDecryption::decrypt_bytes(
						master_key,
						&stored_key.key_nonce,
						stored_key.algorithm,
						&stored_key.key,
						&[],
					)
					.await
					.map_err(|e| {
						self.remove_from_queue(uuid).ok();
						e
					})?;

					// Hash the key once with the parameters/algorithm the user selected during first mount
					let hashed_key = stored_key
						.hashing_algorithm
						.hash(key, stored_key.content_salt, None)
						.map_err(|e| {
							self.remove_from_queue(uuid).ok();
							e
						})?;

					self.keymount.insert(
						uuid,
						MountedKey {
							uuid: stored_key.uuid,
							hashed_key,
						},
					);

					self.remove_from_queue(uuid)?;
				}
			}

			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This function is used for getting the key value itself, from a given UUID.
	///
	/// The master password/salt needs to be present, so we are able to decrypt the key itself from the stored key.
	pub async fn get_key(&self, uuid: Uuid) -> Result<Protected<String>> {
		if let Some(stored_key) = self.keystore.get(&uuid) {
			let master_key = StreamDecryption::decrypt_bytes(
				derive_key(
					self.get_root_key().await?,
					stored_key.salt,
					ROOT_KEY_CONTEXT,
				),
				&stored_key.master_key_nonce,
				stored_key.algorithm,
				&stored_key.master_key,
				&[],
			)
			.await
			.map_or(Err(Error::IncorrectPassword), |k| {
				Ok(Protected::new(to_array(k.into_inner())?))
			})?;

			// Decrypt the StoredKey using the decrypted master key
			let key = StreamDecryption::decrypt_bytes(
				master_key,
				&stored_key.key_nonce,
				stored_key.algorithm,
				&stored_key.key,
				&[],
			)
			.await?;

			Ok(Protected::new(String::from_utf8(key.expose().clone())?))
		} else {
			Err(Error::KeyNotFound)
		}
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
	/// You may optionally provide a content salt, if not one will be generated (used primarily for password-based decryption)
	#[allow(clippy::needless_pass_by_value)]
	pub async fn add_to_keystore(
		&self,
		key: Protected<String>,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		memory_only: bool,
		automount: bool,
		content_salt: Option<Salt>,
	) -> Result<Uuid> {
		let uuid = Uuid::new_v4();

		// Generate items we'll need for encryption
		let key_nonce = generate_nonce(algorithm);
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);

		let content_salt = content_salt.map_or_else(generate_salt, |v| v);

		// salt used for the kdf
		let salt = generate_salt();

		// Encrypt the master key with a derived key (derived from the root key)
		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(
			StreamEncryption::encrypt_bytes(
				derive_key(self.get_root_key().await?, salt, ROOT_KEY_CONTEXT),
				&master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		// Encrypt the actual key (e.g. user-added/autogenerated, text-encodable)
		let encrypted_key = StreamEncryption::encrypt_bytes(
			master_key,
			&key_nonce,
			algorithm,
			key.expose().as_bytes(),
			&[],
		)
		.await?;

		// Insert it into the Keystore
		self.keystore.insert(
			uuid,
			StoredKey {
				uuid,
				version: LATEST_STORED_KEY,
				key_type: StoredKeyType::User,
				algorithm,
				hashing_algorithm,
				content_salt,
				master_key: encrypted_master_key,
				master_key_nonce,
				key_nonce,
				key: encrypted_key,
				salt,
				memory_only,
				automount,
			},
		);

		// Return the ID so it can be identified
		Ok(uuid)
	}

	#[allow(clippy::needless_pass_by_value)]
	fn convert_secret_key_string(secret_key: Protected<String>) -> Protected<SecretKey> {
		let mut secret_key_sanitized = secret_key.expose().clone();
		secret_key_sanitized.retain(|c| c != '-' && !c.is_whitespace());

		// we shouldn't be letting on to *what* failed so we use a random secret key here if it's still invalid
		// could maybe do this better (and make use of the subtle crate)

		let secret_key = hex::decode(secret_key_sanitized)
			.ok()
			.map_or(Vec::new(), |v| v);

		to_array(secret_key)
			.ok()
			.map_or_else(generate_secret_key, Protected::new)
	}

	/// This function is for accessing the internal keymount.
	///
	/// We could add a log to this, so that the user can view accesses
	pub fn access_keymount(&self, uuid: Uuid) -> Result<MountedKey> {
		self.keymount
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.clone()))
	}

	/// This function is for accessing a `StoredKey`.
	pub fn access_keystore(&self, uuid: Uuid) -> Result<StoredKey> {
		self.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.clone()))
	}

	/// This allows you to set the default key
	pub async fn set_default(&self, uuid: Uuid) -> Result<()> {
		if self.keystore.contains_key(&uuid) {
			*self.default.lock().await = Some(uuid);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This allows you to get the default key's ID
	pub async fn get_default(&self) -> Result<Uuid> {
		self.default.lock().await.ok_or(Error::NoDefaultKeySet)
	}

	/// This should ONLY be used internally.
	async fn get_root_key(&self) -> Result<Protected<Key>> {
		self.root_key
			.lock()
			.await
			.clone()
			.ok_or(Error::NoMasterPassword)
	}

	pub async fn get_verification_key(&self) -> Result<StoredKey> {
		self.verification_key
			.lock()
			.await
			.clone()
			.ok_or(Error::NoVerificationKey)
	}

	pub fn is_memory_only(&self, uuid: Uuid) -> Result<bool> {
		self.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.memory_only))
	}

	pub fn change_automount_status(&self, uuid: Uuid, status: bool) -> Result<()> {
		let updated_key = self
			.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| {
				let mut updated_key = v.clone();
				updated_key.automount = status;
				Ok(updated_key)
			})?;

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
	pub fn enumerate_hashed_keys(&self) -> Vec<Protected<Key>> {
		self.keymount
			.iter()
			.map(|mounted_key| mounted_key.hashed_key.clone())
			.collect::<Vec<Protected<Key>>>()
	}

	/// This function is for converting a memory-only key to a saved key which syncs to the library.
	///
	/// The returned value needs to be written to the database.
	pub fn sync_to_database(&self, uuid: Uuid) -> Result<StoredKey> {
		if !self.is_memory_only(uuid)? {
			return Err(Error::KeyNotMemoryOnly);
		}

		let updated_key = self
			.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| {
				let mut updated_key = v.clone();
				updated_key.memory_only = false;
				Ok(updated_key)
			})?;

		self.keystore.remove(&uuid);
		self.keystore.insert(uuid, updated_key.clone());

		Ok(updated_key)
	}

	/// This function is for removing a previously-added master password
	pub async fn clear_root_key(&self) -> Result<()> {
		*self.root_key.lock().await = None;

		Ok(())
	}

	/// This function is used for checking if the key manager is unlocked.
	pub async fn is_unlocked(&self) -> Result<bool> {
		Ok(self.root_key.lock().await.is_some())
	}

	/// This function is used for unmounting all keys at once.
	pub fn empty_keymount(&self) {
		// i'm unsure whether or not `.clear()` also calls drop
		// if it doesn't, we're going to need to find another way to call drop on these values
		// that way they will be zeroized and removed from memory fully
		self.keymount.clear();
	}

	/// This function is for unmounting a key from the key manager
	///
	/// This does not remove the key from the key store
	pub fn unmount(&self, uuid: Uuid) -> Result<()> {
		self.keymount.remove(&uuid).ok_or(Error::KeyNotMounted)?;

		Ok(())
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

	pub fn get_queue(&self) -> Vec<Uuid> {
		self.mounting_queue.iter().map(|u| *u).collect()
	}

	pub fn is_queued(&self, uuid: Uuid) -> bool {
		self.mounting_queue.contains(&uuid)
	}

	pub async fn is_unlocking(&self) -> Result<bool> {
		Ok(self
			.mounting_queue
			.contains(&self.get_verification_key().await?.uuid))
	}

	pub fn remove_from_queue(&self, uuid: Uuid) -> Result<()> {
		self.mounting_queue
			.remove(&uuid)
			.ok_or(Error::KeyNotQueued)?;

		Ok(())
	}
}
