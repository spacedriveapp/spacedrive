//! This module defines all of the possible types used throughout this crate,
//! in an effort to add additional type safety.
use aead::generic_array::{ArrayLength, GenericArray};
use std::fmt::Display;
use subtle::ConstantTimeEq;

use crate::util::{generate_fixed, ToArray};
use crate::{Error, Protected};

use crate::primitives::{
	AAD_LEN, AES_256_GCM_NONCE_LEN, ARGON2ID_HARDENED, ARGON2ID_PARANOID, ARGON2ID_STANDARD,
	B3BALLOON_HARDENED, B3BALLOON_PARANOID, B3BALLOON_STANDARD, ENCRYPTED_KEY_LEN, KEY_LEN,
	SALT_LEN, SECRET_KEY_LEN, XCHACHA20_POLY1305_NONCE_LEN,
};

pub struct MagicBytes<const I: usize>([u8; I]);

impl<const I: usize> MagicBytes<I> {
	#[must_use]
	pub const fn new(bytes: [u8; I]) -> Self {
		Self(bytes)
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8; I] {
		&self.0
	}
}

#[derive(Clone, Copy)]
pub struct DerivationContext(&'static str);

impl DerivationContext {
	#[must_use]
	pub const fn new(context: &'static str) -> Self {
		Self(context)
	}

	#[must_use]
	pub const fn inner(&self) -> &'static str {
		self.0
	}
}

/// These parameters define the password-hashing level.
///
/// The greater the parameter, the longer the password will take to hash.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize,))]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum Params {
	Standard,
	Hardened,
	Paranoid,
}

impl Params {
	#[must_use]
	pub const fn default() -> Self {
		Self::Standard
	}
}

/// This defines all available password hashing algorithms.
#[derive(Clone, Copy)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize, serde::Deserialize),
	serde(tag = "name", content = "params")
)]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	BalloonBlake3(Params),
}

impl HashingAlgorithm {
	#[must_use]
	pub const fn default() -> Self {
		Self::Argon2id(Params::default())
	}

	#[must_use]
	pub const fn get_parameters(&self) -> (u32, u32, u32) {
		match self {
			Self::Argon2id(p) => match p {
				Params::Standard => ARGON2ID_STANDARD,
				Params::Hardened => ARGON2ID_HARDENED,
				Params::Paranoid => ARGON2ID_PARANOID,
			},
			Self::BalloonBlake3(p) => match p {
				Params::Standard => B3BALLOON_STANDARD,
				Params::Hardened => B3BALLOON_HARDENED,
				Params::Paranoid => B3BALLOON_PARANOID,
			},
		}
	}
}

/// This should be used for providing a nonce to encrypt/decrypt functions.
///
/// You may also generate a nonce for a given algorithm with `Nonce::generate()`
// TODO(brxken128): evaluate this `Copy` - can be expensive
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum Nonce {
	XChaCha20Poly1305([u8; XCHACHA20_POLY1305_NONCE_LEN]),
	Aes256Gcm([u8; AES_256_GCM_NONCE_LEN]),
}

impl Nonce {
	#[must_use]
	pub fn generate(algorithm: Algorithm) -> Self {
		match algorithm {
			Algorithm::Aes256Gcm => Self::Aes256Gcm(generate_fixed()),
			Algorithm::XChaCha20Poly1305 => Self::XChaCha20Poly1305(generate_fixed()),
		}
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		match self {
			Self::Aes256Gcm(x) => x,
			Self::XChaCha20Poly1305(x) => x,
		}
	}

	#[must_use]
	pub const fn len(&self) -> usize {
		match self {
			Self::Aes256Gcm(x) => x.len(),
			Self::XChaCha20Poly1305(x) => x.len(),
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

impl<I> From<Nonce> for GenericArray<u8, I>
where
	I: ArrayLength<u8>,
{
	fn from(value: Nonce) -> Self {
		match value {
			Nonce::Aes256Gcm(x) => Self::clone_from_slice(&x),
			Nonce::XChaCha20Poly1305(x) => Self::clone_from_slice(&x),
		}
	}
}

impl TryFrom<Vec<u8>> for Nonce {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		match value.len() {
			8 => Ok(Self::Aes256Gcm(value.to_array()?)),
			20 => Ok(Self::XChaCha20Poly1305(value.to_array()?)),
			_ => Err(Error::LengthMismatch),
		}
	}
}

/// These are all possible algorithms that can be used for encryption and decryption
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

impl Algorithm {
	#[must_use]
	pub const fn default() -> Self {
		Self::XChaCha20Poly1305
	}

	/// This function allows us to get the nonce length for a given encryption algorithm
	#[must_use]
	pub const fn nonce_len(&self) -> usize {
		match self {
			Self::Aes256Gcm => AES_256_GCM_NONCE_LEN,
			Self::XChaCha20Poly1305 => XCHACHA20_POLY1305_NONCE_LEN,
		}
	}
}

/// This should be used for providing a key to functions.
///
/// It can either be a random key, or a hashed key.
///
/// You may also generate a secure random key with `Key::generate()`
#[derive(Clone)]
pub struct Key(Protected<[u8; KEY_LEN]>);

impl Key {
	#[must_use]
	pub const fn new(v: [u8; KEY_LEN]) -> Self {
		Self(Protected::new(v))
	}

	#[must_use]
	pub const fn expose(&self) -> &[u8; KEY_LEN] {
		self.0.expose()
	}

	#[must_use]
	pub fn generate() -> Self {
		Self::new(generate_fixed())
	}
}

impl ConstantTimeEq for Key {
	fn ct_eq(&self, other: &Self) -> subtle::Choice {
		self.expose().ct_eq(other.expose())
	}
}

impl PartialEq for Key {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

impl<I> From<Key> for GenericArray<u8, I>
where
	I: ArrayLength<u8>,
{
	fn from(value: Key) -> Self {
		Self::clone_from_slice(value.expose())
	}
}

impl From<blake3::Hash> for Key {
	fn from(value: blake3::Hash) -> Self {
		Self::new(value.into())
	}
}

impl From<Protected<[u8; KEY_LEN]>> for Key {
	fn from(value: Protected<[u8; KEY_LEN]>) -> Self {
		Self(value)
	}
}

impl TryFrom<Protected<Vec<u8>>> for Key {
	type Error = Error;

	fn try_from(value: Protected<Vec<u8>>) -> Result<Self, Self::Error> {
		Ok(Self::new(value.into_inner().to_array()?))
	}
}

/// This should be used for providing a secret key to functions.
///
/// You may also generate a secret key with `SecretKey::generate()`
#[derive(Clone)]
pub struct SecretKey(Protected<[u8; SECRET_KEY_LEN]>);

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
		Self::new(generate_fixed())
	}

	#[must_use]
	pub fn to_vec(self) -> Vec<u8> {
		self.0.to_vec()
	}
}

/// This should be used for passing a secret key string around.
///
/// It is `SECRET_KEY_LEN` bytes, encoded in hex and delimited with `-` every 6 characters.
#[derive(Clone)]
pub struct SecretKeyString(Protected<String>);

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

		secret_key
			.to_array()
			.ok()
			.map_or_else(Self::generate, Self::new)
	}
}

impl TryFrom<Protected<Vec<u8>>> for SecretKey {
	type Error = crate::Error;

	fn try_from(v: Protected<Vec<u8>>) -> Result<Self, Self::Error> {
		Ok(Self::new(v.into_inner().to_array()?))
	}
}

/// This should be used for passing an encrypted key around.
///
/// The length of the encrypted key is `ENCRYPTED_KEY_LEN` (which is `KEY_LEM` + `AEAD_TAG_LEN`).
///
/// This also stores the associated `Nonce`, in order to make the API a lot cleaner.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct EncryptedKey(
	#[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))]
	[u8; ENCRYPTED_KEY_LEN],
	Nonce,
);

impl EncryptedKey {
	#[must_use]
	pub const fn new(v: [u8; ENCRYPTED_KEY_LEN], nonce: Nonce) -> Self {
		Self(v, nonce)
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8; ENCRYPTED_KEY_LEN] {
		&self.0
	}

	#[must_use]
	pub const fn nonce(&self) -> &Nonce {
		&self.1
	}
}

impl ConstantTimeEq for EncryptedKey {
	fn ct_eq(&self, other: &Self) -> subtle::Choice {
		self.inner().ct_eq(other.inner())
	}
}

impl PartialEq for EncryptedKey {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
pub enum Aad {
	Standard([u8; AAD_LEN]),
	Null,
}

impl Aad {
	#[must_use]
	pub fn generate() -> Self {
		Self::Standard(generate_fixed())
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		match self {
			Self::Standard(b) => b,
			Self::Null => &[],
		}
	}
}

/// This should be used for passing a salt around.
///
/// You may also generate a salt with `Salt::generate()`
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "encoding", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "rspc", derive(rspc::Type))]
pub struct Salt([u8; SALT_LEN]);

impl Salt {
	#[must_use]
	pub fn generate() -> Self {
		Self(generate_fixed())
	}

	#[must_use]
	pub const fn new(v: [u8; SALT_LEN]) -> Self {
		Self(v)
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8; SALT_LEN] {
		&self.0
	}
}

impl TryFrom<Vec<u8>> for Salt {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		Ok(Self(value.to_array()?))
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

impl Display for HashingAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Argon2id(p) => write!(f, "Argon2id ({p})"),
			Self::BalloonBlake3(p) => write!(f, "BLAKE3-Balloon ({p})"),
		}
	}
}

impl Display for Params {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Standard => write!(f, "Standard"),
			Self::Hardened => write!(f, "Hardened"),
			Self::Paranoid => write!(f, "Paranoid"),
		}
	}
}

impl Display for Algorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::XChaCha20Poly1305 => write!(f, "XChaCha20-Poly1305"),
			Self::Aes256Gcm => write!(f, "AES-256-GCM"),
		}
	}
}
