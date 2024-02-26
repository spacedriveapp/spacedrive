use crate::{types::Algorithm, vault::EphemeralVault, Error, Protected, Result};

use super::{Identifier, KeyringBackend, KeyringInterface};

pub struct SessionKeyring {
	inner: EphemeralVault<String, Vec<u8>>,
}

impl KeyringInterface for SessionKeyring {
	fn new() -> Result<Self> {
		Ok(Self {
			inner: EphemeralVault::new(Algorithm::default()),
		})
	}

	fn name(&self) -> KeyringBackend {
		KeyringBackend::Session
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.inner.contains_key(&id.hash()).map_or(false, |x| x)
	}

	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		Ok(Protected::new(
			self.inner.get(&id.hash()).map_err(|_| Error::Keyring)?,
		))
	}

	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		self.inner
			.insert(id.hash(), value.into_inner())
			.map_err(|_| Error::Keyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner.remove(&id.hash()).map_err(|_| Error::Keyring)
	}
}
