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

use crate::{
	crypto::stream::{Algorithm, StreamDecryption, StreamEncryption},
	primitives::{
		types::{
			EncryptedKey, Key, Nonce, OnboardingConfig, Password, Salt, SecretKey, SecretKeyString,
		},
		APP_IDENTIFIER, LATEST_STORED_KEY, MASTER_PASSWORD_CONTEXT, ROOT_KEY_CONTEXT,
		SECRET_KEY_IDENTIFIER,
	},
	Error, Protected, Result,
};

use dashmap::{DashMap, DashSet};
use uuid::Uuid;

use super::{
	hashing::HashingAlgorithm,
	keyring::{Identifier, KeyringInterface},
};

/// This is a stored key, and can be freely written to Prisma/another database.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct StoredKey {
	pub uuid: Uuid, // uuid for identification. shared with mounted keys
	pub version: StoredKeyVersion,
	pub key_type: StoredKeyType,
	pub algorithm: Algorithm, // encryption algorithm for encrypting the master key. can be changed (requires a re-encryption though)
	pub hashing_algorithm: HashingAlgorithm, // hashing algorithm used for hashing the key with the content salt
	pub content_salt: Salt,
	pub master_key: EncryptedKey, // this is for encrypting the `key`
	pub master_key_nonce: Nonce,  // nonce for encrypting the master key
	pub key_nonce: Nonce,         // nonce used for encrypting the main key
	pub key: Vec<u8>, // encrypted. the password stored in spacedrive (e.g. generated 64 char key)
	pub salt: Salt,
	pub memory_only: bool,
	pub automount: bool,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum StoredKeyType {
	User,
	Root,
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum StoredKeyVersion {
	V1,
}

/// This is a mounted key, and needs to be kept somewhat hidden.
///
/// This contains the plaintext key, and the same key hashed with the content salt.
#[derive(Clone)]
pub struct MountedKey {
	pub uuid: Uuid,      // used for identification. shared with stored keys
	pub hashed_key: Key, // this is hashed with the content salt, for instant access
}

/// This is the key manager itself.
///
/// It contains the keystore, the keymount, the master password and the default key.
///
/// Use the associated functions to interact with it.
pub struct KeyManager {
	root_key: Mutex<Option<Key>>, // the root key for the vault
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

	// A returned error here should be treated as `false`
	pub async fn keyring_contains(&self, library_uuid: Uuid, usage: String) -> Result<()> {
		self.get_keyring()?.lock().await.retrieve(Identifier {
			application: APP_IDENTIFIER,
			library_uuid: &library_uuid.to_string(),
			usage: &usage,
		})?;

		Ok(())
	}

	// This verifies that the key manager is unlocked before continuing the calling function.
	pub async fn ensure_unlocked(&self) -> Result<()> {
		self.is_unlocked()
			.await
			.then_some(())
			.ok_or(Error::NotUnlocked)
	}

	// This verifies that the target key is not already queued before continuing the operation.
	pub fn ensure_not_queued(&self, uuid: Uuid) -> Result<()> {
		(!self.is_queued(uuid))
			.then_some(())
			.ok_or(Error::KeyAlreadyMounted)
	}

	// This verifies that the target key is not already mounted before continuing the operation.
	pub fn ensure_not_mounted(&self, uuid: Uuid) -> Result<()> {
		(!self.keymount.contains_key(&uuid))
			.then_some(())
			.ok_or(Error::KeyAlreadyMounted)
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
	/// For a secret key to be considered valid, it must be 18 bytes encoded in hex. It can be separated with `-`.
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
		value: SecretKeyString,
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
		let content_salt = Salt::generate();
		let secret_key = SecretKey::generate();

		dbg!(SecretKeyString::from(secret_key.clone()).expose());

		let algorithm = config.algorithm;
		let hashing_algorithm = config.hashing_algorithm;

		// Hash the master password
		let hashed_password = hashing_algorithm.hash(
			Protected::new(config.password.expose().as_bytes().to_vec()),
			content_salt,
			Some(secret_key.clone()),
		)?;

		let salt = Salt::generate();

		// Generate items we'll need for encryption
		let master_key = Key::generate();
		let master_key_nonce = Nonce::generate(algorithm)?;

		let root_key = Key::generate();
		let root_key_nonce = Nonce::generate(algorithm)?;

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = EncryptedKey::try_from(
			StreamEncryption::encrypt_bytes(
				Key::derive(hashed_password, salt, MASTER_PASSWORD_CONTEXT),
				master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			root_key_nonce,
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

			keyring.insert(identifier, secret_key.into()).ok();
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
		self.ensure_unlocked().await?;

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
		self.ensure_unlocked().await?;

		let secret_key = SecretKey::generate();
		let content_salt = Salt::generate();

		dbg!(SecretKeyString::from(secret_key.clone()).expose());

		let hashed_password = hashing_algorithm.hash(
			Protected::new(master_password.expose().as_bytes().to_vec()),
			content_salt,
			Some(secret_key.clone()),
		)?;

		// Generate items we'll need for encryption
		let master_key = Key::generate();
		let master_key_nonce = Nonce::generate(algorithm)?;

		let root_key = self.get_root_key().await?;
		let root_key_nonce = Nonce::generate(algorithm)?;

		let salt = Salt::generate();

		// Encrypt the master key with the hashed master password
		let encrypted_master_key = EncryptedKey::try_from(
			StreamEncryption::encrypt_bytes(
				Key::derive(hashed_password, salt, MASTER_PASSWORD_CONTEXT),
				master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		let encrypted_root_key = StreamEncryption::encrypt_bytes(
			master_key,
			root_key_nonce,
			algorithm,
			root_key.expose(),
			&[],
		)
		.await?;

		// will update if it's already present
		self.keyring_insert(
			library_uuid,
			SECRET_KEY_IDENTIFIER.to_string(),
			secret_key.into(),
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
		secret_key: SecretKeyString,        // at the time of the backup
		stored_keys: &[StoredKey],          // from the backup
	) -> Result<Vec<StoredKey>> {
		self.ensure_unlocked().await?;

		// this backup should contain a verification key, which will tell us the algorithm+hashing algorithm
		let secret_key = secret_key.into();

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
					Key::derive(
						hashed_password,
						old_verification_key.salt,
						MASTER_PASSWORD_CONTEXT,
					),
					old_verification_key.master_key_nonce,
					old_verification_key.algorithm,
					&old_verification_key.master_key,
					&[],
				)
				.await?;

				// get the root key from the backup
				let old_root_key = StreamDecryption::decrypt_bytes(
					Key::try_from(master_key)?,
					old_verification_key.key_nonce,
					old_verification_key.algorithm,
					&old_verification_key.key,
					&[],
				)
				.await?;

				Key::try_from(old_root_key)?
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
						Key::derive(old_root_key.clone(), key.salt, ROOT_KEY_CONTEXT),
						key.master_key_nonce,
						key.algorithm,
						&key.master_key,
						&[],
					)
					.await
					.map_or(Err(Error::IncorrectPassword), Key::try_from)?;

					// generate a new nonce
					let master_key_nonce = Nonce::generate(key.algorithm)?;

					let salt = Salt::generate();

					// encrypt the master key with the current root key
					let encrypted_master_key = EncryptedKey::try_from(
						StreamEncryption::encrypt_bytes(
							Key::derive(self.get_root_key().await?, salt, ROOT_KEY_CONTEXT),
							master_key_nonce,
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
	/// This minimizes the risk of an attacker obtaining the master password, as both of these are required to unlock the vault (and both should be stored separately).
	///
	/// Both values need to be correct, otherwise this function will return a generic error.
	///
	/// The invalidate function is to handle query invalidation, so that the UI updates correctly. Leave it blank if this isn't required.
	///
	/// Note: The invalidation function is ran after updating the queue both times, so it isn't required externally.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn unlock<F>(
		&self,
		master_password: Password,
		provided_secret_key: Option<SecretKeyString>,
		library_uuid: Uuid,
		invalidate: F,
	) -> Result<()>
	where
		F: Fn() + Send,
	{
		let verification_key = (*self.verification_key.lock().await)
			.as_ref()
			.map_or(Err(Error::NoVerificationKey), |k| Ok(k.clone()))?;

		self.ensure_not_queued(verification_key.uuid)?;

		let secret_key = if let Some(secret_key) = provided_secret_key.clone() {
			secret_key.into()
		} else {
			self.get_keyring()?
				.lock()
				.await
				.retrieve(Identifier {
					application: APP_IDENTIFIER,
					library_uuid: &library_uuid.to_string(),
					usage: SECRET_KEY_IDENTIFIER,
				})
				.map(|x| SecretKeyString::new(String::from_utf8(x.expose().clone()).unwrap()))?
				.into()
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
					Key::derive(
						hashed_password,
						verification_key.salt,
						MASTER_PASSWORD_CONTEXT,
					),
					verification_key.master_key_nonce,
					verification_key.algorithm,
					&verification_key.master_key,
					&[],
				)
				.await
				.map_err(|_| {
					self.remove_from_queue(verification_key.uuid).ok();
					Error::IncorrectPassword
				})?;

				*self.root_key.lock().await = Some(
					Key::try_from(
						StreamDecryption::decrypt_bytes(
							Key::try_from(master_key)?,
							verification_key.key_nonce,
							verification_key.algorithm,
							&verification_key.key,
							&[],
						)
						.await?,
					)
					.map_err(|e| {
						self.remove_from_queue(verification_key.uuid).ok();
						e
					})?,
				);

				self.remove_from_queue(verification_key.uuid)?;
			}
		}

		if let Some(secret_key) = provided_secret_key {
			// converting twice ensures it's formatted correctly
			self.keyring_insert(
				library_uuid,
				SECRET_KEY_IDENTIFIER.to_string(),
				SecretKeyString::from(SecretKey::from(secret_key)),
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
		self.ensure_unlocked().await?;
		self.ensure_not_mounted(uuid)?;
		self.ensure_not_queued(uuid)?;

		if let Some(stored_key) = self.keystore.get(&uuid) {
			match stored_key.version {
				StoredKeyVersion::V1 => {
					self.mounting_queue.insert(uuid);

					let master_key = StreamDecryption::decrypt_bytes(
						Key::derive(
							self.get_root_key().await?,
							stored_key.salt,
							ROOT_KEY_CONTEXT,
						),
						stored_key.master_key_nonce,
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
						Key::try_from,
					)?;
					// Decrypt the StoredKey using the decrypted master key
					let key = StreamDecryption::decrypt_bytes(
						master_key,
						stored_key.key_nonce,
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
	pub async fn get_key(&self, uuid: Uuid) -> Result<Password> {
		self.ensure_unlocked().await?;

		if let Some(stored_key) = self.keystore.get(&uuid) {
			let master_key = StreamDecryption::decrypt_bytes(
				Key::derive(
					self.get_root_key().await?,
					stored_key.salt,
					ROOT_KEY_CONTEXT,
				),
				stored_key.master_key_nonce,
				stored_key.algorithm,
				&stored_key.master_key,
				&[],
			)
			.await
			.map_or(Err(Error::IncorrectPassword), Key::try_from)?;

			// Decrypt the StoredKey using the decrypted master key
			let key = StreamDecryption::decrypt_bytes(
				master_key,
				stored_key.key_nonce,
				stored_key.algorithm,
				&stored_key.key,
				&[],
			)
			.await?;

			Ok(Password::new(String::from_utf8(key.expose().clone())?))
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
		key: Password,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		memory_only: bool,
		automount: bool,
		content_salt: Option<Salt>,
	) -> Result<Uuid> {
		self.ensure_unlocked().await?;

		let uuid = Uuid::new_v4();

		// Generate items we'll need for encryption
		let key_nonce = Nonce::generate(algorithm)?;
		let master_key = Key::generate();
		let master_key_nonce = Nonce::generate(algorithm)?;

		let content_salt = content_salt.map_or_else(Salt::generate, |v| v);

		// salt used for the kdf
		let salt = Salt::generate();

		// Encrypt the master key with a derived key (derived from the root key)
		let encrypted_master_key = EncryptedKey::try_from(
			StreamEncryption::encrypt_bytes(
				Key::derive(self.get_root_key().await?, salt, ROOT_KEY_CONTEXT),
				master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		// Encrypt the actual key (e.g. user-added/autogenerated, text-encodable)
		let encrypted_key = StreamEncryption::encrypt_bytes(
			master_key,
			key_nonce,
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

	/// This function is for accessing the internal keymount.
	///
	/// We could add a log to this, so that the user can view accesses
	pub async fn access_keymount(&self, uuid: Uuid) -> Result<MountedKey> {
		self.ensure_unlocked().await?;

		self.keymount
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.clone()))
	}

	/// This function is for accessing a `StoredKey`.
	pub async fn access_keystore(&self, uuid: Uuid) -> Result<StoredKey> {
		self.ensure_unlocked().await?;

		self.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.clone()))
	}

	/// This allows you to set the default key
	pub async fn set_default(&self, uuid: Uuid) -> Result<()> {
		self.ensure_unlocked().await?;

		if self.keystore.contains_key(&uuid) {
			*self.default.lock().await = Some(uuid);
			Ok(())
		} else {
			Err(Error::KeyNotFound)
		}
	}

	/// This allows you to get the default key's ID
	pub async fn get_default(&self) -> Result<Uuid> {
		self.ensure_unlocked().await?;

		self.default.lock().await.ok_or(Error::NoDefaultKeySet)
	}

	/// This should ONLY be used internally.
	async fn get_root_key(&self) -> Result<Key> {
		self.root_key.lock().await.clone().ok_or(Error::NotUnlocked)
	}

	pub async fn get_verification_key(&self) -> Result<StoredKey> {
		self.verification_key
			.lock()
			.await
			.clone()
			.ok_or(Error::NoVerificationKey)
	}

	pub async fn is_memory_only(&self, uuid: Uuid) -> Result<bool> {
		self.ensure_unlocked().await?;

		self.keystore
			.get(&uuid)
			.map_or(Err(Error::KeyNotFound), |v| Ok(v.memory_only))
	}

	pub async fn change_automount_status(&self, uuid: Uuid, status: bool) -> Result<()> {
		self.ensure_unlocked().await?;

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
	pub fn enumerate_hashed_keys(&self) -> Vec<Key> {
		self.keymount
			.iter()
			.map(|mounted_key| mounted_key.hashed_key.clone())
			.collect::<Vec<Key>>()
	}

	/// This function is for converting a memory-only key to a saved key which syncs to the library.
	///
	/// The returned value needs to be written to the database.
	pub async fn sync_to_database(&self, uuid: Uuid) -> Result<StoredKey> {
		if !self.is_memory_only(uuid).await? {
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
	pub async fn is_unlocked(&self) -> bool {
		self.root_key.lock().await.is_some()
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
	pub fn dump_keystore(&self) -> Vec<StoredKey> {
		self.keystore.iter().map(|key| key.clone()).collect()
	}

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
