use rand::{RngCore, SeedableRng};
use secrecy::Secret;

use crate::{
	error::Error,
	keys::hashing::{password_hash_argon2id, Params},
};

// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

/// The size used for streaming blocks. This size seems to offer the best performance compared to alternatives.
/// The file size gain is 16 bytes per 1MiB (due to the AEAD tag)
pub const BLOCK_SIZE: usize = 1_048_576;

pub const ENCRYPTED_MASTER_KEY_LEN: usize = 48;
pub const MASTER_KEY_LEN: usize = 32;

// These are all possible algorithms that can be used for encryption
// They tie in heavily with `StreamEncryption` and `StreamDecryption`
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

// These are the different "modes" for encryption
// Stream works in "blocks", incrementing the nonce on each block (so the same nonce isn't used twice)
// Memory loads all data into memory before encryption, and encrypts it in one pass.
// Stream mode is going to be the default for files, containers, etc. as  memory usage is roughly equal to the `BLOCK_SIZE`
// Memory mode is only going to be used for small amounts of data (such as a master key) - streaming modes aren't viable here
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Mode {
	Stream,
	Memory,
}

// (Password)HashingAlgorithm
pub enum HashingAlgorithm {
	Argon2id(Params),
}

impl HashingAlgorithm {
	pub fn hash(
		&self,
		password: Secret<Vec<u8>>,
		salt: [u8; SALT_LEN],
	) -> Result<Secret<[u8; 32]>, Error> {
		match self {
			Self::Argon2id(params) => password_hash_argon2id(password, salt, *params),
		}
	}
}

impl Algorithm {
	// This function calculates the expected nonce length for a given algorithm
	// 4 bytes are deducted for streaming mode, due to the LE31 counter being the last 4 bytes of the nonce
	#[must_use]
	pub const fn nonce_len(&self, mode: Mode) -> usize {
		let base = match self {
			Self::XChaCha20Poly1305 => 24,
			Self::Aes256Gcm => 12,
		};

		match mode {
			Mode::Stream => base - 4,
			Mode::Memory => base,
		}
	}
}

// The length can easily be obtained via `algorithm.nonce_len(mode)`
#[must_use]
pub fn generate_nonce(len: usize) -> Vec<u8> {
	let mut nonce = vec![0u8; len];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut nonce);
	nonce
}

#[must_use]
pub fn generate_salt() -> [u8; SALT_LEN] {
	let mut salt = [0u8; SALT_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
	salt
}

#[must_use]
pub fn generate_master_key() -> [u8; MASTER_KEY_LEN] {
	let mut master_key = [0u8; MASTER_KEY_LEN];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut master_key);
	master_key
}
