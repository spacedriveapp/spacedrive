use crate::{hashing::Hasher, Protected, Result};
mod portable;
use portable::PortableKeyring;

#[cfg(not(any(target_os = "linux", target_os = "ios")))]
use portable::PortableKeyring as DefaultKeyring;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::LinuxKeyring as DefaultKeyring;

// #[cfg(target_os = "macos")]
// pub mod macos;
// #[cfg(target_os = "macos")]
// pub use macos::MacosKeyring as DefaultKeyring;

#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "ios")]
pub use ios::IosKeyring as DefaultKeyring;

pub(self) trait KeyringInterface {
	fn new() -> Result<Self>
	where
		Self: Sized;

	fn get(&self, id: &Identifier) -> Result<Protected<String>>;
	fn remove(&self, id: &Identifier) -> Result<()>;
	fn insert(&self, id: &Identifier, value: Protected<String>) -> Result<()>;
	fn contains_key(&self, id: &Identifier) -> bool;
	fn name(&self) -> KeyringName;
}

#[allow(dead_code)]
pub enum KeyringName {
	Portable,
	Linux,
	MacOS,
	Ios,
}

#[derive(Clone, Copy)]
pub enum KeyringType {
	Default,
	Portable,
}

#[derive(Clone)]
pub struct Identifier {
	id: String,
	usage: String,
	application: String,
}

impl Identifier {
	#[must_use]
	pub fn new(id: &'static str, usage: &'static str, application: &'static str) -> Self {
		Self {
			id: id.to_string(),
			usage: usage.to_string(),
			application: application.to_string(),
		}
	}

	#[must_use]
	pub fn hash(&self) -> String {
		format!(
			"{}:{}",
			self.application,
			Hasher::blake3_hex(&[self.id.as_bytes(), self.usage.as_bytes()].concat())
		)
	}
}

pub struct Keyring {
	inner: Box<dyn KeyringInterface + Send + Sync>,
}

impl Keyring {
	pub fn new(backend: KeyringType) -> Result<Self> {
		let kr = match backend {
			KeyringType::Default => Self {
				inner: Box::new(DefaultKeyring::new()?),
			},
			KeyringType::Portable => Self {
				inner: Box::new(PortableKeyring::new()?),
			},
		};

		Ok(kr)
	}

	pub fn get(&self, id: &Identifier) -> Result<Protected<String>> {
		self.inner.get(id)
	}

	#[must_use]
	pub fn contains_key(&self, id: &Identifier) -> bool {
		self.inner.contains_key(id)
	}

	pub fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner.remove(id)
	}

	pub fn insert(&self, id: &Identifier, value: Protected<String>) -> Result<()> {
		self.inner.insert(id, value)
	}

	#[must_use]
	pub fn name(&self) -> KeyringName {
		self.inner.name()
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::Protected;

	use super::{Identifier, Keyring, KeyringType};

	#[test]
	fn full_portable() {
		let password = Protected::new("SuperSecurePassword".to_string());
		let identifier = Identifier::new("0000-0000-0000-0000", "Password", "Crypto");
		let keyring = Keyring::new(KeyringType::Portable).unwrap();

		keyring.insert(&identifier, password.clone()).unwrap();
		assert!(keyring.contains_key(&identifier));

		let pw = keyring.get(&identifier).unwrap();

		assert_eq!(pw.expose(), password.expose());

		keyring.remove(&identifier).unwrap();

		assert!(!keyring.contains_key(&identifier));
	}
}
