use crate::{keys::keychain::linux::LinuxKeyring, Protected, Result};
use std::collections::HashMap;

pub mod linux;

#[derive(Clone)]
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
	// delete
	// fn lock(&self) -> Result<()>;
	// fn unlock(&self) -> Result<()>;
}

pub struct KeyringInterface {
	keyring: Box<dyn Keyring>,
}

// could just conditionally export each of the keyrings (linux/macos/windows) as `KeyringInterface` depending on target OS?
// not sure which is better
impl KeyringInterface {
	pub fn new() -> Self {
		#[cfg(target_os = "linux")]
		let keyring = Box::new(LinuxKeyring::new());

		#[cfg(not(target_os = "linux"))]
		panic!("OS is not compatible with the keyring API yet");

		Self { keyring }
	}

	pub fn insert(&self, identifier: Identifier, value: Protected<String>) -> Result<()> {
		self.keyring.insert(identifier, value)
	}

	pub fn retrieve(&self, identifier: Identifier) -> Result<Protected<Vec<u8>>> {
		self.keyring.retrieve(identifier)
	}
}

#[test]
pub fn insert_and_retrieve() {
	let k = KeyringInterface::new();

	let id = Identifier {
		application: "Spacedrive",
		library_uuid: "53605dfa-764a-4cca-b6aa-195906ba114b",
		usage: "Secret key",
	};

	k.insert(
		id.clone(),
		Protected::new("7A544B-644A55-737754-596C4E-446A724-64A3F6".to_string()),
	)
	.unwrap();

	dbg!(k.retrieve(id).unwrap().expose());
}
