use super::{Identifier, Keyring};
use crate::{Error, Protected, Result};

pub struct PortableKeyring;

impl Keyring for PortableKeyring {
	fn new() -> Result<Self>
	where
		Self: Sized,
	{
		Ok(Self {})
	}

	fn insert(&self, _identifier: Identifier<'_>, _value: Protected<Vec<u8>>) -> Result<()> {
		Err(Error::KeyringNotSupported)
	}

	fn delete(&self, _identifier: Identifier<'_>) -> Result<()> {
		Err(Error::KeyringNotSupported)
	}

	fn retrieve(&self, _identifier: Identifier<'_>) -> Result<Protected<Vec<u8>>> {
		Err(Error::KeyringNotSupported)
	}
}
