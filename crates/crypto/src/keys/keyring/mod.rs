use crate::{primitives::types::SecretKeyString, Protected, Result};

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;

/// This identifier is platform-agnostic and is used for identifying keys within OS keyrings
#[derive(Clone, Copy)]
pub struct Identifier<'a> {
	pub application: &'a str,
	pub library_uuid: &'a str,
	pub usage: &'a str,
}

impl<'a> Identifier<'a> {
	#[cfg(target_os = "linux")]
	#[must_use]
	pub fn to_hashmap(self) -> std::collections::HashMap<&'a str, &'a str> {
		[
			("Application", self.application),
			("Library", self.library_uuid),
			("Usage", self.usage),
		]
		.into_iter()
		.collect()
	}

	#[cfg(target_os = "linux")]
	#[must_use]
	pub fn generate_linux_label(&self) -> String {
		format!("{} - {}", self.application, self.usage)
	}

	#[cfg(any(target_os = "macos", target_os = "ios"))]
	#[must_use]
	pub fn to_apple_account(self) -> String {
		format!("{} - {}", self.library_uuid, self.usage)
	}
}

pub trait Keyring {
	fn insert(&self, identifier: Identifier, value: SecretKeyString) -> Result<()>;
	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>>;
	fn delete(&self, identifier: Identifier) -> Result<()>;
}

/// This should be used to interact with all OS keyrings.
pub struct KeyringInterface {
	keyring: Box<dyn Keyring + Send>,
}

impl KeyringInterface {
	pub fn new() -> Result<Self> {
		#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios")))]
		return Err(crate::Error::KeyringNotSupported);

		#[cfg(target_os = "linux")]
		let keyring = Box::new(self::linux::LinuxKeyring::new()?);

		#[cfg(any(target_os = "macos", target_os = "ios"))]
		let keyring = Box::new(self::apple::AppleKeyring {});

		#[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
		Ok(Self { keyring })
	}

	pub fn insert(&self, identifier: Identifier, value: SecretKeyString) -> Result<()> {
		self.keyring.insert(identifier, value)
	}

	pub fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		self.keyring.retrieve(identifier)
	}

	pub fn delete(&self, identifier: Identifier) -> Result<()> {
		self.keyring.delete(identifier)
	}
}
