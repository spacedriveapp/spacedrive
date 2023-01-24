//! This module contains the keyslot header item.
//!
//! At least one keyslot needs to be attached to a main header.
//!
//! Headers have limitations on the maximum amount of keyslots, and you should double check before usage.
//!
//! The `Keyslot::new()` function should always be used to create a keyslot, as it handles encrypting the master key.
//!
//! # Examples
//!
//! ```rust,ignore
//! use sd_crypto::header::keyslot::{Keyslot, KeyslotVersion};
//! use sd_crypto::Protected;
//! use sd_crypto::keys::hashing::{HashingAlgorithm, Params};
//! use sd_crypto::crypto::stream::Algorithm;
//! use sd_crypto::primitives::generate_master_key;
//!
//!
//! let user_password = Protected::new(b"password".to_vec());
//! let master_key = generate_master_key();
//!
//! let keyslot = Keyslot::new(KeyslotVersion::V1, Algorithm::XChaCha20Poly1305, HashingAlgorithm::Argon2id(Params::Standard), user_password, &master_key).unwrap();
//! ```
use std::io::Read;

use crate::{
	crypto::stream::{Algorithm, StreamDecryption, StreamEncryption},
	keys::hashing::HashingAlgorithm,
	primitives::{
		derive_key, generate_nonce, generate_salt, to_array, ENCRYPTED_KEY_LEN, FILE_KEY_CONTEXT,
		KEY_LEN, SALT_LEN,
	},
	Error, Protected, Result,
};

/// A keyslot - 96 bytes (as of V1), and contains all the information for future-proofing while keeping the size reasonable
///
/// The algorithm (should) be inherited from the parent (the header, in this case), but that's not a guarantee so we include it here too
#[derive(Clone)]
pub struct Keyslot {
	pub version: KeyslotVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub salt: [u8; SALT_LEN], // the salt used for deriving a KEK from a (key/content salt) hash
	pub content_salt: [u8; SALT_LEN],
	pub master_key: [u8; ENCRYPTED_KEY_LEN], // this is encrypted so we can store it
	pub nonce: Vec<u8>,
}

pub const KEYSLOT_SIZE: usize = 112;

/// This defines the keyslot version
///
/// The goal is to not increment this much, but it's here in case we need to make breaking changes
#[derive(Clone, Copy)]
pub enum KeyslotVersion {
	V1,
}

impl Keyslot {
	/// This should be used for creating a keyslot.
	///
	/// This handles generating the nonce and encrypting the master key.
	///
	/// You will need to provide the password, and a generated master key (this can't generate it, otherwise it can't be used elsewhere)
	#[allow(clippy::needless_pass_by_value)]
	pub async fn new(
		version: KeyslotVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		content_salt: [u8; SALT_LEN],
		hashed_key: Protected<[u8; KEY_LEN]>,
		master_key: Protected<[u8; KEY_LEN]>,
	) -> Result<Self> {
		let nonce = generate_nonce(algorithm);

		let salt = generate_salt();

		let encrypted_master_key = to_array::<ENCRYPTED_KEY_LEN>(
			StreamEncryption::encrypt_bytes(
				derive_key(hashed_key, salt, FILE_KEY_CONTEXT),
				&nonce,
				algorithm,
				master_key.expose(),
				&[],
			)
			.await?,
		)?;

		Ok(Self {
			version,
			algorithm,
			hashing_algorithm,
			salt,
			content_salt,
			master_key: encrypted_master_key,
			nonce,
		})
	}

	/// This function should not be used directly, use `header.decrypt_master_key()` instead
	///
	/// This attempts to decrypt the master key for a single keyslot
	///
	/// An error will be returned on failure.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn decrypt_master_key(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<Vec<u8>>> {
		let key = self
			.hashing_algorithm
			.hash(password, self.content_salt, None)
			.map_err(|_| Error::PasswordHash)?;

		StreamDecryption::decrypt_bytes(
			derive_key(key, self.salt, FILE_KEY_CONTEXT),
			&self.nonce,
			self.algorithm,
			&self.master_key,
			&[],
		)
		.await
	}

	/// This function should not be used directly, use `header.decrypt_master_key()` instead
	///
	/// This attempts to decrypt the master key for a single keyslot, using a pre-hashed key
	///
	/// No hashing is done internally.
	///
	/// An error will be returned on failure.
	pub async fn decrypt_master_key_from_prehashed(
		&self,
		key: Protected<[u8; KEY_LEN]>,
	) -> Result<Protected<Vec<u8>>> {
		StreamDecryption::decrypt_bytes(
			derive_key(key, self.salt, FILE_KEY_CONTEXT),
			&self.nonce,
			self.algorithm,
			&self.master_key,
			&[],
		)
		.await
	}

	/// This function is used to serialize a keyslot into bytes
	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		match self.version {
			KeyslotVersion::V1 => [
				self.version.to_bytes().as_ref(),
				self.algorithm.to_bytes().as_ref(),
				self.hashing_algorithm.to_bytes().as_ref(),
				&self.salt,
				&self.content_salt,
				&self.master_key,
				&self.nonce,
				&vec![0u8; 26 - self.nonce.len()],
			]
			.into_iter()
			.flatten()
			.copied()
			.collect(),
		}
	}

	/// This function reads a keyslot from a reader
	///
	/// It will leave the cursor at the end of the keyslot on success
	///
	/// The cursor will not be rewound on error.
	pub fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: Read,
	{
		let mut version = [0u8; 2];
		reader.read_exact(&mut version)?;
		let version = KeyslotVersion::from_bytes(version)?;

		match version {
			KeyslotVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read_exact(&mut algorithm)?;
				let algorithm = Algorithm::from_bytes(algorithm)?;

				let mut hashing_algorithm = [0u8; 2];
				reader.read_exact(&mut hashing_algorithm)?;
				let hashing_algorithm = HashingAlgorithm::from_bytes(hashing_algorithm)?;

				let mut salt = [0u8; SALT_LEN];
				reader.read_exact(&mut salt)?;

				let mut content_salt = [0u8; SALT_LEN];
				reader.read_exact(&mut content_salt)?;

				let mut master_key = [0u8; ENCRYPTED_KEY_LEN];
				reader.read_exact(&mut master_key)?;

				let mut nonce = vec![0u8; algorithm.nonce_len()];
				reader.read_exact(&mut nonce)?;

				reader.read_exact(&mut vec![0u8; 26 - nonce.len()])?;

				let keyslot = Self {
					version,
					algorithm,
					hashing_algorithm,
					salt,
					content_salt,
					master_key,
					nonce,
				};

				Ok(keyslot)
			}
		}
	}
}
