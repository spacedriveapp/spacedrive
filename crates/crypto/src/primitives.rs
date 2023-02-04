//! This module contains constant values and functions that are used around the crate.
//!
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the streaming block size.
use rand::{RngCore, SeedableRng};
use zeroize::Zeroize;

use crate::{
	crypto::stream::Algorithm,
	header::{
		file::FileHeaderVersion, keyslot::KeyslotVersion, metadata::MetadataVersion,
		preview_media::PreviewMediaVersion,
	},
	keys::{hashing::HashingAlgorithm, keymanager::StoredKeyVersion},
	Error, Protected, Result,
};

/// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

pub const SECRET_KEY_LEN: usize = 18;

/// The size used for streaming encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag). Plus the size of the header.
pub const BLOCK_SIZE: usize = 1_048_576;

pub const AEAD_TAG_SIZE: usize = 16;

/// The length of the encrypted master key
pub const ENCRYPTED_KEY_LEN: usize = 48;

/// The length of the (unencrypted) master key
pub const KEY_LEN: usize = 32;

pub const PASSPHRASE_LEN: usize = 7;

pub const APP_IDENTIFIER: &str = "Spacedrive";
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

pub const LATEST_FILE_HEADER: FileHeaderVersion = FileHeaderVersion::V1;
pub const LATEST_KEYSLOT: KeyslotVersion = KeyslotVersion::V1;
pub const LATEST_METADATA: MetadataVersion = MetadataVersion::V1;
pub const LATEST_PREVIEW_MEDIA: PreviewMediaVersion = PreviewMediaVersion::V1;
pub const LATEST_STORED_KEY: StoredKeyVersion = StoredKeyVersion::V1;

pub const ROOT_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:53:54 root key derivation"; // used for deriving keys from the root key
pub const MASTER_PASSWORD_CONTEXT: &str =
	"spacedrive 2022-12-14 15:35:41 master password hash derivation"; // used for deriving keys from the master password hash
pub const FILE_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:54:12 file key derivation"; // used for deriving keys from user key/content salt hashes (for file encryption)

#[derive(Clone)]
pub struct Key(pub Protected<[u8; KEY_LEN]>);

impl Key {
	pub fn new(v: [u8; KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &[u8; KEY_LEN] {
		self.0.expose()
	}
}

#[derive(Clone)]
pub struct SecretKey(pub Protected<[u8; SECRET_KEY_LEN]>);

impl SecretKey {
	pub fn new(v: [u8; SECRET_KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &[u8; SECRET_KEY_LEN] {
		self.0.expose()
	}
}

impl Into<SecretKeyString> for SecretKey {
	fn into(self) -> SecretKeyString {
		let hex_string: String = hex::encode_upper(self.0.expose())
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

		SecretKeyString::new(hex_string)
	}
}

impl From<SecretKeyString> for SecretKey {
	fn from(v: SecretKeyString) -> Self {
		let mut secret_key_sanitized = v.expose().clone();
		secret_key_sanitized.retain(|c| c != '-' && !c.is_whitespace());

		// we shouldn't be letting on to *what* failed so we use a random secret key here if it's still invalid
		// could maybe do this better (and make use of the subtle crate)

		let secret_key = hex::decode(secret_key_sanitized)
			.ok()
			.map_or(Vec::new(), |v| v);

		to_array(secret_key)
			.ok()
			.map_or_else(generate_secret_key, SecretKey::new)
	}
}

#[derive(Clone)]
pub struct Password(pub Protected<String>);

impl Password {
	pub fn new(v: String) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &String {
		self.0.expose()
	}
}

#[derive(Clone)]
pub struct SecretKeyString(pub Protected<String>);

impl SecretKeyString {
	pub fn new(v: String) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &String {
		self.0.expose()
	}
}

#[cfg(feature = "serde")]
use serde_big_array::BigArray;
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct EncryptedKey(
	#[cfg_attr(feature = "serde", serde(with = "BigArray"))] // salt used for file data
	pub  [u8; ENCRYPTED_KEY_LEN],
);

#[derive(Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct Salt(pub [u8; SALT_LEN]);

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct OnboardingConfig {
	pub password: Protected<String>,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
}

/// This should be used for generating nonces for encryption.
///
/// An algorithm is required so this function can calculate the length of the nonce.
///
/// This function uses `ChaCha20Rng` for generating cryptographically-secure random data
#[must_use]
pub fn generate_nonce(algorithm: Algorithm) -> Vec<u8> {
	let mut nonce = vec![0u8; algorithm.nonce_len()];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut nonce);
	nonce
}

/// This should be used for generating salts for hashing.
///
/// This function uses `ChaCha20Rng` for generating cryptographically-secure random data
#[must_use]
pub fn generate_salt() -> Salt {
	let mut salt = [0u8; SALT_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
	Salt(salt)
}

/// This should be used for generating salts for hashing.
///
/// This function uses `ChaCha20Rng` for generating cryptographically-secure random data
#[must_use]
pub fn generate_secret_key() -> SecretKey {
	let mut secret_key = [0u8; SECRET_KEY_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut secret_key);
	SecretKey::new(secret_key)
}

/// This generates a master key, which should be used for encrypting the data
///
/// This is then stored (encrypted) within the header.
///
/// This function uses `ChaCha20Rng` for generating cryptographically-secure random data
#[must_use]
pub fn generate_master_key() -> Key {
	let mut master_key = [0u8; KEY_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut master_key);
	Key::new(master_key)
}

#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_key(key: Key, salt: Salt, context: &str) -> Key {
	let mut input = key.0.expose().to_vec();
	input.extend_from_slice(&salt.0);

	let key = blake3::derive_key(context, &input);

	input.zeroize();

	Key::new(key)
}

/// This is used for converting a `Vec<u8>` to an array of bytes
///
/// It's main usage is for converting an encrypted master key from a `Vec<u8>` to `EncryptedKey`
///
/// As the master key is encrypted at this point, it does not need to be `Protected<>`
///
/// This function still `zeroize`s any data it can
pub fn to_array<const I: usize>(bytes: Vec<u8>) -> Result<[u8; I]> {
	bytes.try_into().map_err(|mut b: Vec<u8>| {
		b.zeroize();
		Error::VecArrSizeMismatch
	})
}
