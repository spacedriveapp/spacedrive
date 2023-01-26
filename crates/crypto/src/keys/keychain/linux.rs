#![cfg(target_os = "linux")]
//! This is Spacedrive's Linux keychain implementation, which makes use of the Secret Service API.
//!
//! This does strictly require DBus.

use secret_service::{Collection, EncryptionType, SecretService};

use crate::{
	keys::keychain::{Identifier, Keyring},
	Protected, Result,
};

pub struct LinuxKeyring<'a> {
	// username: String,
	pub service: SecretService<'a>,
}

impl<'a> LinuxKeyring<'a> {
	pub fn new() -> Self {
		Self {
			service: SecretService::new(EncryptionType::Dh).unwrap(),
		}
	}

	fn get_collection(&self) -> Collection {
		let collection = self.service.get_default_collection().unwrap();

		if collection.is_locked().unwrap() {
			collection.unlock().unwrap();
		}

		collection
	}
}

impl<'a> Keyring for LinuxKeyring<'a> {
	fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		let collection = self.get_collection();

		collection
			.create_item(
				&(identifier.application.to_string() + ":" + identifier.usage),
				identifier.to_hashmap(),
				value.expose().as_bytes(),
				true,
				"text/plain",
			)
			.unwrap();

		Ok(())
	}

	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		let collection = self.get_collection();

		let item = collection.search_items(identifier.to_hashmap()).unwrap();

		Ok(Protected::new(item.get(0).unwrap().get_secret().unwrap())) // can also get secret_type here
	}

	fn delete(&self, identifier: Identifier) -> Result<()> {
		let collection = self.get_collection();
		collection
			.search_items(identifier.to_hashmap())
			.unwrap()
			.get(0)
			.unwrap()
			.delete()
			.unwrap();

		Ok(())
	}
}
