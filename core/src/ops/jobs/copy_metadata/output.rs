//! Output types for copy metadata query.

use crate::ops::files::copy::metadata::CopyJobMetadata;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from the copy metadata query.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyMetadataOutput {
	/// The copy job metadata, if the job exists and is a copy job
	pub metadata: Option<CopyJobMetadata>,
	/// Error message if the job is not a copy job or doesn't have metadata
	pub error: Option<String>,
}

impl CopyMetadataOutput {
	/// Create a successful output with metadata
	pub fn with_metadata(metadata: CopyJobMetadata) -> Self {
		Self {
			metadata: Some(metadata),
			error: None,
		}
	}

	/// Create an error output
	pub fn with_error(error: impl Into<String>) -> Self {
		Self {
			metadata: None,
			error: Some(error.into()),
		}
	}
}
