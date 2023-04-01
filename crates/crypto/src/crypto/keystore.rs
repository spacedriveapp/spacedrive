use std::{collections::HashMap, hash::Hash, sync::Mutex};

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
	crypto::{Decryptor, Encryptor},
	hashing::Hasher,
	types::{Aad, Algorithm, DerivationContext, Key, Nonce, Salt},
	Error, Result,
};

const KEYSTORE_CONTEXT: DerivationContext =
	DerivationContext::new("crypto 2023-03-31 18:47:21 encrypted keystore context");

const KEYSTORE_ITEM_LIMIT: usize = 64;

pub struct Keystore<K>
where
	K: Hash + Eq,
{
	key: Key,
	algorithm: Algorithm,
	inner: Mutex<HashMap<K, KeystoreItem>>,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct KeystoreItem(#[zeroize(skip)] Salt, #[zeroize(skip)] Nonce, Vec<u8>);

impl<K> Keystore<K>
where
	K: Hash + Eq,
{
	#[must_use]
	pub fn new(algorithm: Algorithm) -> Self {
		Self {
			key: Key::generate(),
			algorithm,
			inner: Mutex::new(HashMap::new()),
		}
	}

	#[must_use]
	pub fn new_with_key(key: Key, algorithm: Algorithm) -> Self {
		Self {
			key,
			algorithm,
			inner: Mutex::new(HashMap::new()),
		}
	}

	pub fn contains_key(&self, id: &K) -> bool {
		self.inner
			.lock()
			.map_err(|_| Error::Keystore)
			.map_or(false, |x| x.contains_key(id))
	}

	pub fn get(&self, id: &K) -> Result<Vec<u8>> {
		let item = self
			.inner
			.lock()
			.map_err(|_| Error::Keystore)?
			.get(id)
			.ok_or(Error::Keystore)?
			.clone();

		let value = Decryptor::decrypt_tiny(
			Hasher::derive_key(self.key.clone(), item.0, KEYSTORE_CONTEXT),
			item.1,
			self.algorithm,
			&item.2,
			Aad::Null,
		)?;

		Ok(value.into_inner())
	}

	#[allow(clippy::needless_pass_by_value)]
	pub fn insert(&self, id: K, value: Vec<u8>) -> Result<()> {
		// so `value` is dropped once it goes out of scope, but doesn't need to be mutable
		let value = Zeroizing::new(value);

		if self.inner.lock().map_err(|_| Error::Keystore)?.len() + 1 > KEYSTORE_ITEM_LIMIT {
			return Err(Error::Keystore);
		}

		let salt = Salt::generate();
		let nonce = Nonce::generate(self.algorithm);

		let bytes = Encryptor::encrypt_tiny(
			Hasher::derive_key(self.key.clone(), salt, KEYSTORE_CONTEXT),
			nonce,
			self.algorithm,
			&value,
			Aad::Null,
		)?;

		self.inner
			.lock()
			.map_err(|_| Error::Keystore)?
			.insert(id, KeystoreItem(salt, nonce, bytes));

		Ok(())
	}

	pub fn remove(&self, id: &K) -> Result<()> {
		self.inner
			.lock()
			.map_err(|_| Error::Keystore)?
			.remove(id)
			.map_or_else(|| Err(Error::Keystore), |_| Ok(()))
	}
}
