#![cfg(any(target_os = "macos", target_os = "ios"))]
//! This is Spacedrive's Apple OS keychain integration. It has no strict dependencies.
use super::{Identifier, Keyring};
use crate::{Error, Protected, Result};
use security_framework::passwords::{
	delete_generic_password, get_generic_password, set_generic_password,
};
use users::get_current_username;

pub struct AppleKeyring {
	username: String,
}

fn get_username() -> Result<String> {
	get_current_username().map_or(Err(Error::KeyringError), |x| {
		Ok(x.to_str()
			.map_or(Err(Error::KeyringError), |x| Ok(x.to_string())))
	})?
}

impl AppleKeyring {
	pub fn new() -> Result<Self> {
		let k = Self {
			username: get_username()?,
		};

		Ok(k)
	}
}

impl Keyring for AppleKeyring {
	fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		set_generic_password(
			&identifier.to_apple_service(),
			&self.username,
			value.expose().as_bytes(),
		)
		.map_err(Error::AppleKeyringError)?;

		Ok(())
	}
	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		let pass = get_generic_password(&identifier.to_apple_service(), &self.username)
			.map(Protected::new)
			.map_err(Error::AppleKeyringError)?;

		Ok(pass)
	}
	fn delete(&self, identifier: Identifier) -> Result<()> {
		delete_generic_password(&identifier.to_apple_service(), &self.username)
			.map_err(Error::AppleKeyringError)?;

		Ok(())
	}
}
