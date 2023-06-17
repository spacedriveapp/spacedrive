use std::{convert::TryFrom, ffi::OsString, fmt::Display, path::PathBuf, str::FromStr};

use mime::Mime;

use crate::{DesktopEntry, Error, ExecMode, Result};

pub enum HandlerType {
	Mime(Mime),
	Ext(String),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handler(OsString);

impl Display for Handler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0.to_string_lossy())
	}
}

impl FromStr for Handler {
	type Err = Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let handler = Self::assume_valid(s.into());
		handler.get_entry()?;
		Ok(handler)
	}
}

impl Handler {
	pub fn assume_valid(name: OsString) -> Self {
		Self(name)
	}

	pub fn get_path(&self) -> Result<PathBuf> {
		let mut path = PathBuf::from("applications");
		path.push(&self.0);
		xdg::BaseDirectories::new()?
			.find_data_file(path)
			.ok_or(Error::BadPath(self.0.to_string_lossy().to_string()))
	}

	pub fn get_entry(&self) -> Result<DesktopEntry> {
		DesktopEntry::try_from(&self.get_path()?)
	}

	pub fn launch(&self, args: &[&str]) -> Result<()> {
		self.get_entry()?.exec(ExecMode::Launch, args)
	}

	pub fn open(&self, args: &[&str]) -> Result<()> {
		self.get_entry()?.exec(ExecMode::Open, args)
	}
}
