//! This is Spacedrive's iOS keyring integration. It has no strict dependencies.
use super::{Identifier, KeyringInterface};
use crate::{Error, Result};
use security_framework::passwords::{
	delete_generic_password, get_generic_password, set_generic_password,
};

impl Identifier {
	#[must_use]
	pub fn to_apple_account(&self) -> String {
		format!("{} - {}", self.id, self.usage)
	}
}

pub struct IosKeyring;

impl KeyringInterface for IosKeyring {
	fn new() -> Result<Self>
	where
		Self: Sized,
	{
		Ok(Self {})
	}

	fn get(&self, id: &Identifier) -> Result<String> {
		let key = get_generic_password(&id.application, &id.to_apple_account())
			.map_err(Error::AppleKeyring)?;

		String::from_utf8(key).map_err(|_| Error::KeyringError)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		get_generic_password(&id.application, &id.to_apple_account()).map_or(false, |_| true)
	}

	fn insert(&self, id: &Identifier, value: String) -> Result<()> {
		set_generic_password(&id.application, &id.to_apple_account(), value.as_bytes())
			.map_err(Error::AppleKeyring)
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		delete_generic_password(&id.application, &id.to_apple_account())
			.map_err(Error::AppleKeyring)
	}

	fn name(&self) -> KeyringName {
		KeyringName::Ios
	}
}
