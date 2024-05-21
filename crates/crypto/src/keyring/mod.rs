use crate::{Error, Protected, Result};
use std::fmt::Display;

mod identifier;
mod session;

use identifier::Identifier;
use session::SessionKeyring;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;

// #[cfg(target_os = "windows")]
// mod windows;

const MAX_LEN: usize = 128;

// TODO(brxken128): use `Encrypted<T>` type here?

pub trait KeyringInterface {
	fn new() -> Result<Self>
	where
		Self: Sized;
	fn name(&self) -> KeyringBackend;
	fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>>;
	fn remove(&self, id: &Identifier) -> Result<()>;
	fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()>;
	fn contains_key(&self, id: &Identifier) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyringBackend {
	Session,
	#[cfg(target_os = "macos")]
	MacOS,
	#[cfg(target_os = "ios")]
	Ios,
	#[cfg(target_os = "linux")]
	Linux(LinuxKeyring),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LinuxKeyring {
	#[cfg(target_os = "linux")]
	Keyutils,
	#[cfg(all(target_os = "linux", feature = "secret-service"))]
	SecretService,
}

impl Display for KeyringBackend {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			Self::Session => "Session",
			#[cfg(target_os = "macos")]
			Self::MacOS => "MacOS",
			#[cfg(target_os = "ios")]
			Self::Ios => "iOS",
			#[cfg(target_os = "linux")]
			Self::Linux(k) => match k {
				LinuxKeyring::Keyutils => "KeyUtils",
				#[cfg(feature = "secret-service")]
				LinuxKeyring::SecretService => "Secret Service",
			},
		};

		f.write_str(s)
	}
}

pub struct Keyring {
	inner: Box<dyn KeyringInterface + Send + Sync>,
}

impl Keyring {
	pub fn new(backend: KeyringBackend) -> Result<Self> {
		let inner: Box<dyn KeyringInterface + Send + Sync> = match backend {
			KeyringBackend::Session => Box::new(SessionKeyring::new()?),
			#[cfg(target_os = "macos")]
			KeyringBackend::MacOS => Box::new(apple::MacosKeyring::new()?),
			#[cfg(target_os = "Ios")]
			KeyringBackend::Ios => Box::new(apple::IosKeyring::new()?),
			#[cfg(target_os = "linux")]
			KeyringBackend::Linux(k) => match k {
				LinuxKeyring::Keyutils => Box::new(linux::KeyutilsKeyring::new()?),
				#[cfg(feature = "secret-service")]
				LinuxKeyring::SecretService => Box::new(linux::SecretServiceKeyring::new()?),
			},
		};

		Ok(Self { inner })
	}

	#[inline]
	pub fn get(&self, id: &Identifier) -> Result<Protected<Vec<u8>>> {
		self.inner.get(id)
	}

	#[inline]
	#[must_use]
	pub fn contains_key(&self, id: &Identifier) -> bool {
		self.inner.contains_key(id)
	}

	#[inline]
	pub fn remove(&self, id: &Identifier) -> Result<()> {
		self.inner.remove(id)
	}

	#[inline]
	pub fn insert(&self, id: &Identifier, value: Protected<Vec<u8>>) -> Result<()> {
		if value.expose().len() > MAX_LEN {
			return Err(Error::Validity); // should be "value too long"
		}

		self.inner.insert(id, value)
	}

	#[inline]
	#[must_use]
	pub fn name(&self) -> KeyringBackend {
		self.inner.name()
	}
}

#[cfg(test)]
mod tests {
	use crate::Protected;

	use super::{Identifier, Keyring, KeyringBackend};

	#[test]
	fn full_session() {
		let password = Protected::new(b"SuperSecurePassword".to_vec());
		let identifier = Identifier::new("0000-0000-0000-0000", "Password", "Crypto");
		let keyring = Keyring::new(KeyringBackend::Session).unwrap();

		keyring.insert(&identifier, password.clone()).unwrap();
		assert!(keyring.contains_key(&identifier));

		let pw = keyring.get(&identifier).unwrap();

		assert_eq!(pw.expose(), password.expose());

		keyring.remove(&identifier).unwrap();

		assert!(!keyring.contains_key(&identifier));
	}

	#[test]
	#[cfg(target_os = "linux")]
	#[ignore]
	fn linux_keyutils() {
		let password = Protected::new(b"SuperSecurePassword".to_vec());
		let identifier = Identifier::new("0000-0000-0000-0000", "Password", "Crypto");
		let keyring = Keyring::new(KeyringBackend::Linux(super::LinuxKeyring::Keyutils)).unwrap();

		keyring.insert(&identifier, password.clone()).unwrap();
		assert!(keyring.contains_key(&identifier));

		let pw = keyring.get(&identifier).unwrap();

		assert_eq!(pw.expose(), password.expose());

		keyring.remove(&identifier).unwrap();

		assert!(!keyring.contains_key(&identifier));
	}

	#[test]
	#[cfg(target_os = "linux")]
	#[ignore]
	fn linux_secret_service() {
		let password = Protected::new(b"SuperSecurePassword".to_vec());
		let identifier = Identifier::new("0000-0000-0000-0000", "Password", "Crypto");
		let keyring =
			Keyring::new(KeyringBackend::Linux(super::LinuxKeyring::SecretService)).unwrap();

		keyring.insert(&identifier, password.clone()).unwrap();
		assert!(keyring.contains_key(&identifier));

		let pw = keyring.get(&identifier).unwrap();

		assert_eq!(pw.expose(), password.expose());

		keyring.remove(&identifier).unwrap();

		assert!(!keyring.contains_key(&identifier));
	}

	#[test]
	#[cfg(target_os = "macos")]
	#[ignore]
	fn macos() {
		let password = Protected::new(b"SuperSecurePassword".to_vec());
		let identifier = Identifier::new("0000-0000-0000-0000", "Password", "Crypto");
		let keyring = Keyring::new(KeyringBackend::MacOS).unwrap();

		keyring.insert(&identifier, password.clone()).unwrap();
		assert!(keyring.contains_key(&identifier));

		let pw = keyring.get(&identifier).unwrap();

		assert_eq!(pw.expose(), password.expose());

		keyring.remove(&identifier).unwrap();

		assert!(!keyring.contains_key(&identifier));
	}
}
