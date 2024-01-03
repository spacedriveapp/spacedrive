use std::ops::Deref;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Debug, Serialize, Clone, Type)]
pub struct LibraryName(String);

#[derive(Debug, Error)]
pub enum LibraryNameError {
	#[error("empty")]
	Empty,
	#[error("needs-trim")]
	NeedsTrim,
}

impl LibraryName {
	pub fn new(name: impl Into<String>) -> Result<Self, LibraryNameError> {
		let name = name.into();

		if name.is_empty() {
			return Err(LibraryNameError::Empty);
		}

		if name.starts_with(' ') || name.ends_with(' ') {
			return Err(LibraryNameError::NeedsTrim);
		}

		Ok(Self(name))
	}
}

impl<'de> Deserialize<'de> for LibraryName {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		LibraryName::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
	}
}

impl AsRef<str> for LibraryName {
	fn as_ref(&self) -> &str {
		&self.0
	}
}

impl Deref for LibraryName {
	type Target = String;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<LibraryName> for String {
	fn from(name: LibraryName) -> Self {
		name.0
	}
}
