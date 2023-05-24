//! This module defines all of the possible types used throughout this crate,
//! in an effort to add additional type safety.
use aead::generic_array::{ArrayLength, GenericArray};
use cmov::Cmov;
use std::fmt::{Debug, Display, Write};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::ct::{Choice, ConstantTimeEq, ConstantTimeEqNull};
use crate::utils::{generate_fixed, ToArray};
use crate::{Error, Protected};

use crate::primitives::{
	AAD_HEADER_LEN, AAD_LEN, AES_256_GCM_NONCE_LEN, AES_256_GCM_SIV_NONCE_LEN, ARGON2ID_HARDENED,
	ARGON2ID_PARANOID, ARGON2ID_STANDARD, BLAKE3_BALLOON_HARDENED, BLAKE3_BALLOON_PARANOID,
	BLAKE3_BALLOON_STANDARD, ENCRYPTED_KEY_LEN, KEY_LEN, SALT_LEN, SECRET_KEY_LEN,
	XCHACHA20_POLY1305_NONCE_LEN,
};

#[derive(Clone, Copy)]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	Blake3Balloon(Params),
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
			Self::Blake3Balloon(p) => match p {
				Params::Standard => BLAKE3_BALLOON_STANDARD,
				Params::Hardened => BLAKE3_BALLOON_HARDENED,
				Params::Paranoid => BLAKE3_BALLOON_PARANOID,
			},
		}
	}
}

/// This should be used for providing a nonce to encrypt/decrypt functions.
///
/// You may also generate a nonce for a given algorithm with `Nonce::generate()`
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Nonce {
	Aes256Gcm([u8; AES_256_GCM_NONCE_LEN]),
	Aes256GcmSiv([u8; AES_256_GCM_SIV_NONCE_LEN]),
	XChaCha20Poly1305([u8; XCHACHA20_POLY1305_NONCE_LEN]),
}

impl Nonce {
	#[must_use]
	pub fn generate(algorithm: Algorithm) -> Self {
		match algorithm {
			Algorithm::Aes256Gcm => Self::Aes256Gcm(generate_fixed()),
			Algorithm::Aes256GcmSiv => Self::Aes256GcmSiv(generate_fixed()),
			Algorithm::XChaCha20Poly1305 => Self::XChaCha20Poly1305(generate_fixed()),
		}
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		match self {
			Self::Aes256Gcm(x) | Self::Aes256GcmSiv(x) => x,
			Self::XChaCha20Poly1305(x) => x,
		}
	}

	#[must_use]
	pub const fn len(&self) -> usize {
		match self {
			Self::Aes256Gcm(x) | Self::Aes256GcmSiv(x) => x.len(),
			Self::XChaCha20Poly1305(x) => x.len(),
		}
	}

	#[must_use]
	pub const fn is_empty(&self) -> bool {
		match self {
			Self::Aes256Gcm(x) | Self::Aes256GcmSiv(x) => x.is_empty(),
			Self::XChaCha20Poly1305(x) => x.is_empty(),
		}
	}

	#[must_use]
	pub const fn algorithm(&self) -> Algorithm {
		match self {
			Self::Aes256Gcm(_) => Algorithm::Aes256Gcm,
			Self::Aes256GcmSiv(_) => Algorithm::Aes256GcmSiv,
			Self::XChaCha20Poly1305(_) => Algorithm::XChaCha20Poly1305,
		}
	}

	pub fn validate(&self, algorithm: Algorithm) -> crate::Result<()> {
		let mut x = 1u8;
		x.cmovz(&0, (self.algorithm().ct_eq(&algorithm)).unwrap_u8());
		x.cmovz(&0, (self.inner().ct_ne_null()).unwrap_u8());

		bool::from(Choice::from(x))
			.then_some(())
			.ok_or(Error::Validity)
	}
}

impl ConstantTimeEq for Nonce {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		self.inner().ct_eq(rhs.inner())
	}
}

impl<I> From<&Nonce> for GenericArray<u8, I>
where
	I: ArrayLength<u8>,
{
	fn from(value: &Nonce) -> Self {
		match value {
			Nonce::Aes256Gcm(x) | Nonce::Aes256GcmSiv(x) => Self::clone_from_slice(x),
			Nonce::XChaCha20Poly1305(x) => Self::clone_from_slice(x),
		}
	}
}

/// These are all possible algorithms that can be used for encryption and decryption
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Algorithm {
	Aes256Gcm,
	Aes256GcmSiv,
	XChaCha20Poly1305,
}

impl ConstantTimeEq for Algorithm {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		#[allow(clippy::as_conversions)]
		(*self as u8).ct_eq(&(*rhs as u8))
	}
}

impl PartialEq for Algorithm {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

impl Algorithm {
	#[must_use]
	pub const fn default() -> Self {
		Self::XChaCha20Poly1305
	}

	/// This function returns the nonce length for a given encryption algorithm
	#[must_use]
	pub const fn nonce_len(&self) -> usize {
		match self {
			Self::Aes256Gcm => AES_256_GCM_NONCE_LEN,
			Self::Aes256GcmSiv => AES_256_GCM_SIV_NONCE_LEN,
			Self::XChaCha20Poly1305 => XCHACHA20_POLY1305_NONCE_LEN,
		}
	}
}

/// This should be used for providing a key to functions.
///
/// It can either be a random key, or a hashed key.
///
/// You may also generate a secure random key with `Key::generate()`
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Key([u8; KEY_LEN]);

impl Key {
	#[must_use]
	pub const fn new(v: [u8; KEY_LEN]) -> Self {
		Self(v)
	}

	#[must_use]
	pub const fn expose(&self) -> &[u8] {
		&self.0
	}

	#[must_use]
	pub fn generate() -> Self {
		Self::new(generate_fixed())
	}

	pub fn validate(&self) -> crate::Result<()> {
		bool::from(self.expose().ct_ne_null())
			.then_some(())
			.ok_or(Error::Validity)
	}
}

impl ConstantTimeEq for Key {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		self.expose().ct_eq(rhs.expose())
	}
}

impl PartialEq for Key {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

impl<I> From<&Key> for GenericArray<u8, I>
where
	I: ArrayLength<u8>,
{
	fn from(value: &Key) -> Self {
		Self::clone_from_slice(value.expose())
	}
}

impl From<blake3::Hash> for Key {
	fn from(value: blake3::Hash) -> Self {
		Self::new(value.into())
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
// /// You may also generate a secret key with `SecretKey::generate()`
#[derive(Zeroize, ZeroizeOnDrop, Clone)]
pub enum SecretKey {
	Standard([u8; SECRET_KEY_LEN]),
	Variable(Vec<u8>),
	Null,
}

impl SecretKey {
	#[must_use]
	pub const fn new(v: [u8; SECRET_KEY_LEN]) -> Self {
		Self::Standard(v)
	}

	#[must_use]
	pub fn expose(&self) -> &[u8] {
		match self {
			Self::Standard(v) => v,
			Self::Variable(v) => v,
			Self::Null => &[],
		}
	}

	#[must_use]
	pub fn generate() -> Self {
		Self::new(generate_fixed())
	}
}

impl TryFrom<Protected<Vec<u8>>> for SecretKey {
	type Error = Error;

	fn try_from(value: Protected<Vec<u8>>) -> Result<Self, Self::Error> {
		let sk = match value.expose().len() {
			// this won't fail as we check the size
			SECRET_KEY_LEN => Self::Standard(value.into_inner().to_array()?),
			0 => Self::Null,
			_ => Self::Variable(value.into_inner()),
		};

		Ok(sk)
	}
}

impl TryFrom<Protected<String>> for SecretKey {
	type Error = Error;

	fn try_from(value: Protected<String>) -> Result<Self, Self::Error> {
		let mut s = value.into_inner();
		s.retain(|c| c.is_ascii_hexdigit());

		// shouldn't fail as `SecretKey::try_from` is (essentially) infallible
		hex::decode(s)
			.ok()
			.map_or(Protected::new(vec![]), Protected::new)
			.try_into()
			.map_err(|_| Error::Validity)
	}
}

impl Display for SecretKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = hex::encode(self.expose()).to_uppercase();
		let separator_distance = s.len() / 6;
		s.chars().enumerate().try_for_each(|(i, c)| {
			f.write_char(c)?;
			if (i + 1) % separator_distance == 0 && (i + 1) != s.len() {
				f.write_char('-')?;
			}

			Ok(())
		})
	}
}

/// This should be used for passing an encrypted key around.
///
/// The length of the encrypted key is `ENCRYPTED_KEY_LEN` (which is `KEY_LEM` + `AEAD_TAG_LEN`).
///
/// This also stores the associated `Nonce`, in order to make the API a lot cleaner.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
	pub const fn inner(&self) -> &[u8] {
		&self.0
	}

	#[must_use]
	pub const fn nonce(&self) -> &Nonce {
		&self.1
	}
}

impl ConstantTimeEq for EncryptedKey {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		// short circuit if algorithm (and therefore nonce lengths) don't match
		if !bool::from(self.nonce().algorithm().ct_eq(&rhs.nonce().algorithm())) {
			return Choice::from(0);
		}

		let mut x = 1u8;
		x.cmovz(&0u8, self.nonce().ct_eq(rhs.nonce()).unwrap_u8());
		x.cmovz(&0u8, self.inner().ct_eq(rhs.inner()).unwrap_u8());
		Choice::from(x)
	}
}

impl PartialEq for EncryptedKey {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Aad {
	Standard([u8; AAD_LEN]),
	Header(
		#[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))]
		[u8; AAD_HEADER_LEN],
	),
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
			Self::Header(b) => b,
			Self::Null => &[],
		}
	}
}

impl ConstantTimeEq for Aad {
	fn ct_eq(&self, other: &Self) -> Choice {
		self.inner().ct_eq(other.inner())
	}
}

impl PartialEq for Aad {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

/// This should be used for passing a salt around.
///
/// You may also generate a salt with `Salt::generate()`
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
	pub const fn inner(&self) -> &[u8] {
		&self.0
	}
}

impl TryFrom<Vec<u8>> for Salt {
	type Error = Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		Ok(Self::new(value.to_array()?))
	}
}

impl Display for HashingAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Argon2id(p) => write!(f, "Argon2id ({p})"),
			Self::Blake3Balloon(p) => write!(f, "BLAKE3-Balloon ({p})"),
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
			Self::Aes256Gcm => write!(f, "AES-256-GCM"),
			Self::Aes256GcmSiv => write!(f, "AES-256-GCM-SIV"),
			Self::XChaCha20Poly1305 => write!(f, "XChaCha20-Poly1305"),
		}
	}
}

impl Debug for Algorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self}")
	}
}

impl Debug for Key {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("[REDACTED]")
	}
}

impl Debug for EncryptedKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("[REDACTED]")
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		primitives::{
			AES_256_GCM_NONCE_LEN, ENCRYPTED_KEY_LEN, KEY_LEN, XCHACHA20_POLY1305_NONCE_LEN,
		},
		types::{EncryptedKey, Key, Nonce},
	};

	use super::Algorithm;

	const KEY: Key = Key::new([0x23; KEY_LEN]);
	const KEY2: Key = Key::new([0x24; KEY_LEN]);

	const EK: [[u8; ENCRYPTED_KEY_LEN]; 2] = [[0x20; ENCRYPTED_KEY_LEN], [0x21; ENCRYPTED_KEY_LEN]];
	const NONCES: [Nonce; 2] = [
		Nonce::XChaCha20Poly1305([5u8; XCHACHA20_POLY1305_NONCE_LEN]),
		Nonce::Aes256Gcm([1u8; AES_256_GCM_NONCE_LEN]),
	];

	#[test]
	fn encrypted_key_eq() {
		// same key and nonce
		assert_eq!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[0], NONCES[0])
		);

		// same key, different nonce
		assert_ne!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[0], NONCES[1])
		);

		// different key, same nonce
		assert_ne!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[1], NONCES[0])
		);
	}

	#[test]
	#[should_panic]
	fn encrypted_key_eq_different_key() {
		// different key, same nonce
		assert_eq!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[1], NONCES[0])
		);
	}

	#[test]
	#[should_panic]
	fn encrypted_key_eq_different_nonce() {
		// same key, different nonce
		assert_eq!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[0], NONCES[1])
		);
	}

	#[test]
	fn key_eq() {
		assert_eq!(KEY, KEY);
	}

	#[test]
	#[should_panic]
	fn key_eq_fail() {
		assert_eq!(KEY, KEY2);
	}

	#[test]
	fn algorithm_eq() {
		assert_eq!(Algorithm::XChaCha20Poly1305, Algorithm::XChaCha20Poly1305);
	}

	#[test]
	#[should_panic]
	fn algorithm_eq_fail() {
		assert_eq!(Algorithm::XChaCha20Poly1305, Algorithm::Aes256Gcm);
	}

	#[test]
	fn key_validate() {
		KEY.validate().unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn key_validate_fail() {
		Key::new([0u8; KEY_LEN]).validate().unwrap();
	}

	#[test]
	fn nonce_validate() {
		Nonce::generate(Algorithm::default())
			.validate(Algorithm::default())
			.unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn nonce_validate_different_algorithms() {
		Nonce::generate(Algorithm::XChaCha20Poly1305)
			.validate(Algorithm::Aes256Gcm)
			.unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn nonce_validate_null() {
		Nonce::XChaCha20Poly1305([0u8; XCHACHA20_POLY1305_NONCE_LEN])
			.validate(Algorithm::XChaCha20Poly1305)
			.unwrap();
	}
}
