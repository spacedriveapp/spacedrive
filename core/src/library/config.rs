use std::{marker::PhantomData, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{migrations, util::migrator::FileMigrator};

use super::LibraryManagerError;

const MIGRATOR: FileMigrator<LibraryConfig> = FileMigrator {
	current_version: migrations::LIBRARY_VERSION,
	migration_fn: migrations::migration_library,
	phantom: PhantomData,
};

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Serialize, Deserialize, Clone, Type, Default)]
pub struct LibraryConfig {
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: String,
	// /// is_encrypted is a flag that is set to true if the library is encrypted.
	// #[serde(default)]
	// pub is_encrypted: bool,
}

impl LibraryConfig {
	/// read will read the configuration from disk and return it.
	pub(super) fn read(file_dir: PathBuf) -> Result<LibraryConfig, LibraryManagerError> {
		MIGRATOR.load(&file_dir).map_err(Into::into)
	}

	/// save will write the configuration back to disk
	pub(super) fn save(
		file_dir: PathBuf,
		config: &LibraryConfig,
	) -> Result<(), LibraryManagerError> {
		MIGRATOR.save(&file_dir, config.clone())?;
		Ok(())
	}
}

// used to return to the frontend with uuid context
#[derive(Serialize, Deserialize, Debug, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub config: LibraryConfig,
}
