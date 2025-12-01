//! Input types for location import operations

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Input for importing a location from SQL dump
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationImportInput {
	/// Path to the SQL dump file to import
	pub import_path: PathBuf,
	/// Optional new name for the imported location (overrides name in dump)
	pub new_name: Option<String>,
	/// Whether to skip entries that already exist (by UUID)
	#[serde(default)]
	pub skip_existing: bool,
}
