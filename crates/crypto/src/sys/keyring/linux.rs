//! This is Spacedrive's Linux keyring implementation, which makes use of the Secret Service API.
//!
//! This does strictly require `DBus`, and either `gnome-keyring`, `kwallet` or another implementor of the Secret Service API.

use secret_service::{Collection, EncryptionType, SecretService};

use super::{Identifier, Keyring};
use crate::{Error, Protected, Result};

impl<'a> Identifier<'a> {
	#[must_use]
	pub fn to_hashmap(self) -> std::collections::HashMap<&'a str, &'a str> {
		[
			("Application", self.application),
			("ID", self.id),
			("Usage", self.usage),
		]
		.into_iter()
		.collect()
	}

	#[must_use]
	pub fn generate_linux_label(&self) -> String {
		format!("{} - {}", self.application, self.usage)
	}
}

pub struct LinuxKeyring<'a> {
	pub service: SecretService<'a>,
}

impl<'a> LinuxKeyring<'a> {
	fn get_collection(&self) -> Result<Collection<'_>> {
		let collection = self.service.get_default_collection()?;

		if collection.is_locked()? {
			collection.unlock()?;
		}

		Ok(collection)
	}
}

impl<'a> Keyring for LinuxKeyring<'a> {
	fn new() -> Result<Self>
	where
		Self: Sized,
	{
		Ok(Self {
			service: SecretService::new(EncryptionType::Dh)?,
		})
	}

	fn insert(&self, identifier: Identifier<'_>, value: Protected<Vec<u8>>) -> Result<()> {
		self.get_collection()?.create_item(
			&identifier.generate_linux_label(),
			identifier.to_hashmap(),
			value.expose(),
			true,
			"text/plain",
		)?;

		Ok(())
	}

	fn retrieve(&self, identifier: Identifier<'_>) -> Result<Protected<Vec<u8>>> {
		let collection = self.get_collection()?;
		let items = collection.search_items(identifier.to_hashmap())?;

		items
			.get(0)
			.map_or(Err(Error::KeyringError), |k| Ok(k.get_secret()?.into()))
	}

	fn delete(&self, identifier: Identifier<'_>) -> Result<()> {
		self.get_collection()?
			.search_items(identifier.to_hashmap())?
			.get(0)
			.map_or(Err(Error::KeyringError), |k| {
				k.delete()?;
				Ok(())
			})
	}
}
