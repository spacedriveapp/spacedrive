use crate::sd_path::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;
/// A single file copy operation, whether successful or failed
#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct CopyOperation {
	/// Source path of the file
	pub source: SdPath,
	/// Target path where the file was/should be copied to
	pub target: SdPath,
	/// Size of the file in bytes
	pub size: u64,
	/// Whether this was a cross-device copy
	pub cross_device: bool,
	/// Time taken for the copy in milliseconds (None if failed)
	pub duration_ms: Option<u64>,
	/// Error message if the copy failed (None if successful)
	pub error: Option<String>,
}

/// Statistics and results from a copy operation
#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct CopyStats {
	/// Total number of files to copy
	pub total_files: u32,
	/// Total bytes to copy
	pub total_bytes: u64,
	/// Number of files successfully copied
	pub completed_files: u32,
	/// Number of bytes successfully copied
	pub completed_bytes: u64,
	/// Average speed in bytes per second
	pub speed: u64,
	/// List of successful copy operations
	pub successful: Vec<CopyOperation>,
	/// List of failed copy operations
	pub failed: Vec<CopyOperation>,
}

impl Default for CopyStats {
	fn default() -> Self {
		Self {
			total_files: 0,
			total_bytes: 0,
			completed_files: 0,
			completed_bytes: 0,
			speed: 0,
			successful: Vec::new(),
			failed: Vec::new(),
		}
	}
}

impl CopyStats {
	pub fn files_skipped(&self) -> u32 {
		self.total_files - (self.completed_files + self.failed.len() as u32)
	}

	pub fn successful_operations(&self) -> impl Iterator<Item = &CopyOperation> {
		self.successful.iter()
	}

	pub fn failed_operations(&self) -> impl Iterator<Item = &CopyOperation> {
		self.failed.iter()
	}
}
