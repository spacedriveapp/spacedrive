use rand::{RngCore, SeedableRng};
use std::ops::Deref;

use crate::{crypto::stream::Algorithm, keys::hashing::HashingAlgorithm, Error, Protected};

#[derive(Clone, Copy)]
// pub struct Nonce<const I: usize>(pub [u8; I]);
pub enum Nonce {
	XChaCha20Poly1305([u8; 20]),
	Aes256Gcm([u8; 8]),
}

impl Nonce {
	pub fn generate(algorithm: Algorithm) -> crate::Result<Self> {
		let mut nonce = vec![0u8; algorithm.nonce_len()];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut nonce);
		Ok(Nonce::try_from(nonce)?)
	}
}

impl TryFrom<Vec<u8>> for Nonce {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		match value.len() {
			8 => Ok(Nonce::Aes256Gcm(to_array(&value)?)),
			20 => Ok(Nonce::XChaCha20Poly1305(to_array(&value)?)),
			_ => Err(Error::NonceLengthMismatch),
		}
	}
}

#[derive(Clone)]
pub struct Key(pub Protected<[u8; KEY_LEN]>);

impl Key {
	pub fn new(v: [u8; KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &[u8; KEY_LEN] {
		self.0.expose()
	}

	pub fn generate() -> Self {
		let mut key = [0u8; KEY_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut key);
		Key::new(key)
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

#[derive(Clone)]
pub struct SecretKey(pub Protected<[u8; SECRET_KEY_LEN]>);

impl SecretKey {
	pub fn new(v: [u8; SECRET_KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	pub fn expose(&self) -> &[u8; SECRET_KEY_LEN] {
		self.0.expose()
	}

	pub fn generate() -> Self {
		let mut secret_key = [0u8; SECRET_KEY_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut secret_key);
		SecretKey::new(secret_key)
	}
}

impl Deref for SecretKey {
	type Target = Protected<[u8; SECRET_KEY_LEN]>;

	fn deref(&self) -> &Self::Target {
		&self.0
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

		to_array(&secret_key)
			.ok()
			.map_or_else(generate_secret_key, Self::new)
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

use super::{
	rng::generate_secret_key, to_array, ENCRYPTED_KEY_LEN, KEY_LEN, SALT_LEN, SECRET_KEY_LEN,
};
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct EncryptedKey(
	#[cfg_attr(feature = "serde", serde(with = "BigArray"))] // salt used for file data
	pub  [u8; ENCRYPTED_KEY_LEN],
);

impl Deref for EncryptedKey {
	type Target = [u8; ENCRYPTED_KEY_LEN];

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

#[derive(Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct Salt(pub [u8; SALT_LEN]);

impl Salt {
	pub fn generate() -> Self {
		let mut salt = [0u8; SALT_LEN];
		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut salt);
		Salt(salt)
	}
}

impl Deref for Salt {
	type Target = [u8; SALT_LEN];

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
#[cfg_attr(feature = "rspc", derive(specta::Type))]
pub struct OnboardingConfig {
	pub password: Protected<String>,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
}
