//! This is Spacedrive's Apple OS keychain integration. It has no strict dependencies.
//!
//! This has been tested on macOS, but should work just the same for iOS (according to the `security_framework` documentation)

use super::{Identifier, Keyring};
use crate::{Error, Protected, Result};
use security_framework::passwords::{
	delete_generic_password, get_generic_password, set_generic_password,
};

pub struct AppleKeyring;

impl Keyring for AppleKeyring {
	fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		set_generic_password(
			identifier.application,
			&identifier.to_apple_account(),
			value.expose().as_bytes(),
		)
		.map_err(Error::AppleKeyringError)
	}
	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		get_generic_password(identifier.application, &identifier.to_apple_account())
			.map(Protected::new)
			.map_err(Error::AppleKeyringError)
	}
	fn delete(&self, identifier: Identifier) -> Result<()> {
		delete_generic_password(identifier.application, &identifier.to_apple_account())
			.map_err(Error::AppleKeyringError)
	}
}
