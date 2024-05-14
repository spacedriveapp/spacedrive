//! This is Spacedrive's Linux keyring implementation, which makes use of the `secret-service` API (provided by `gnome-passwords` and `kwallet`).
use crate::keyring::{Identifier, KeyringBackend, KeyringInterface, LinuxKeyring};
use crate::{Error, Protected, Result};
use secret_service::blocking::{Collection, SecretService};
use secret_service::EncryptionType;

pub struct SecretServiceKeyring {
	session: SecretService<'static>,
}

impl SecretServiceKeyring {
	fn new() -> Result<Self> {
		Ok(Self {
			session: SecretService::connect(EncryptionType::Dh)?,
		})
	}

	fn get_collection(&self) -> Result<Collection<'_>> {
		let k = self.session.get_default_collection()?;
		k.unlock()?;

		Ok(k)
	}
}

impl KeyringInterface for SecretServiceKeyring {
	fn new() -> Result<Self> {
		Self::new()
	}

	fn name(&self) -> KeyringBackend {
		KeyringBackend::Linux(LinuxKeyring::SecretService)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.get_collection().ok().is_some_and(|k| {
			k.search_items(id.as_sec_ser_identifier())
				.ok()
				.map_or(false, |x| !x.is_empty())
		})
	}

	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		self.get_collection()?
			.search_items(id.as_sec_ser_identifier())?
			.first()
			.map_or(Err(Error::Keyring), |k| {
				Ok(Protected::new(hex::decode(k.get_secret()?)?))
			})
	}

	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		self.get_collection()?.create_item(
			&id.application(),
			id.as_sec_ser_identifier(),
			hex::encode(value.expose()).as_bytes(),
			true,
			"text/plain",
		)?;

		Ok(())
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		self.get_collection()?
			.search_items(id.as_sec_ser_identifier())?
			.first()
			.map_or(Err(Error::Keyring), |k| {
				k.delete()?;
				Ok(())
			})
	}
}
