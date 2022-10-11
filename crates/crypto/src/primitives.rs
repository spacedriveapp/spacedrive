use rand::{RngCore, SeedableRng};
use zeroize::Zeroize;

use crate::{
	error::Error,
	keys::hashing::{password_hash_argon2id, Params},
	protected::Protected,
};

// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

/// The size used for streaming blocks. This size seems to offer the best performance compared to alternatives.
///
/// This was changed from 1MiB due to blazingly fast speeds, and still pretty low file size gain.
///
/// The file size gain is 16 bytes per 0.0625MiB (due to the AEAD tag)
pub const BLOCK_SIZE: usize = 65_536;

/// The length of the encrypted master key
pub const ENCRYPTED_MASTER_KEY_LEN: usize = 48;

/// The length of the (unencrypted) master key
pub const MASTER_KEY_LEN: usize = 32;

/// These are all possible algorithms that can be used for encryption
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

/// A hashing algorithm with desired parameters
#[derive(Clone, Copy)]
pub enum HashingAlgorithm {
	Argon2id(Params),
}

impl HashingAlgorithm {
	/// This function should be used to hash passwords
	///
	/// It also handles all the security "levels"
	pub fn hash(
		&self,
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
	) -> Result<Protected<[u8; 32]>, Error> {
		match self {
			Self::Argon2id(params) => password_hash_argon2id(password, salt, *params),
		}
	}
}

impl Algorithm {
	// This function calculates the expected nonce length for a given algorithm
	// 4 bytes are deducted for streaming mode, due to the LE31 counter being the last 4 bytes of the nonce
	#[must_use]
	pub const fn nonce_len(&self) -> usize {
		match self {
			Self::XChaCha20Poly1305 => 20,
			Self::Aes256Gcm => 8,
		}
	}
}

/// The length can easily be obtained via `algorithm.nonce_len()`
///
/// This function uses `ChaCha20Rng` for cryptographically-securely generating random data
#[must_use]
pub fn generate_nonce(len: usize) -> Vec<u8> {
	let mut nonce = vec![0u8; len];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut nonce);
	nonce
}

/// This function uses `ChaCha20Rng` for cryptographically-securely generating random data
#[must_use]
pub fn generate_salt() -> [u8; SALT_LEN] {
	let mut salt = [0u8; SALT_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
	salt
}

/// This generates a master key, which should be used for encrypting the data
///
/// This is then stored encrypted in the header
///
/// This function uses `ChaCha20Rng` for cryptographically-securely generating random data
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
