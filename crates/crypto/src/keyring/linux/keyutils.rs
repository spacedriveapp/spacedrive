//! This is Spacedrive's Linux keyring implementation, which makes use of the `keyutils` API (provided by modern Linux kernels).
use crate::keyring::{Identifier, KeyringBackend, KeyringInterface, LinuxKeyring};
use crate::{Protected, Result};
use linux_keyutils::{KeyPermissionsBuilder, KeyRing, KeyRingIdentifier, Permission};

pub struct KeyutilsKeyring {
	session: KeyRing,
	persistent: KeyRing,
}

const WEEK: usize = 604_800;

impl KeyutilsKeyring {
	pub fn new() -> Result<Self> {
		Ok(Self {
			session: KeyRing::from_special_id(KeyRingIdentifier::Session, false)?,
			persistent: KeyRing::get_persistent(KeyRingIdentifier::Session)?,
		})
	}
}

impl KeyringInterface for KeyutilsKeyring {
	fn new() -> Result<Self> {
		Self::new()
	}

	fn name(&self) -> KeyringBackend {
		KeyringBackend::Linux(LinuxKeyring::Keyutils)
	}

	fn contains_key(&self, id: &Identifier) -> bool {
		self.session.search(&id.hash()).map_or(false, |_| true)
	}

	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		let key = self.session.search(&id.hash())?;

		self.session.link_key(key)?;
		self.persistent.link_key(key)?;

		Ok(Protected::new(key.read_to_vec()?))
	}

	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		let key = self.session.add_key(&id.hash(), value.expose())?;
		key.set_timeout(WEEK)?;

		let perms = KeyPermissionsBuilder::builder()
			.posessor(Permission::ALL)
			.user(Permission::empty())
			.group(Permission::empty())
			.world(Permission::empty())
			.build();

		key.set_perms(perms)?;

		self.persistent.link_key(key)?;

		Ok(())
	}

	fn remove(&self, id: &Identifier) -> Result<()> {
		let key = self.session.search(&id.hash())?;

		key.invalidate()?;

		Ok(())
	}
}
