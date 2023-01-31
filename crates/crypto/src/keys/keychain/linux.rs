//! This is Spacedrive's Linux keychain implementation, which makes use of the Secret Service API.
//!
//! This does strictly require `DBus`, and either `gnome-keyring`, `kwallet` or another implementor of the Secret Service API.

#![cfg(target_os = "linux")]
use secret_service::{Collection, EncryptionType, SecretService};

use crate::{
	keys::keychain::{Identifier, Keyring},
	Error, Protected, Result,
};

pub struct LinuxKeyring<'a> {
	pub service: SecretService<'a>,
}

impl<'a> LinuxKeyring<'a> {
	pub fn new() -> Result<Self> {
		Ok(Self {
			service: SecretService::new(EncryptionType::Dh)?,
		})
	}

	fn get_collection(&self) -> Result<Collection> {
		let collection = self.service.get_default_collection()?;

		collection.is_locked()?.then(|| {
			collection.unlock()?;
			Ok::<_, Error>(())
		});

		Ok(collection)
	}
}

impl<'a> Keyring for LinuxKeyring<'a> {
	fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		self.get_collection()?.create_item(
			&identifier.generate_linux_label(),
			identifier.to_hashmap(),
			value.expose().as_bytes(),
			true,
			"text/plain",
		)?;

		Ok(())
	}

	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		let collection = self.get_collection()?;
		let items = collection.search_items(identifier.to_hashmap())?;

		items.get(0).map_or(Err(Error::KeyringError), |k| {
			Ok(Protected::new(k.get_secret()?))
		})
	}

	fn delete(&self, identifier: Identifier) -> Result<()> {
		self.get_collection()?
			.search_items(identifier.to_hashmap())?
			.get(0)
			.map_or(Err(Error::KeyringError), |k| {
				k.delete()?;
				Ok(())
			})
	}
}
