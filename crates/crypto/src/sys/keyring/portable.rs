use super::Keyring;
use crate::{Error, Result};

pub struct PortableKeyring;

impl Keyring for PortableKeyring {
	fn new() -> Result<Self>
	where
		Self: Sized,
	{
		Ok(Self {})
	}

	fn insert(
		&self,
		_identifier: super::Identifier<'_>,
		_value: crate::Protected<Vec<u8>>,
	) -> Result<()> {
		Err(Error::KeyringNotSupported)
	}

	fn delete(&self, _identifier: super::Identifier<'_>) -> Result<()> {
		Err(Error::KeyringNotSupported)
	}

	fn retrieve(&self, _identifier: super::Identifier<'_>) -> Result<crate::Protected<Vec<u8>>> {
		Err(Error::KeyringNotSupported)
	}
}
