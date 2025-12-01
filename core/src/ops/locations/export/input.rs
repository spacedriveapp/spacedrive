//! Input types for location export operations

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// Input for exporting a location
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationExportInput {
	/// The UUID of the location to export
	pub location_uuid: Uuid,
	/// Path where the SQL dump file will be written
	pub export_path: PathBuf,
	/// Include content identities (file hashes, dedup info)
	#[serde(default = "default_true")]
	pub include_content_identities: bool,
	/// Include media metadata (EXIF, video/audio info)
	#[serde(default = "default_true")]
	pub include_media_data: bool,
	/// Include user metadata (notes, favorites)
	#[serde(default = "default_true")]
	pub include_user_metadata: bool,
	/// Include tags and tag relationships
	#[serde(default = "default_true")]
	pub include_tags: bool,
}

fn default_true() -> bool {
	true
}
