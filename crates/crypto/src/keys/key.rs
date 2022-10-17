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

use serde_big_array::BigArray;

use super::hashing::HashingAlgorithm;

// The terminology in this file is very confusing.
// The `master_key` is specific to the `StoredKey`, and is just used internally for encryption.
// The `key` is what the user added/generated within their Spacedrive key manager.
// The `password` in this sense is the user's "master password", similar to a password manager's main password
// The `hashed_key` refers to the value you'd pass to PVM/MD decryption functions. It has been pre-hashed with the content salt.
// The content salt refers to the semi-universal salt that's used for metadata/preview media (unique to each key in the manager)

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredKey {
	pub version: StoredKeyVersion,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
	pub salt: [u8; SALT_LEN],
	pub content_salt: [u8; SALT_LEN], // salt used for file PVM and MD
	#[serde(with = "BigArray")]
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is for encrypting the `key`
	pub master_key_nonce: Vec<u8>,
	pub key_nonce: Vec<u8>, // nonce used for encrypting the main key
	pub key: Vec<u8>,       // the key stored in spacedrive (e.g. generated 64 char key)
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum StoredKeyVersion {
	V1,
}

impl StoredKey {
	pub fn new(
		version: StoredKeyVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		password: &Protected<Vec<u8>>, // the user's provided password to unlock the keyvault
		key: &Protected<Vec<u8>>,      // the actual key stored in spacedrive's key manager
	) -> Result<Self, Error> {
		let key_nonce = generate_nonce(algorithm);
		let master_key = generate_master_key();
		let master_key_nonce = generate_nonce(algorithm);
		let salt = generate_salt();
		let content_salt = generate_salt(); // for PVM/MD

		let hashed_password = hashing_algorithm.hash(password.clone(), salt)?;

		let encrypted_master_key: [u8; 48] = to_array(StreamEncryption::encrypt_bytes(
			hashed_password,
			&master_key_nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		let encrypted_key =
			StreamEncryption::encrypt_bytes(master_key, &key_nonce, algorithm, key, &[])?;

		Ok(Self {
			version,
			algorithm,
			hashing_algorithm,
			salt,
			content_salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			key_nonce,
			key: encrypted_key,
		})
	}

	pub fn mount(&self, password: &Protected<Vec<u8>>) -> Result<MountedKey, Error> {
		let hashed_password = self.hashing_algorithm.hash(password.clone(), self.salt)?;

		let mut master_key = [0u8; MASTER_KEY_LEN];

		let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
			hashed_password,
			&self.master_key_nonce,
			self.algorithm,
			&self.master_key,
			&[],
		) {
			master_key.copy_from_slice(&decrypted_master_key);
			Ok(Protected::new(master_key))
		} else {
			Err(Error::IncorrectPassword)
		}?;

		let key = StreamDecryption::decrypt_bytes(
			master_key,
			&self.key_nonce,
			self.algorithm,
			&self.key,
			&[],
		)?;

		// this can be used for encrypting and decrypt metadata/preview media quickly
		let hashed_key = self
			.hashing_algorithm
			.hash(key.clone(), self.content_salt)?;

		let mounted_key = MountedKey {
			key,
			content_salt: self.content_salt,
			hashed_key,
			hashing_algorithm: self.hashing_algorithm,
		};

		Ok(mounted_key)
	}
}

pub struct MountedKey {
	pub key: Protected<Vec<u8>>,
	pub content_salt: [u8; SALT_LEN],
	pub hashed_key: Protected<[u8; 32]>, // this is used for encrypting/decrypting MD/PVM
	pub hashing_algorithm: HashingAlgorithm,
}
