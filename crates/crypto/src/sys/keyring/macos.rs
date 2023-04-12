//! This is Spacedrive's `MacOS` keyring integration. It has no strict dependencies.
use super::{Identifier, KeyringInterface, KeyringName};
use crate::{Error, Protected, Result};
use security_framework::os::macos::keychain::SecKeychain;

impl Identifier {
	#[must_use]
	pub fn to_apple_account(&self) -> String {
		format!("{} - {}", self.id, self.usage)
	}
}

pub struct MacosKeyring {
	inner: SecKeychain,
}

impl KeyringInterface for MacosKeyring {
	fn new() -> Result<Self> {
		Ok(Self {
			inner: SecKeychain::default()?,
		})
	}

	fn get(&self, id: &Identifier) -> Result<Protected<String>> {
		let key = self
			.inner
			.find_generic_password(&id.application, &id.to_apple_account())
			.map_err(Error::AppleKeyring)?
			.0
			.to_owned();

		String::from_utf8(key)
			.map(Protected::new)
			.map_err(|_| Error::Keyring)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.inner
			.find_generic_password(&id.application, &id.to_apple_account())
			.map_or(false, |_| true)
	}

	fn insert(&self, id: &Identifier, value: Protected<String>) -> Result<()> {
		self.inner
			.set_generic_password(
				&id.application,
				&id.to_apple_account(),
				value.expose().as_bytes(),
			)
			.map_err(Error::AppleKeyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner
			.find_generic_password(&id.application, &id.to_apple_account())
			.map_err(Error::AppleKeyring)?
			.1
			.delete();

		Ok(())
	}

	fn name(&self) -> KeyringName {
		KeyringName::MacOS
	}
}
