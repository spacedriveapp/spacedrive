use zeroize::{Zeroize, Zeroizing};

use crate::{
	encrypted::Encrypted,
	types::{Algorithm, Key},
	Error, Result,
};
use std::{collections::HashMap, hash::Hash, sync::Mutex};

pub const EPHEMERAL_VAULT_ITEM_LIMIT: usize = 128;

pub struct EphemeralVault<K, T>
where
	K: Hash + Eq,
{
	key: Key,
	algorithm: Algorithm,
	inner: Mutex<HashMap<K, Encrypted<T>>>,
}

impl<K, T> EphemeralVault<K, T>
where
	K: Hash + Eq,
	T: bincode::Encode + bincode::Decode + Zeroize + Clone,
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

	pub fn contains_key(&self, id: &K) -> Result<bool> {
		self.inner
			.lock()
			.map_or(Err(Error::Keystore), |x| Ok(x.contains_key(id)))
	}

	pub fn get(&self, id: &K) -> Result<T> {
		self.inner
			.lock()
			.map_err(|_| Error::Keystore)?
			.get(id)
			.cloned()
			.ok_or(Error::Keystore)?
			.decrypt(&self.key)
	}

	pub fn insert(&self, id: K, value: T) -> Result<()> {
		let value = Zeroizing::new(value);

		if self.inner.lock().map_err(|_| Error::Keystore)?.len() + 1 > EPHEMERAL_VAULT_ITEM_LIMIT {
			return Err(Error::Keystore);
		}

		let encrypted = Encrypted::new(&self.key.clone(), &*value, self.algorithm)?;

		self.inner
			.lock()
			.map_err(|_| Error::Keystore)?
			.insert(id, encrypted);

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
