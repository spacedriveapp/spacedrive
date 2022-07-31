use std::{
	fs::File,
	io::{BufReader, Seek, SeekFrom},
	path::PathBuf,
};

use rspc::Type;
use serde::{Deserialize, Serialize};
use std::io::Write;
use uuid::Uuid;

use crate::node::ConfigMetadata;

use super::LibraryManagerError;

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Serialize, Deserialize, Clone, Type, Default)]
pub struct LibraryConfig {
	#[serde(flatten)]
	pub metadata: ConfigMetadata,
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: String,
}

impl LibraryConfig {
	/// read will read the configuration from disk and return it.
	pub(super) async fn read(file_dir: PathBuf) -> Result<LibraryConfig, LibraryManagerError> {
		let mut file = File::open(&file_dir)?;
		let base_config: ConfigMetadata = serde_json::from_reader(BufReader::new(&mut file))?;

		Self::migrate_config(base_config.version, file_dir)?;

		file.seek(SeekFrom::Start(0))?;
		Ok(serde_json::from_reader(BufReader::new(&mut file))?)
	}

	/// save will write the configuration back to disk
	pub(super) async fn save(
		file_dir: PathBuf,
		config: &LibraryConfig,
	) -> Result<(), LibraryManagerError> {
		File::create(file_dir)?.write_all(serde_json::to_string(config)?.as_bytes())?;
		Ok(())
	}

	/// migrate_config is a function used to apply breaking changes to the library config file.
	fn migrate_config(
		current_version: Option<String>,
		config_path: PathBuf,
	) -> Result<(), LibraryManagerError> {
		match current_version {
			None => Err(LibraryManagerError::Migration(format!(
				"Your Spacedrive library at '{}' is missing the `version` field",
				config_path.display()
			))),
			_ => Ok(()),
		}
	}
}

// used to return to the frontend with uuid context
#[derive(Serialize, Deserialize, Debug, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub config: LibraryConfig,
}
