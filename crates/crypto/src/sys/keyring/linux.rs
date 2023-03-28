//! This is Spacedrive's Linux keyring implementation, which makes use of the `keyutils` API (provided by modern Linux kernels).
use linux_keyutils::{KeyPermissionsBuilder, KeyRing, KeyRingIdentifier, Permission};

use crate::{Error, Protected, Result};

use super::{Identifier, KeyringInterface, KeyringName};

pub struct LinuxKeyring {
	session: KeyRing,
	persistent: KeyRing,
}

const WEEK: usize = 604_800;

impl LinuxKeyring {
	pub fn new() -> Result<Self> {
		let session = KeyRing::from_special_id(KeyRingIdentifier::Session, false)?;
		let persistent = KeyRing::get_persistent(KeyRingIdentifier::Session)?;

		let s = Self {
			session,
			persistent,
		};

		Ok(s)
	}
}

impl KeyringInterface for LinuxKeyring {
	fn new() -> Result<Self> {
		let session = KeyRing::from_special_id(KeyRingIdentifier::Session, false)?;
		let persistent = KeyRing::get_persistent(KeyRingIdentifier::Session)?;

		let s = Self {
			session,
			persistent,
		};

		Ok(s)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.session.search(&id.hash()).map_or(false, |_| true)
	}

	fn get(&self, id: &Identifier) -> Result<Protected<String>> {
		let key = self.session.search(&id.hash())?;

		self.session.link_key(key)?;
		self.persistent.link_key(key)?;

		let buffer = key.read_to_vec()?;

		String::from_utf8(buffer)
			.map(Protected::new)
			.map_err(|_| Error::KeyringError)
	}

	fn insert(&self, id: &Identifier, value: Protected<String>) -> Result<()> {
		let key = self.session.add_key(&id.hash(), value.expose())?;
		key.set_timeout(WEEK)?;

		let p = KeyPermissionsBuilder::builder()
			.posessor(Permission::ALL)
			.user(Permission::ALL)
			.group(Permission::VIEW | Permission::READ)
			.build();

		key.set_perms(p)?;

		self.persistent.link_key(key)?;

		Ok(())
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		let key = self.session.search(&id.hash())?;

		key.invalidate()?;

		Ok(())
	}

	fn name(&self) -> KeyringName {
		KeyringName::Linux
	}
}
