use int_enum::IntEnum;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VersionManagerError {
	#[error("Invalid version")]
	InvalidVersion,
	#[error("Version file does not exist")]
	VersionFileDoesNotExist,
	#[error("Error while converting integer to enum")]
	IntConversionError,
	#[error("Malformed version file")]
	MalformedVersionFile,
	#[error(transparent)]
	IO(#[from] std::io::Error),
	#[error(transparent)]
	ParseIntError(#[from] std::num::ParseIntError),
}

///
/// An abstract system for saving a text file containing a version number.
/// The version number is an integer that can be converted to and from an enum.
/// The enum must implement the IntEnum trait.
///
pub struct VersionManager<T: IntEnum<Int = i32>> {
	version_file_path: String,
	_marker: std::marker::PhantomData<T>,
}

impl<T: IntEnum<Int = i32>> VersionManager<T> {
	pub fn new(version_file_path: &str) -> Self {
		VersionManager {
			version_file_path: version_file_path.to_string(),
			_marker: std::marker::PhantomData,
		}
	}

	pub fn get_version(&self) -> Result<T, VersionManagerError> {
		if Path::new(&self.version_file_path).exists() {
			let contents = fs::read_to_string(&self.version_file_path)?;
			let version = i32::from_str(contents.trim())?;
			T::from_int(version).map_err(|_| VersionManagerError::IntConversionError)
		} else {
			Err(VersionManagerError::VersionFileDoesNotExist)
		}
	}

	pub fn set_version(&self, version: T) -> Result<(), VersionManagerError> {
		let mut file = fs::File::create(&self.version_file_path)?;
		file.write_all(version.int_value().to_string().as_bytes())?;
		Ok(())
	}

	// pub async fn migrate<F: FnMut(T) -> Result<(), VersionManagerError>>(
	// 	&self,
	// 	current: T,
	// 	latest: T,
	// 	mut migrate_fn: F,
	// ) -> Result<(), VersionManagerError> {
	// 	for version_int in (current.int_value() + 1)..=latest.int_value() {
	// 		let version = match T::from_int(version_int) {
	// 			Ok(version) => version,
	// 			Err(_) => return Err(VersionManagerError::IntConversionError),
	// 		};
	// 		migrate_fn(version)?;
	// 	}

	// 	self.set_version(latest)?;

	// 	Ok(())
	// }
}
