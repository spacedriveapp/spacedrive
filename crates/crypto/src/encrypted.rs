use bincode::{Decode, Encode};
use std::marker::PhantomData;

use crate::{
	crypto::{Decryptor, Encryptor},
	encoding::{decode, encode},
	hashing::Hasher,
	primitives::ENCRYPTED_TYPE_CONTEXT,
	types::{Aad, Algorithm, Key, Nonce, Salt},
	Protected, Result,
};

#[derive(Clone, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Encrypted<T> {
	data: Vec<u8>,
	algorithm: Algorithm,
	nonce: Nonce,
	salt: Salt,
	#[cfg_attr(feature = "specta", specta(skip))]
	_type: PhantomData<T>,
}

impl<T> Encrypted<T> {
	pub fn new(key: &Key, item: &T, algorithm: Algorithm) -> Result<Self>
	where
		T: Encode + Decode,
	{
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);

		let bytes = Encryptor::encrypt_tiny(
			&Hasher::derive_key(key, salt, ENCRYPTED_TYPE_CONTEXT),
			&nonce,
			algorithm,
			&encode(item)?,
			Aad::Null,
		)?;

		Ok(Self {
			data: bytes,
			algorithm,
			salt,
			nonce,
			_type: PhantomData,
		})
	}

	pub fn new_from_bytes(
		key: &Key,
		item: &Protected<Vec<u8>>,
		algorithm: Algorithm,
	) -> Result<Self> {
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);

		let bytes = Encryptor::encrypt_tiny(
			&Hasher::derive_key(key, salt, ENCRYPTED_TYPE_CONTEXT),
			&nonce,
			algorithm,
			item.expose(),
			Aad::Null,
		)?;

		Ok(Self {
			data: bytes,
			algorithm,
			salt,
			nonce,
			_type: PhantomData,
		})
	}

	pub fn decrypt(self, key: &Key) -> Result<T>
	where
		T: Encode + Decode,
	{
		let bytes = Decryptor::decrypt_bytes(
			&Hasher::derive_key(key, self.salt, ENCRYPTED_TYPE_CONTEXT),
			&self.nonce,
			self.algorithm,
			&self.data,
			Aad::Null,
		)?
		.into_inner();

		decode(&bytes)
	}

	pub fn decrypt_bytes(self, key: &Key) -> Result<Protected<Vec<u8>>> {
		let bytes = Decryptor::decrypt_bytes(
			&Hasher::derive_key(key, self.salt, ENCRYPTED_TYPE_CONTEXT),
			&self.nonce,
			self.algorithm,
			&self.data,
			Aad::Null,
		)?
		.into_inner();

		Ok(bytes.into())
	}

	pub fn as_bytes(&self) -> Result<Vec<u8>>
	where
		T: Encode + Decode,
	{
		encode(&self)
	}

	// check if key is okay
	#[must_use]
	pub fn validate_key(&self, key: &Key) -> bool {
		Decryptor::decrypt_bytes(
			&Hasher::derive_key(key, self.salt, ENCRYPTED_TYPE_CONTEXT),
			&self.nonce,
			self.algorithm,
			&self.data,
			Aad::Null,
		)
		.is_ok()
	}

	#[must_use]
	pub fn get_bytes(&self) -> Vec<u8> {
		self.data.clone()
	}

	#[must_use]
	pub const fn get_salt(&self) -> Salt {
		self.salt
	}

	#[must_use]
	pub const fn get_nonce(&self) -> Nonce {
		self.nonce
	}

	#[must_use]
	pub const fn get_algorithm(&self) -> Algorithm {
		self.algorithm
	}
}
