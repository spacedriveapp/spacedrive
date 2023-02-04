use rand::{RngCore, SeedableRng};
use zeroize::Zeroize;

use crate::crypto::stream::Algorithm;

use super::{
	types::{Key, Salt, SecretKey},
	KEY_LEN, SALT_LEN, SECRET_KEY_LEN,
};

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
	input.extend_from_slice(&*salt);

	let key = blake3::derive_key(context, &input);

	input.zeroize();

	Key::new(key)
}
