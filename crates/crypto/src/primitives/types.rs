//! This module defines all of the possible types used throughout this crate,
//! in an effort to add additional type safety.
use rand::{RngCore, SeedableRng};
use std::ops::Deref;
use zeroize::Zeroize;

use crate::{crypto::stream::Algorithm, keys::hashing::HashingAlgorithm, Error, Protected};

use super::{to_array, ENCRYPTED_KEY_LEN, KEY_LEN, SALT_LEN, SECRET_KEY_LEN};

#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// This should be used for providing a nonce to encrypt/decrypt functions.
///
/// You may also generate a nonce for a given algorithm with `Nonce::generate()`
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum Nonce {
	XChaCha20Poly1305([u8; 20]),
	Aes256Gcm([u8; 8]),
}

impl Nonce {
	pub fn generate(algorithm: Algorithm) -> crate::Result<Self> {
		let mut nonce = vec![0u8; algorithm.nonce_len()];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut nonce);
		Self::try_from(nonce)
	}

	#[must_use]
	pub const fn len(&self) -> usize {
		match self {
			Self::Aes256Gcm(_) => 8,
			Self::XChaCha20Poly1305(_) => 20,
		}
	}

	#[must_use]
	pub const fn is_empty(&self) -> bool {
		match self {
			Self::Aes256Gcm(x) => x.is_empty(),
			Self::XChaCha20Poly1305(x) => x.is_empty(),
		}
	}
}

impl TryFrom<Vec<u8>> for Nonce {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		match value.len() {
			8 => Ok(Self::Aes256Gcm(to_array(&value)?)),
			20 => Ok(Self::XChaCha20Poly1305(to_array(&value)?)),
			_ => Err(Error::VecArrSizeMismatch),
		}
	}
}

impl AsRef<[u8]> for Nonce {
	fn as_ref(&self) -> &[u8] {
		match self {
			Self::Aes256Gcm(x) => x,
			Self::XChaCha20Poly1305(x) => x,
		}
	}
}

impl Deref for Nonce {
	type Target = [u8];
	fn deref(&self) -> &Self::Target {
		match self {
			Self::Aes256Gcm(x) => x,
			Self::XChaCha20Poly1305(x) => x,
		}
	}
}

/// This should be used for providing a key to functions.
///
/// It can either be a random key, or a hashed key.
///
/// You may also generate a secure random key with `Key::generate()`
#[derive(Clone)]
pub struct Key(pub Protected<[u8; KEY_LEN]>);

impl Key {
	#[must_use]
	pub const fn new(v: [u8; KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	#[must_use]
	#[allow(clippy::needless_pass_by_value)]
	pub fn derive(key: Self, salt: Salt, context: &str) -> Self {
		let mut input = key.expose().to_vec();
		input.extend_from_slice(&salt);
		let key = blake3::derive_key(context, &input);

		input.zeroize();

		Self::new(key)
	}

	#[must_use]
	pub const fn expose(&self) -> &[u8; KEY_LEN] {
		self.0.expose()
	}

	#[must_use]
	pub fn generate() -> Self {
		let mut key = [0u8; KEY_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut key);
		Self::new(key)
	}
}

impl TryFrom<Protected<Vec<u8>>> for Key {
	type Error = Error;

	fn try_from(value: Protected<Vec<u8>>) -> Result<Self, Self::Error> {
		Ok(Self::new(to_array(value.expose())?))
	}
}

impl Deref for Key {
	type Target = Protected<[u8; KEY_LEN]>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// This should be used for providing a secret key to functions.
///
/// You may also generate a secret key with `SecretKey::generate()`
#[derive(Clone)]
pub struct SecretKey(pub Protected<[u8; SECRET_KEY_LEN]>);

impl SecretKey {
	#[must_use]
	pub const fn new(v: [u8; SECRET_KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	#[must_use]
	pub const fn expose(&self) -> &[u8; SECRET_KEY_LEN] {
		self.0.expose()
	}

	#[must_use]
	pub fn generate() -> Self {
		let mut secret_key = [0u8; SECRET_KEY_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut secret_key);
		Self::new(secret_key)
	}
}

impl Deref for SecretKey {
	type Target = Protected<[u8; SECRET_KEY_LEN]>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// This should be used for passing a secret key string around.
///
/// It is `SECRET_KEY_LEN` bytes, encoded in hex and delimited with `-` every 6 characters.
#[derive(Clone)]
pub struct SecretKeyString(pub Protected<String>);

impl SecretKeyString {
	#[must_use]
	pub const fn new(v: String) -> Self {
		Self(Protected::new(v))
	}

	#[must_use]
	pub const fn expose(&self) -> &String {
		self.0.expose()
	}
}

impl From<SecretKey> for SecretKeyString {
	fn from(v: SecretKey) -> Self {
		let hex_string: String = hex::encode_upper(v.0.expose())
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

		Self::new(hex_string)
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

		to_array(&secret_key)
			.ok()
			.map_or_else(Self::generate, Self::new)
	}
}

/// This should be used for passing a password around.
///
/// It can be a string of any length.
#[derive(Clone)]
pub struct Password(pub Protected<String>);

impl Password {
	#[must_use]
	pub const fn new(v: String) -> Self {
		Self(Protected::new(v))
	}

	#[must_use]
	pub const fn expose(&self) -> &String {
		self.0.expose()
	}
}

/// This should be used for passing an encrypted key around.
///
/// This is always `ENCRYPTED_KEY_LEN` (which is `KEY_LEM` + `AEAD_TAG_LEN`)
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct EncryptedKey(
	#[cfg_attr(feature = "serde", serde(with = "BigArray"))] // salt used for file data
	pub  [u8; ENCRYPTED_KEY_LEN],
);

impl Deref for EncryptedKey {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl TryFrom<Vec<u8>> for EncryptedKey {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		Ok(Self(to_array(&value)?))
	}
}

/// This should be used for passing a salt around.
///
/// You may also generate a salt with `Salt::generate()`
#[derive(Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct Salt(pub [u8; SALT_LEN]);

impl Salt {
	#[must_use]
	pub fn generate() -> Self {
		let mut salt = [0u8; SALT_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
		Self(salt)
	}
}

impl Deref for Salt {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl TryFrom<Vec<u8>> for Salt {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		Ok(Self(to_array(&value)?))
	}
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct OnboardingConfig {
	pub password: Protected<String>,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
}
