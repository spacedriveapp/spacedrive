//! Input types for library export operations

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Input for exporting a library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryExportInput {
	pub library_id: Uuid,
	pub export_path: PathBuf,
	pub include_thumbnails: bool,
	pub include_previews: bool,
}
