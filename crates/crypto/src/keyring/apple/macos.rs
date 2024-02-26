//! This is Spacedrive's `MacOS` keyring integration. It has no strict dependencies.
use crate::{
	keyring::{Identifier, KeyringBackend, KeyringInterface},
	Error, Protected, Result,
};
use security_framework::os::macos::keychain::SecKeychain;

pub struct MacosKeyring {
	inner: SecKeychain,
}

impl KeyringInterface for MacosKeyring {
	fn new() -> Result<Self> {
		Ok(Self {
			inner: SecKeychain::default()?,
		})
	}

	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		Ok(self
			.inner
			.find_generic_password(&id.application(), &id.as_apple_identifer())
			.map_err(Error::AppleKeyring)?
			.0
			.to_owned()
			.into())
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.inner
			.find_generic_password(&id.application(), &id.as_apple_identifer())
			.map_or(false, |_| true)
	}

	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		self.inner
			.set_generic_password(&id.application(), &id.as_apple_identifer(), value.expose())
			.map_err(Error::AppleKeyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner
			.find_generic_password(&id.application(), &id.as_apple_identifer())
			.map_err(Error::AppleKeyring)?
			.1
			.delete();

		Ok(())
	}

	fn name(&self) -> KeyringBackend {
		KeyringBackend::MacOS
	}
}
