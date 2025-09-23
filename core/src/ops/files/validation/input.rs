//! File validation input for external API

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Input for file validation operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileValidationInput {
	/// Paths to validate
	pub paths: Vec<PathBuf>,
	/// Whether to verify file checksums
	pub verify_checksums: bool,
	/// Whether to perform deep scanning
	pub deep_scan: bool,
}
