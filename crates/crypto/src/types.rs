//! This module defines all of the possible types used throughout this crate,
//! in an effort to add additional type safety.
use crate::{
	ct::{Choice, ConstantTimeEq, ConstantTimeEqNull},
	rng::CryptoRng,
	utils::ToArray,
	Error, Protected,
};

use aead::generic_array::{ArrayLength, GenericArray};
use bincode::{Decode, Encode};
use cmov::Cmov;
use std::fmt::{Debug, Display, Write};
use zeroize::{DefaultIsZeroes, Zeroize, ZeroizeOnDrop};

use crate::primitives::{
	AAD_HEADER_LEN, AAD_LEN, AES_256_GCM_SIV_NONCE_LEN, ARGON2ID_HARDENED, ARGON2ID_PARANOID,
	ARGON2ID_STANDARD, BLAKE3_BALLOON_HARDENED, BLAKE3_BALLOON_PARANOID, BLAKE3_BALLOON_STANDARD,
	ENCRYPTED_KEY_LEN, KEY_LEN, SALT_LEN, SECRET_KEY_LEN, XCHACHA20_POLY1305_NONCE_LEN,
};

#[derive(Clone, Copy)]
pub struct MagicBytes<const I: usize>([u8; I]);

impl<const I: usize> MagicBytes<I> {
	#[inline]
	#[must_use]
	pub const fn new(bytes: [u8; I]) -> Self {
		Self(bytes)
	}

	#[inline]
	#[must_use]
	pub const fn inner(&self) -> &[u8; I] {
		&self.0
	}
}

#[derive(Clone, Copy)]
pub struct DerivationContext(&'static str);

impl DerivationContext {
	#[inline]
	#[must_use]
	pub const fn new(context: &'static str) -> Self {
		Self(context)
	}

	#[inline]
	#[must_use]
	pub const fn inner(&self) -> &'static str {
		self.0
	}
}

/// These parameters define the password-hashing level.
///
/// The greater the parameter, the longer the password will take to hash.
#[derive(Clone, Copy, Default, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Params {
	#[default]
	Standard,
	Hardened,
	Paranoid,
}

/// This defines all available password hashing algorithms.
#[derive(Clone, Copy, Encode, Decode)]
#[cfg_attr(
	feature = "serde",
	derive(serde::Serialize, serde::Deserialize),
	serde(tag = "name", content = "params")
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum HashingAlgorithm {
	Argon2id(Params),
	Blake3Balloon(Params),
}

impl Default for HashingAlgorithm {
	fn default() -> Self {
		Self::Argon2id(Params::default())
	}
}

impl HashingAlgorithm {
	#[inline]
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
#[derive(Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Nonce {
	Aes256GcmSiv([u8; AES_256_GCM_SIV_NONCE_LEN]),
	XChaCha20Poly1305([u8; XCHACHA20_POLY1305_NONCE_LEN]),
}

impl Nonce {
	#[inline]
	#[must_use]
	pub fn generate(algorithm: Algorithm) -> Self {
		match algorithm {
			Algorithm::Aes256GcmSiv => Self::Aes256GcmSiv(CryptoRng::generate_fixed()),
			Algorithm::XChaCha20Poly1305 => Self::XChaCha20Poly1305(CryptoRng::generate_fixed()),
		}
	}

	#[inline]
	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		match self {
			Self::Aes256GcmSiv(x) => x,
			Self::XChaCha20Poly1305(x) => x,
		}
	}

	#[inline]
	#[must_use]
	pub const fn len(&self) -> usize {
		match self {
			Self::Aes256GcmSiv(x) => x.len(),
			Self::XChaCha20Poly1305(x) => x.len(),
		}
	}

	#[inline]
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		match self {
			Self::Aes256GcmSiv(x) => x.is_empty(),
			Self::XChaCha20Poly1305(x) => x.is_empty(),
		}
	}

	#[inline]
	#[must_use]
	pub const fn algorithm(&self) -> Algorithm {
		match self {
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
			Nonce::Aes256GcmSiv(x) => Self::clone_from_slice(x),
			Nonce::XChaCha20Poly1305(x) => Self::clone_from_slice(x),
		}
	}
}

/// These are all possible algorithms that can be used for encryption and decryption
#[derive(Clone, Copy, Default, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Algorithm {
	Aes256GcmSiv,
	#[default]
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
	/// This function returns the nonce length for a given encryption algorithm
	#[inline]
	#[must_use]
	pub const fn nonce_len(&self) -> usize {
		match self {
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
#[repr(transparent)]
pub struct Key(Box<[u8; KEY_LEN]>);

impl Key {
	#[inline]
	#[must_use]
	pub fn new(v: [u8; KEY_LEN]) -> Self {
		Self(Box::new(v))
	}

	#[inline]
	#[must_use]
	pub const fn expose(&self) -> &[u8] {
		self.0.as_slice()
	}

	#[inline]
	#[must_use]
	pub fn generate() -> Self {
		Self::new(CryptoRng::generate_fixed())
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

#[cfg(feature = "serde")]
impl serde::Serialize for Key {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serdect::array::serialize_hex_lower_or_bin(&self.expose(), serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Key {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let mut buf = [0u8; 32];
		serdect::array::deserialize_hex_or_bin(&mut buf, deserializer)?;
		Ok(Self::new(buf))
	}
}

// The `serde` feature is needed as this makes use of a crate called `serdect` which
// allows for constant-time serialization and deserialization. We then use `bincode`s
// compatability layer to serialize through that, so in theory it should remain constant-time
#[cfg(feature = "serde")]
impl Encode for Key {
	fn encode<E: bincode::enc::Encoder>(
		&self,
		encoder: &mut E,
	) -> Result<(), bincode::error::EncodeError> {
		bincode::serde::Compat(self).encode(encoder)?;

		Ok(())
	}
}

// The `serde` feature is needed as this makes use of a crate called `serdect` which
// allows for constant-time serialization and deserialization. We then use `bincode`s
// compatability layer to serialize through that, so in theory it should remain constant-time
#[cfg(feature = "serde")]
impl Decode for Key {
	fn decode<D: bincode::de::Decoder>(
		decoder: &mut D,
	) -> Result<Self, bincode::error::DecodeError> {
		Ok(bincode::serde::Compat::decode(decoder)?.0)
	}
}

impl<I> From<&Key> for GenericArray<u8, I>
where
	I: ArrayLength<u8>,
{
	fn from(value: &Key) -> Self {
		GenericArray::clone_from_slice(value.expose())
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

impl TryFrom<Protected<Box<[u8]>>> for Key {
	type Error = Error;

	fn try_from(value: Protected<Box<[u8]>>) -> Result<Self, Self::Error> {
		Ok(Self::new(value.expose().to_array()?))
	}
}

//
// impl bincode::Encode for Key {
// 	fn encode<E: bincode::enc::Encoder>(
// 		&self,
// 		encoder: &mut E,
// 	) -> Result<(), bincode::error::EncodeError> {
// 		serdect::array::serialize_hex_lower_or_bin(self.expose(), bincode::serde::)
// 	}
// }

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
	#[inline]
	#[must_use]
	pub const fn new(v: [u8; SECRET_KEY_LEN]) -> Self {
		Self::Standard(v)
	}

	#[inline]
	#[must_use]
	pub fn expose(&self) -> &[u8] {
		match self {
			Self::Standard(v) => v,
			Self::Variable(v) => v,
			Self::Null => &[],
		}
	}

	#[inline]
	#[must_use]
	pub fn generate() -> Self {
		Self::new(CryptoRng::generate_fixed())
	}
}

impl TryFrom<Protected<Vec<u8>>> for SecretKey {
	type Error = Error;

	fn try_from(value: Protected<Vec<u8>>) -> Result<Self, Self::Error> {
		let sk = match value.expose().len() {
			// this won't fail as we check the size
			SECRET_KEY_LEN => Self::new(value.into_inner().to_array()?),
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
		hex::decode(&s)
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
#[derive(Clone, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EncryptedKey(
	#[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))]
	[u8; ENCRYPTED_KEY_LEN],
	Nonce,
);

impl EncryptedKey {
	#[inline]
	#[must_use]
	pub const fn new(v: [u8; ENCRYPTED_KEY_LEN], nonce: Nonce) -> Self {
		Self(v, nonce)
	}

	#[inline]
	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		&self.0
	}

	#[inline]
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

#[derive(Clone, Copy, Default, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Aad {
	Standard([u8; AAD_LEN]),
	Header(
		#[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))]
		[u8; AAD_HEADER_LEN],
	),
	#[default]
	Null,
}

impl Aad {
	#[inline]
	#[must_use]
	pub fn generate() -> Self {
		Self::Standard(CryptoRng::generate_fixed())
	}

	#[inline]
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
#[derive(Clone, Copy, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Salt([u8; SALT_LEN]);

impl DefaultIsZeroes for Salt {}

impl Default for Salt {
	fn default() -> Self {
		Self([0u8; SALT_LEN])
	}
}

impl Salt {
	#[inline]
	#[must_use]
	pub fn generate() -> Self {
		Self(CryptoRng::generate_fixed())
	}

	#[inline]
	#[must_use]
	pub const fn new(v: [u8; SALT_LEN]) -> Self {
		Self(v)
	}

	#[inline]
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
	use super::Algorithm;
	use crate::{
		primitives::{
			AES_256_GCM_SIV_NONCE_LEN, ENCRYPTED_KEY_LEN, KEY_LEN, XCHACHA20_POLY1305_NONCE_LEN,
		},
		types::{EncryptedKey, Key, Nonce},
	};

	const EK: [[u8; ENCRYPTED_KEY_LEN]; 2] = [[0x20; ENCRYPTED_KEY_LEN], [0x21; ENCRYPTED_KEY_LEN]];
	const NONCES: [Nonce; 2] = [
		Nonce::XChaCha20Poly1305([5u8; XCHACHA20_POLY1305_NONCE_LEN]),
		Nonce::Aes256GcmSiv([8u8; AES_256_GCM_SIV_NONCE_LEN]),
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
	#[should_panic(expected = "assertion")]
	fn encrypted_key_eq_different_key() {
		// different key, same nonce
		assert_eq!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[1], NONCES[0])
		);
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn encrypted_key_eq_different_nonce() {
		// same key, different nonce
		assert_eq!(
			EncryptedKey::new(EK[0], NONCES[0]),
			EncryptedKey::new(EK[0], NONCES[1])
		);
	}

	#[test]
	fn key_eq() {
		assert_eq!(Key::new([0x23; KEY_LEN]), Key::new([0x23; KEY_LEN]));
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn key_eq_fail() {
		assert_eq!(Key::new([0x23; KEY_LEN]), Key::new([0x24; KEY_LEN]));
	}

	#[test]
	fn algorithm_eq() {
		assert_eq!(Algorithm::XChaCha20Poly1305, Algorithm::XChaCha20Poly1305);
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn algorithm_eq_fail() {
		assert_eq!(Algorithm::XChaCha20Poly1305, Algorithm::Aes256GcmSiv);
	}

	#[test]
	fn key_validate() {
		Key::new([0x23; KEY_LEN]).validate().unwrap();
	}

	#[test]
	#[should_panic(expected = "Validity")]
	fn key_validate_null() {
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
			.validate(Algorithm::Aes256GcmSiv)
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
