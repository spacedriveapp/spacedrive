use crate::{crypto::keystore::Keystore, types::Algorithm, Error, Protected, Result};

use super::{Identifier, KeyringInterface, KeyringName};

pub struct PortableKeyring {
	inner: Keystore<String>,
}

impl KeyringInterface for PortableKeyring {
	fn new() -> Result<Self> {
		Ok(Self {
			inner: Keystore::new(Algorithm::default()),
		})
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.inner.contains_key(&id.hash()).map_or(false, |x| x)
	}

	fn get(&self, id: &Identifier) -> Result<Protected<String>> {
		let item = self.inner.get(&id.hash()).map_err(|_| Error::Keyring)?;

		String::from_utf8(item)
			.map(Protected::new)
			.map_err(|_| Error::Keyring)
	}

	fn insert(&self, id: &Identifier, value: Protected<String>) -> Result<()> {
		self.inner
			.insert(id.hash(), value.into_inner().into_bytes())
			.map_err(|_| Error::Keyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner.remove(&id.hash()).map_err(|_| Error::Keyring)
	}

	fn name(&self) -> KeyringName {
		KeyringName::Portable
	}
}
