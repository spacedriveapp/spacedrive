//! This is Spacedrive's `iOS` keyring integration. It has no strict dependencies.
use crate::{
	keyring::{Identifier, KeyringBackend, KeyringInterface},
	Error, Protected, Result,
};
use security_framework::passwords::{
	delete_generic_password, get_generic_password, set_generic_password,
};

pub struct IosKeyring;

impl KeyringInterface for IosKeyring {
	fn new() -> Result<Self> {
		Ok(Self {})
	}

	fn name(&self) -> KeyringBackend {
		KeyringBackend::Ios
	}

	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		get_generic_password(&id.application(), &id.as_apple_identifer())
			.map_err(Error::AppleKeyring)
			.map(Into::into)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		get_generic_password(&id.application(), &id.as_apple_identifer()).map_or(false, |_| true)
	}

	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		set_generic_password(&id.application(), &id.as_apple_identifer(), value.expose())
			.map_err(Error::AppleKeyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		delete_generic_password(&id.application(), &id.as_apple_identifer())
			.map_err(Error::AppleKeyring)
	}
}
