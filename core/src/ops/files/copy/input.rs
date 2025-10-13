//! Core input types for file copy operations

use super::action::{FileCopyAction, FileCopyActionBuilder};
use super::job::CopyOptions;
use crate::domain::addressing::{SdPath, SdPathBatch};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Copy method preference for file operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum CopyMethod {
	/// Automatically select the best method based on source and destination
	Auto,
	/// Use atomic operations (rename for moves, APFS clone for copies, etc.)
	Atomic,
	/// Use streaming copy/move (works across all scenarios)
	Streaming,
}

impl Default for CopyMethod {
	fn default() -> Self {
		CopyMethod::Auto
	}
}

#[cfg(feature = "cli")]
impl std::fmt::Display for CopyMethod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			CopyMethod::Auto => write!(f, "auto"),
			CopyMethod::Atomic => write!(f, "atomic"),
			CopyMethod::Streaming => write!(f, "streaming"),
		}
	}
}

/// Core input structure for file copy operations
/// This is the canonical interface that all external APIs (CLI, REST) convert to
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileCopyInput {
	/// Source files or directories to copy (domain addressing)
	pub sources: SdPathBatch,

	/// Destination path (domain addressing)
	pub destination: SdPath,

	/// Whether to overwrite existing files
	pub overwrite: bool,

	/// Whether to verify checksums during copy
	pub verify_checksum: bool,

	/// Whether to preserve file timestamps
	pub preserve_timestamps: bool,

	/// Whether to delete source files after copying (move operation)
	pub move_files: bool,

	/// Preferred copy method to use
	pub copy_method: CopyMethod,

	/// How to handle file conflicts (set by CLI confirmation)
	pub on_conflict: Option<super::action::FileConflictResolution>,
}

impl FileCopyInput {
	/// Create a new FileCopyInput with default options from local filesystem paths
	pub fn new<D: Into<PathBuf>>(sources: Vec<PathBuf>, destination: D) -> Self {
		let paths = sources.into_iter().map(|p| SdPath::local(p)).collect();
		Self {
			sources: SdPathBatch { paths },
			destination: SdPath::local(destination.into()),
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
			move_files: false,
			copy_method: CopyMethod::Auto,
			on_conflict: None,
		}
	}

	/// Create a single file copy input
	pub fn single_file<S: Into<PathBuf>, D: Into<PathBuf>>(source: S, destination: D) -> Self {
		Self::new(vec![source.into()], destination)
	}

	/// Set overwrite option
	pub fn with_overwrite(mut self, overwrite: bool) -> Self {
		self.overwrite = overwrite;
		self
	}

	/// Set checksum verification option
	pub fn with_verification(mut self, verify: bool) -> Self {
		self.verify_checksum = verify;
		self
	}

	/// Set timestamp preservation option
	pub fn with_timestamp_preservation(mut self, preserve: bool) -> Self {
		self.preserve_timestamps = preserve;
		self
	}

	/// Set move files option
	pub fn with_move(mut self, move_files: bool) -> Self {
		self.move_files = move_files;
		self
	}

	/// Set copy method preference
	pub fn with_copy_method(mut self, copy_method: CopyMethod) -> Self {
		self.copy_method = copy_method;
		self
	}

	/// Convert to CopyOptions for the job system
	pub fn to_copy_options(&self) -> CopyOptions {
		CopyOptions {
			overwrite: self.overwrite,
			verify_checksum: self.verify_checksum,
			preserve_timestamps: self.preserve_timestamps,
			delete_after_copy: self.move_files,
			move_mode: None, // Will be determined by job system
			copy_method: self.copy_method.clone(),
		}
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.sources.paths.is_empty() {
			errors.push("At least one source file must be specified".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Get a summary string for logging/display
	pub fn summary(&self) -> String {
		let operation = if self.move_files { "Move" } else { "Copy" };
		let source_count = self.sources.paths.len();
		let source_desc = if source_count == 1 {
			"1 source".to_string()
		} else {
			format!("{} sources", source_count)
		};

		format!("{} {} to {:?}", operation, source_desc, self.destination,)
	}
}

impl Default for FileCopyInput {
	fn default() -> Self {
		Self {
			sources: SdPathBatch { paths: Vec::new() },
			destination: SdPath::local(PathBuf::new()),
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
			move_files: false,
			copy_method: CopyMethod::Auto,
			on_conflict: None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_input() {
		let input = FileCopyInput::new(vec!["/file1.txt".into(), "/file2.txt".into()], "/dest/");

		assert_eq!(input.sources.paths.len(), 2);
		assert!(!input.overwrite);
		assert!(input.preserve_timestamps);
		assert!(!input.move_files);
	}

	#[test]
	fn test_single_file() {
		let input = FileCopyInput::single_file("/source.txt", "/dest.txt");

		assert_eq!(input.sources.paths.len(), 1);
	}

	#[test]
	fn test_fluent_api() {
		let input = FileCopyInput::single_file("/source.txt", "/dest.txt")
			.with_overwrite(true)
			.with_verification(true)
			.with_timestamp_preservation(false)
			.with_move(true);

		assert!(input.overwrite);
		assert!(input.verify_checksum);
		assert!(!input.preserve_timestamps);
		assert!(input.move_files);
	}

	#[test]
	fn test_validation_empty_sources() {
		let input = FileCopyInput::default();
		let result = input.validate();

		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(errors.iter().any(|e| e.contains("At least one source")));
	}

	#[test]
	fn test_validation_success() {
		let input = FileCopyInput::new(vec!["/file.txt".into()], "/dest/");
		assert!(input.validate().is_ok());
	}
}
