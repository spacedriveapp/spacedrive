use crate::{
	ct::{Choice, ConstantTimeEq, ConstantTimeEqNull},
	rng::CryptoRng,
	Error,
};

use std::fmt;

use aead::array::Array;
use blake3::{Hash, Hasher};
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use typenum::{consts::U32, U64};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// This should be used for encrypting and decrypting data.
///
/// You can pass an existing key to [`SecretKey::new`] or you may also generate
/// a secure random key with [`SecretKey::generate`], passing the [`CryptoRng`] to generate
/// random bytes.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
#[repr(transparent)]
pub struct SecretKey(pub(crate) Array<u8, U32>);

impl fmt::Debug for SecretKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("SecretKey(<REDACTED>)")
	}
}

impl SecretKey {
	#[inline]
	#[must_use]
	pub const fn new(v: Array<u8, U32>) -> Self {
		Self(v)
	}

	#[inline]
	#[must_use]
	pub fn generate(rng: &mut CryptoRng) -> Self {
		let mut key_candidate = rng.generate_fixed();

		while bool::from(key_candidate.ct_eq_null()) {
			key_candidate = rng.generate_fixed();
		}

		Self(key_candidate.into())
	}

	#[must_use]
	pub fn to_hash(&self) -> Hash {
		let mut hasher = Hasher::new();
		hasher.update(&self.0);
		hasher.finalize()
	}
}

impl ConstantTimeEq for SecretKey {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		self.0.ct_eq(&rhs.0)
	}
}

impl PartialEq for SecretKey {
	fn eq(&self, other: &Self) -> bool {
		self.ct_eq(other).into()
	}
}

impl Serialize for SecretKey {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serdect::array::serialize_hex_lower_or_bin(&self.0, serializer)
	}
}

impl AsRef<[u8]> for SecretKey {
	fn as_ref(&self) -> &[u8] {
		&self.0
	}
}

impl<'de> Deserialize<'de> for SecretKey {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let mut buf = [0u8; 32];
		serdect::array::deserialize_hex_or_bin(&mut buf, deserializer)?;
		Ok(Self::new(buf.into()))
	}
}

impl From<&SecretKey> for Array<u8, U32> {
	fn from(SecretKey(key): &SecretKey) -> Self {
		*key
	}
}

impl From<&SecretKey> for Vec<u8> {
	fn from(SecretKey(key): &SecretKey) -> Self {
		key.to_vec()
	}
}

impl From<SecretKey> for Vec<u8> {
	fn from(SecretKey(key): SecretKey) -> Self {
		key.to_vec()
	}
}

impl TryFrom<&[u8]> for SecretKey {
	type Error = Error;

	fn try_from(key: &[u8]) -> Result<Self, Self::Error> {
		if key.len() != 32 {
			return Err(Error::InvalidKeySize(key.len()));
		}

		Ok(Self(Array([
			key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9],
			key[10], key[11], key[12], key[13], key[14], key[15], key[16], key[17], key[18],
			key[19], key[20], key[21], key[22], key[23], key[24], key[25], key[26], key[27],
			key[28], key[29], key[30], key[31],
		])))
	}
}

impl From<GenericArray<u8, U32>> for SecretKey {
	fn from(key: GenericArray<u8, U32>) -> Self {
		Self(Array([
			key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9],
			key[10], key[11], key[12], key[13], key[14], key[15], key[16], key[17], key[18],
			key[19], key[20], key[21], key[22], key[23], key[24], key[25], key[26], key[27],
			key[28], key[29], key[30], key[31],
		]))
	}
}

/// We take only the first 32 bytes of the key, since the rest doesn't fit
impl From<GenericArray<u8, U64>> for SecretKey {
	fn from(key: GenericArray<u8, U64>) -> Self {
		Self(Array([
			key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9],
			key[10], key[11], key[12], key[13], key[14], key[15], key[16], key[17], key[18],
			key[19], key[20], key[21], key[22], key[23], key[24], key[25], key[26], key[27],
			key[28], key[29], key[30], key[31],
		]))
	}
}

#[cfg(test)]
mod tests {
	use std::pin::pin;

	use futures::StreamExt;
	use rand::RngCore;

	use crate::primitives::EncryptedBlock;

	use super::*;

	#[test]
	fn one_shot_test() {
		use super::super::{decrypt::OneShotDecryption, encrypt::OneShotEncryption};
		let mut rng = CryptoRng::new().unwrap();

		let message = b"Eu queria um apartamento no Guarujah; \
		Mas o melhor que eu consegui foi um barraco em Itaquah.";

		let key = SecretKey::generate(&mut rng);

		let encrypted_block = key.encrypt(message, &mut rng).unwrap();
		let decrypted_message = key.decrypt_owned(&encrypted_block).unwrap();

		assert_eq!(message, decrypted_message.as_slice());
	}

	#[test]
	fn one_shot_ref_test() {
		use super::super::{decrypt::OneShotDecryption, encrypt::OneShotEncryption};
		let mut rng = CryptoRng::new().unwrap();

		let message = b"Eu queria um apartamento no Guarujah; \
		Mas o melhor que eu consegui foi um barraco em Itaquah.";

		let key = SecretKey::generate(&mut rng);

		let EncryptedBlock { nonce, cipher_text } = key.encrypt(message, &mut rng).unwrap();

		let mut bytes = Vec::with_capacity(nonce.len() + cipher_text.len());
		bytes.extend_from_slice(nonce.as_slice());
		bytes.extend(cipher_text);

		assert_eq!(
			bytes.len(),
			OneShotEncryption::cipher_text_size(&key, message.len())
		);

		let decrypted_message = key.decrypt(bytes.as_slice().into()).unwrap();

		assert_eq!(message, decrypted_message.as_slice());
	}

	async fn stream_test(rng: &mut CryptoRng, message: &[u8]) {
		use super::super::{decrypt::StreamDecryption, encrypt::StreamEncryption};

		let key = SecretKey::generate(rng);

		let mut encrypted_message = vec![];

		let (nonce, stream) = key.encrypt(message, rng);

		let mut stream = pin!(stream);

		while let Some(res) = stream.next().await {
			encrypted_message.extend(res.unwrap());
		}

		let mut decrypted_message = vec![];

		key.decrypt(&nonce, encrypted_message.as_slice(), &mut decrypted_message)
			.await
			.unwrap();

		assert_eq!(message, decrypted_message.as_slice());
	}

	#[tokio::test]
	async fn stream_test_small() {
		let message = b"Eu sou cagado, veja so como eh que eh; \
		Se der uma chuva de Xuxa, no meu colo cai Peleh; \
		E como aquele ditado que jah dizia; \
		Pau que nasce torto mija fora da bacia";

		stream_test(&mut CryptoRng::new().unwrap(), message).await;
	}

	#[tokio::test]
	async fn stream_test_big() {
		let mut rng = CryptoRng::new().unwrap();

		let mut message =
			vec![0u8; EncryptedBlock::PLAIN_TEXT_SIZE * 10 + EncryptedBlock::PLAIN_TEXT_SIZE / 2];

		rng.fill_bytes(&mut message);

		stream_test(&mut rng, &message).await;
	}

	#[tokio::test]
	async fn stream_test_big_exact() {
		let mut rng = CryptoRng::new().unwrap();

		let mut message = vec![0u8; EncryptedBlock::PLAIN_TEXT_SIZE * 20];

		rng.fill_bytes(&mut message);

		stream_test(&mut rng, &message).await;
	}
}
