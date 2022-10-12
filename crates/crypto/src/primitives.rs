//! This module contains constant values and functions that are used around the crate.
//! 
//! This includes things such as cryptographically-secure random salt/master key/nonce generation,
//! lengths for master keys and even the streaming block size.

use rand::{RngCore, SeedableRng};
use zeroize::Zeroize;

use crate::{
	error::Error,
	Protected, crypto::stream::Algorithm,
};

/// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

/// The size used for streaming encryption/decryption. This size seems to offer the best performance compared to alternatives.
///
/// The file size gain is 16 bytes per 1048576 bytes (due to the AEAD tag)
pub const BLOCK_SIZE: usize = 1_048_576;

/// The length of the encrypted master key
pub const ENCRYPTED_MASTER_KEY_LEN: usize = 48;

/// The length of the (unencrypted) master key
pub const MASTER_KEY_LEN: usize = 32;

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
pub fn generate_salt() -> [u8; SALT_LEN] {
	let mut salt = [0u8; SALT_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
	salt
}

/// This generates a master key, which should be used for encrypting the data
///
/// This is then stored (encrypted) within the header.
///
/// This function uses `ChaCha20Rng` for generating cryptographically-secure random data
#[must_use]
pub fn generate_master_key() -> Protected<[u8; MASTER_KEY_LEN]> {
	let mut master_key = [0u8; MASTER_KEY_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut master_key);
	Protected::new(master_key)
}

/// This is used for converting a `Vec<u8>` to an array of bytes
///
/// It's main usage is for converting an encrypted master key from a `Vec<u8>` to `[u8; ENCRYPTED_MASTER_KEY_LEN]`
///
/// As the master key is encrypted at this point, it does not need to be `Protected<>`
///
/// This function still `zeroize`s any data it can
pub fn to_array<const I: usize>(bytes: Vec<u8>) -> Result<[u8; I], Error> {
	bytes.try_into().map_err(|mut b: Vec<u8>| {
		b.zeroize();
		Error::VecArrSizeMismatch
	})
}
