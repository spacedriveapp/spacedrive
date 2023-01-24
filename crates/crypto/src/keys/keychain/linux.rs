//! This is Spacedrive's Linux keychain implementation, which makes use of the Secret Service API.
//!
//! This does strictly require DBus.

use std::collections::HashMap;

use secret_service::{EncryptionType, SecretService};

pub struct LinuxKeyring<'a> {
	// username: String,
	keyring: SecretService<'a>,
}

impl<'a> LinuxKeyring<'a> {
	pub fn list_all() {
		let ss = SecretService::new(EncryptionType::Dh).unwrap();

		for x in ss
			.get_default_collection()
			.unwrap()
			.get_all_items()
			.unwrap()
		{
			println!("{}", String::from_utf8(x.get_secret().unwrap()).unwrap())
		}
	}

	pub fn insert(text: &str) {
		let ss = SecretService::new(EncryptionType::Dh).unwrap();
		let col = ss.get_default_collection().unwrap();
		let mut h = HashMap::new();
		h.insert("test", text);

		col.create_item("test_label", h, b"sdghsdgh", false, "text/plain")
			.unwrap();
	}
}
