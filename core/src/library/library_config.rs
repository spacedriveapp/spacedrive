use std::{fs::File, io::BufReader, path::PathBuf};

use serde::{Deserialize, Serialize};
use std::io::Write;
use ts_rs::TS;

use super::LibraryManagerError;

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Serialize, Deserialize, Clone, TS, Default)]
#[ts(export)]
pub struct LibraryConfig {
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: String,
}

impl LibraryConfig {
	/// read will read the configuration from disk and return it.
	pub(super) async fn read(file_dir: PathBuf) -> Result<LibraryConfig, LibraryManagerError> {
		let reader = BufReader::new(File::open(file_dir).map_err(LibraryManagerError::IOError)?);
		Ok(serde_json::from_reader(reader).map_err(LibraryManagerError::JsonError)?)
	}

	/// save will write the configuration back to disk
	pub(super) async fn save(
		file_dir: PathBuf,
		config: &LibraryConfig,
	) -> Result<(), LibraryManagerError> {
		File::create(file_dir)
			.map_err(LibraryManagerError::IOError)?
			.write_all(
				serde_json::to_string(config)
					.map_err(LibraryManagerError::JsonError)?
					.as_bytes(),
			)
			.map_err(LibraryManagerError::IOError)?;
		Ok(())
	}
}
