//! Job output types

use crate::ops::indexing::{metrics::IndexerMetrics, state::IndexerStats};

use super::progress::Progress;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;

/// Output from a completed job
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum JobOutput {
	/// Job completed successfully with no specific output
	Success,

	/// File copy job output
	FileCopy {
		copied_count: usize,
		total_bytes: u64,
	},

	/// Indexer job output
	Indexed {
		stats: IndexerStats,
		metrics: IndexerMetrics,
	},

	/// Thumbnail generation output
	ThumbnailsGenerated {
		generated_count: usize,
		failed_count: usize,
	},

	/// Thumbnail generation output (detailed)
	ThumbnailGeneration {
		generated_count: u64,
		skipped_count: u64,
		error_count: u64,
		total_size_bytes: u64,
	},

	/// File move/rename operation output
	FileMove {
		moved_count: usize,
		failed_count: usize,
		total_bytes: u64,
	},

	/// File delete operation output
	FileDelete {
		deleted_count: usize,
		failed_count: usize,
		total_bytes: u64,
	},

	/// Duplicate detection output
	DuplicateDetection {
		duplicate_groups: usize,
		total_duplicates: usize,
		potential_savings: u64,
	},

	/// File validation output
	FileValidation {
		validated_count: usize,
		issues_found: usize,
		total_bytes_validated: u64,
	},

	/// Generic output with custom data
	#[specta(skip)]
	Custom(serde_json::Value),
}

impl JobOutput {
	/// Create a custom output
	pub fn custom<T: Serialize>(data: T) -> Self {
		Self::Custom(serde_json::to_value(data).unwrap_or(serde_json::Value::Null))
	}

	/// Get indexed output if this is an indexed job
	pub fn as_indexed(&self) -> Option<IndexedOutput> {
		match self {
			Self::Indexed { stats, metrics } => Some(IndexedOutput {
				total_files: stats.files,
				total_dirs: stats.dirs,
				total_bytes: stats.bytes,
			}),
			_ => None,
		}
	}

	/// Convert output to a progress representation (for final progress)
	pub fn as_progress(&self) -> Option<Progress> {
		match self {
			Self::Success => Some(Progress::percentage(1.0)),
			Self::FileCopy {
				copied_count,
				total_bytes,
			} => Some(Progress::generic(
				crate::infra::job::generic_progress::GenericProgress::new(
					1.0,
					"Completed",
					format!("Copied {} files", copied_count),
				)
				.with_bytes(*total_bytes, *total_bytes),
			)),
			Self::Indexed { stats, metrics } => Some(Progress::generic(
				crate::infra::job::generic_progress::GenericProgress::new(
					1.0,
					"Completed",
					format!("Indexed {} files, {} directories", stats.files, stats.dirs),
				)
				.with_bytes(stats.bytes, stats.bytes),
			)),
			Self::ThumbnailGeneration {
				generated_count, ..
			} => Some(Progress::generic(
				crate::infra::job::generic_progress::GenericProgress::new(
					1.0,
					"Completed",
					format!("Generated {} thumbnails", generated_count),
				),
			)),
			Self::FileMove { moved_count, .. } => Some(Progress::percentage(1.0)),
			Self::FileDelete { deleted_count, .. } => Some(Progress::percentage(1.0)),
			Self::DuplicateDetection {
				duplicate_groups, ..
			} => Some(Progress::percentage(1.0)),
			Self::FileValidation {
				validated_count, ..
			} => Some(Progress::percentage(1.0)),
			_ => Some(Progress::percentage(1.0)),
		}
	}
}

/// Typed output for indexed jobs
#[derive(Debug, Clone)]
pub struct IndexedOutput {
	pub total_files: u64,
	pub total_dirs: u64,
	pub total_bytes: u64,
}

impl Default for JobOutput {
	fn default() -> Self {
		Self::Success
	}
}

impl fmt::Display for JobOutput {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => write!(f, "Success"),
			Self::FileCopy {
				copied_count,
				total_bytes,
			} => {
				write!(f, "Copied {} files ({} bytes)", copied_count, total_bytes)
			}
			Self::Indexed { stats, metrics } => {
				write!(
					f,
					"Indexed {} files, {} directories ({} bytes)",
					stats.files, stats.dirs, stats.bytes
				)
			}
			Self::ThumbnailsGenerated {
				generated_count,
				failed_count,
			} => {
				write!(
					f,
					"Generated {} thumbnails ({} failed)",
					generated_count, failed_count
				)
			}
			Self::ThumbnailGeneration {
				generated_count,
				skipped_count,
				error_count,
				total_size_bytes,
			} => {
				write!(
					f,
					"Generated {} thumbnails ({} skipped, {} errors, {} bytes)",
					generated_count, skipped_count, error_count, total_size_bytes
				)
			}
			Self::FileMove {
				moved_count,
				failed_count,
				total_bytes,
			} => {
				write!(
					f,
					"Moved {} files ({} failed, {} bytes)",
					moved_count, failed_count, total_bytes
				)
			}
			Self::FileDelete {
				deleted_count,
				failed_count,
				total_bytes,
			} => {
				write!(
					f,
					"Deleted {} files ({} failed, {} bytes)",
					deleted_count, failed_count, total_bytes
				)
			}
			Self::DuplicateDetection {
				duplicate_groups,
				total_duplicates,
				potential_savings,
			} => {
				write!(
					f,
					"Found {} duplicate groups ({} duplicates, {} bytes savings)",
					duplicate_groups, total_duplicates, potential_savings
				)
			}
			Self::FileValidation {
				validated_count,
				issues_found,
				total_bytes_validated,
			} => {
				write!(
					f,
					"Validated {} files ({} issues, {} bytes)",
					validated_count, issues_found, total_bytes_validated
				)
			}
			Self::Custom(_) => write!(f, "Custom output"),
		}
	}
}
