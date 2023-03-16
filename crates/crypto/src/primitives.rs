//! This module contains constant values, functions and types that are used around the crate.
//!
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the STREAM block size.
use rand::{RngCore, SeedableRng};
use zeroize::Zeroize;

use crate::{Error, Result};

#[cfg(feature = "keymanager")]
use crate::keys::keymanager::StoredKeyVersion;

/// This is the salt size.
pub const SALT_LEN: usize = 16;

pub const XCHACHA20_POLY1305_NONCE_LEN: usize = 20;
pub const AES_256_GCM_NONCE_LEN: usize = 8;

/// The length of the secret key, in bytes.
pub const SECRET_KEY_LEN: usize = 18;

/// The block size used for STREAM encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag), plus the size of the header.
pub const BLOCK_LEN: usize = 1_048_576;

/// This is the default AEAD tag size for all encryption algorithms used within the crate.
pub const AEAD_TAG_LEN: usize = 16;

pub const AAD_LEN: usize = 32;

/// The length of encrypted master keys (`KEY_LEN` + `AEAD_TAG_LEN`)
pub const ENCRYPTED_KEY_LEN: usize = 48;

/// The length of plain master/hashed keys
pub const KEY_LEN: usize = 32;

/// Used for OS keyrings to identify our items.
pub const APP_IDENTIFIER: &str = "Spacedrive";

/// Used for OS keyrings to identify our items.
pub const SECRET_KEY_IDENTIFIER: &str = "Secret key";

#[cfg(feature = "headers")]
pub use crate::header::file::LATEST_FILE_HEADER;

/// Defines the latest `StoredKeyVersion`
#[cfg(feature = "keymanager")]
pub const LATEST_STORED_KEY: StoredKeyVersion = StoredKeyVersion::V1;

/// Defines the context string for BLAKE3-KDF in regards to root key derivation
pub const ROOT_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:53:54 root key derivation";

/// Defines the context string for BLAKE3-KDF in regards to master password hash derivation
pub const MASTER_PASSWORD_CONTEXT: &str =
	"spacedrive 2022-12-14 15:35:41 master password hash derivation";

/// Defines the context string for BLAKE3-KDF in regards to file key derivation (for file encryption)
pub const FILE_KEY_CONTEXT: &str = "spacedrive 2022-12-14 12:54:12 file key derivation";

/// This is used for converting a `&[u8]` to an array of bytes.
///
/// It calls `Clone`, via `to_vec()`.
///
/// This function calls `zeroize` on any data it can
pub fn to_array<const I: usize>(bytes: &[u8]) -> Result<[u8; I]> {
	bytes.to_vec().try_into().map_err(|mut b: Vec<u8>| {
		b.zeroize();
		Error::LengthMismatch
	})
}

#[must_use]
pub fn generate_bytes(size: usize) -> Vec<u8> {
	let mut bytes = vec![0u8; size];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);
	dbg!(bytes.len());
	bytes
}

#[must_use]
pub fn generate_byte_array<const I: usize>() -> [u8; I] {
	let mut bytes = [0u8; I];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);

	bytes
}

pub fn ensure_not_null(b: &[u8]) -> Result<()> {
	(!b.iter().all(|x| x == &0u8))
		.then_some(())
		.ok_or(Error::NullType)
}

pub const fn ensure_length(expected: usize, b: &[u8]) -> Result<()> {
	if b.len() != expected {
		return Err(Error::LengthMismatch);
	}
	Ok(())
}
