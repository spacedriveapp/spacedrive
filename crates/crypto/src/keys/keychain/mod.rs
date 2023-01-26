use crate::{keys::keychain::linux::LinuxKeyring, Protected, Result};
use std::collections::HashMap;

pub mod linux;

#[derive(Clone, Copy)]
pub struct Identifier<'a> {
	pub application: &'a str,
	pub library_uuid: &'a str,
	pub usage: &'a str,
}

impl<'a> Identifier<'a> {
	pub fn to_hashmap(self) -> HashMap<&'a str, &'a str> {
		let mut map = HashMap::new();
		map.insert("Application", self.application);
		map.insert("Library", self.library_uuid);
		map.insert("Usage", self.usage);
		map
	}
}

pub trait Keyring {
	fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()>; // updates item if already present
	fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>>;
	fn delete(&self, identifier: Identifier) -> Result<()>;
}

pub struct KeyringInterface {
	keyring: Box<dyn Keyring>,
}

impl KeyringInterface {
	pub fn new() -> Result<Self> {
		#[cfg(target_os = "linux")]
		let keyring = Box::new(LinuxKeyring::new()?);

		#[cfg(not(target_os = "linux"))]
		panic!("OS is not compatible with the keyring API yet");

		Ok(Self { keyring })
	}

	pub fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		self.keyring.insert(identifier, value)
	}

	pub fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		self.keyring.retrieve(identifier)
	}

	pub fn delete(&self, identifier: Identifier) -> Result<()> {
		self.keyring.delete(identifier)
	}
}

// not really a test, just an easy way to run the code
#[test]
pub fn insert_and_retrieve() {
	let k = KeyringInterface::new().unwrap();

	let id = Identifier {
		application: "Spacedrive",
		library_uuid: "53605dfa-764a-4cca-b6aa-195906ba114b",
		usage: "Secret key",
	};

	k.insert(
		id,
		Protected::new("7A544B-644A55-737754-596C4E-446A724-64A3F6".to_string()),
	)
	.unwrap();

	k.retrieve(id).unwrap();
}
