//! # Copy Job Metadata
//!
//! Queryable metadata for file copy operations including file list information.
//! This metadata is stored in the job during the preparation phase and can be
//! queried separately from progress events.

use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Status of a file in the copy operation.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CopyFileStatus {
	/// File is waiting to be copied
	Pending,
	/// File is currently being copied
	Copying,
	/// File has been successfully copied
	Completed,
	/// File copy failed
	Failed,
	/// File was skipped (already exists or user choice)
	Skipped,
}

impl Default for CopyFileStatus {
	fn default() -> Self {
		Self::Pending
	}
}

/// Metadata for a single file or directory in the copy operation.
/// For directories, this represents the entire directory (not flattened).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyFileEntry {
	/// Source path
	pub source_path: SdPath,
	/// Destination path
	pub dest_path: SdPath,
	/// Total size in bytes (for directories, this is the recursive total)
	pub size_bytes: u64,
	/// Whether this entry is a directory
	pub is_directory: bool,
	/// Current status of this file/directory
	pub status: CopyFileStatus,
	/// Error message if status is Failed
	pub error: Option<String>,
	/// Entry UUID if source is in database (for building File objects)
	pub entry_id: Option<uuid::Uuid>,
}

/// Full metadata for a copy job, queryable via jobs.get_copy_metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyJobMetadata {
	/// Strategy metadata (name, description, flags)
	pub strategy: Option<super::routing::CopyStrategyMetadata>,
	/// List of files/directories being copied
	pub files: Vec<CopyFileEntry>,
	/// Total bytes across all files
	pub total_bytes: u64,
	/// Total file count (actual files, not directories)
	pub total_file_count: usize,
	/// Whether this is a move operation
	pub is_move_operation: bool,
	/// Full File domain objects (populated by query, not stored in job)
	#[serde(default)]
	pub file_objects: Vec<crate::domain::file::File>,
}

impl Default for CopyJobMetadata {
	fn default() -> Self {
		Self {
			strategy: None,
			files: Vec::new(),
			total_bytes: 0,
			total_file_count: 0,
			is_move_operation: false,
			file_objects: Vec::new(),
		}
	}
}

impl CopyJobMetadata {
	/// Create new metadata for a copy job
	pub fn new(is_move_operation: bool) -> Self {
		Self {
			strategy: None,
			files: Vec::new(),
			total_bytes: 0,
			total_file_count: 0,
			is_move_operation,
			file_objects: Vec::new(),
		}
	}

	/// Add a file entry to the metadata
	pub fn add_file(&mut self, entry: CopyFileEntry) {
		self.total_bytes += entry.size_bytes;
		if !entry.is_directory {
			self.total_file_count += 1;
		}
		self.files.push(entry);
	}

	/// Set the strategy metadata
	pub fn with_strategy(mut self, strategy: super::routing::CopyStrategyMetadata) -> Self {
		self.strategy = Some(strategy);
		self
	}

	/// Update status of a file by source path
	pub fn update_status(&mut self, source_path: &SdPath, status: CopyFileStatus) {
		if let Some(entry) = self
			.files
			.iter_mut()
			.find(|e| &e.source_path == source_path)
		{
			entry.status = status;
		}
	}

	/// Set error for a file by source path
	pub fn set_error(&mut self, source_path: &SdPath, error: String) {
		if let Some(entry) = self
			.files
			.iter_mut()
			.find(|e| &e.source_path == source_path)
		{
			entry.status = CopyFileStatus::Failed;
			entry.error = Some(error);
		}
	}
}
