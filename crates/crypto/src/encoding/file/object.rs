use crate::{
	crypto::{Decryptor, Encryptor},
	hashing::Hasher,
	types::{Aad, Algorithm, DerivationContext, EncryptedKey, Key, Nonce, Salt},
	Protected, Result,
};

#[derive(Clone)]
pub struct HeaderObjectIdentifier {
	pub(super) key: EncryptedKey, // technically a key, although used as an identifier here
	pub(super) salt: Salt,
}

pub struct HeaderObject {
	pub identifier: HeaderObjectIdentifier,
	pub nonce: Nonce,
	pub data: Vec<u8>,
}

impl HeaderObject {
	pub fn new(
		name: &'static str,
		algorithm: Algorithm,
		master_key: &Key,
		context: DerivationContext,
		aad: Aad,
		data: &[u8],
	) -> Result<Self> {
		let identifier = HeaderObjectIdentifier::new(name, master_key, algorithm, context, aad)?;

		let nonce = Nonce::generate(algorithm);
		let encrypted_data = Encryptor::encrypt_bytes(master_key, &nonce, algorithm, data, aad)?;

		let object = Self {
			identifier,
			nonce,
			data: encrypted_data,
		};

		Ok(object)
	}

	pub(super) fn decrypt(
		&self,
		algorithm: Algorithm,
		aad: Aad,
		master_key: &Key,
	) -> Result<Protected<Vec<u8>>> {
		Decryptor::decrypt_bytes(master_key, &self.nonce, algorithm, &self.data, aad)
	}
}

impl HeaderObjectIdentifier {
	pub fn new(
		name: &'static str,
		master_key: &Key,
		algorithm: Algorithm,
		context: DerivationContext,
		aad: Aad,
	) -> Result<Self> {
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);

		let encrypted_key = Encryptor::encrypt_key(
			&Hasher::derive_key(master_key, salt, context),
			&nonce,
			algorithm,
			&Hasher::blake3(name.as_bytes()),
			aad,
		)?;

		Ok(Self {
			key: encrypted_key,
			salt,
		})
	}

	pub(super) fn decrypt_id(
		&self,
		master_key: &Key,
		algorithm: Algorithm,
		context: DerivationContext,
		aad: Aad,
	) -> Result<Key> {
		Decryptor::decrypt_key(
			&Hasher::derive_key(master_key, self.salt, context),
			algorithm,
			&self.key,
			aad,
		)
	}
}
