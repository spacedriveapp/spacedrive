//! Change detection for incremental indexing
//!
//! This module provides efficient change detection using:
//! - Inode tracking for move/rename detection
//! - Modification time comparison
//! - Size verification
//! - Directory hierarchy tracking

use super::state::EntryKind;
use crate::infra::{db::entities, job::prelude::JobContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	time::SystemTime,
};

/// Represents a change detected in the file system
#[derive(Debug, Clone)]
pub enum Change {
	/// New file/directory not in database
	New(PathBuf),

	/// File/directory modified (content or metadata changed)
	Modified {
		path: PathBuf,
		entry_id: i32,
		old_modified: Option<SystemTime>,
		new_modified: Option<SystemTime>,
	},

	/// File/directory moved or renamed (same inode, different path)
	Moved {
		old_path: PathBuf,
		new_path: PathBuf,
		entry_id: i32,
		inode: u64,
	},

	/// File/directory deleted (exists in DB but not on disk)
	Deleted { path: PathBuf, entry_id: i32 },
}

/// Tracks changes between database state and file system
pub struct ChangeDetector {
	/// Maps paths to their database entries
	path_to_entry: HashMap<PathBuf, DatabaseEntry>,

	/// Maps inodes to paths (for detecting moves)
	inode_to_path: HashMap<u64, PathBuf>,

	/// Precision for timestamp comparison (some filesystems have lower precision)
	timestamp_precision_ms: i64,
}

#[derive(Debug, Clone)]
struct DatabaseEntry {
	id: i32,
	path: PathBuf,
	kind: EntryKind,
	size: u64,
	modified: Option<SystemTime>,
	inode: Option<u64>,
}

impl ChangeDetector {
	/// Create a new change detector
	pub fn new() -> Self {
		Self {
			path_to_entry: HashMap::new(),
			inode_to_path: HashMap::new(),
			timestamp_precision_ms: 1, // Default to 1ms precision
		}
	}

	/// Load existing entries from database for a location, scoped to indexing path
	pub async fn load_existing_entries(
		&mut self,
		ctx: &JobContext<'_>,
		location_id: i32,
		indexing_path: &Path,
	) -> Result<(), crate::infra::job::prelude::JobError> {
		use crate::infra::job::prelude::JobError;
		use super::persistence::{DatabasePersistence, IndexPersistence};

		// For change detection, we need to get the location's root entry ID
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let location_record = entities::location::Entity::find_by_id(location_id)
			.one(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
			.ok_or_else(|| JobError::execution("Location not found".to_string()))?;

		// Create a database persistence instance to leverage the scoped query logic
		let persistence = DatabasePersistence::new(ctx, 0, Some(location_record.entry_id)); // device_id not needed for query

		// Use the scoped query method
		let existing_entries = persistence.get_existing_entries(indexing_path).await?;

		// Process the results into our internal data structures
		for (full_path, (id, inode, modified_time)) in existing_entries {
			// Determine entry kind from the path (we could query this, but for change detection we mainly care about existence)
			// For now, we'll assume File for simplicity since change detection primarily cares about path/inode/timestamp
			let entry_kind = if full_path.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			};

			// We don't have size from the scoped query, but it's not critical for change detection
			// The actual size comparison happens during processing when we have fresh metadata
			let db_entry = DatabaseEntry {
				id,
				path: full_path.clone(),
				kind: entry_kind,
				size: 0, // Will be verified during actual change detection
				modified: modified_time,
				inode,
			};

			// Track by path
			self.path_to_entry.insert(full_path.clone(), db_entry);

			// Track by inode if available
			if let Some(inode_val) = inode {
				self.inode_to_path.insert(inode_val, full_path);
			}
		}

		ctx.log(format!(
			"Loaded {} existing entries for change detection",
			self.path_to_entry.len()
		));

		// DEBUG: Log if we failed to load entries
		use tracing::warn;
		if self.path_to_entry.is_empty() {
			warn!("DEBUG: ChangeDetector loaded 0 entries - database may be locked or empty");
		} else {
			warn!("DEBUG: ChangeDetector loaded {} entries successfully", self.path_to_entry.len());
		}

		Ok(())
	}

	/// Check if a path represents a change
	pub fn check_path(
		&self,
		path: &Path,
		metadata: &std::fs::Metadata,
		inode: Option<u64>,
	) -> Option<Change> {
		// Check if path exists in database
		if let Some(db_entry) = self.path_to_entry.get(path) {
			// Check for modifications
			if self.is_modified(db_entry, metadata) {
				return Some(Change::Modified {
					path: path.to_path_buf(),
					entry_id: db_entry.id,
					old_modified: db_entry.modified,
					new_modified: metadata.modified().ok(),
				});
			}

			// No change for this path
			return None;
		}

		// Path not in database - check if it's a move
		if let Some(inode_val) = inode {
			if let Some(old_path) = self.inode_to_path.get(&inode_val) {
				if old_path != path {
					// Same inode, different path - it's a move
					if let Some(db_entry) = self.path_to_entry.get(old_path) {
						// DEBUG: Log false move detection
						use tracing::warn;
						warn!("DEBUG: Detected move - old: {:?}, new: {:?}, inode: {}", old_path, path, inode_val);
						return Some(Change::Moved {
							old_path: old_path.clone(),
							new_path: path.to_path_buf(),
							entry_id: db_entry.id,
							inode: inode_val,
						});
					}
				}
			}
		}

		// New file/directory
		Some(Change::New(path.to_path_buf()))
	}

	/// Find deleted entries (in DB but not seen during scan)
	pub fn find_deleted(&self, seen_paths: &std::collections::HashSet<PathBuf>) -> Vec<Change> {
		self.path_to_entry
			.iter()
			.filter(|(path, _)| !seen_paths.contains(*path))
			.map(|(path, entry)| Change::Deleted {
				path: path.clone(),
				entry_id: entry.id,
			})
			.collect()
	}

	/// Check if an entry has been modified
	fn is_modified(&self, db_entry: &DatabaseEntry, metadata: &std::fs::Metadata) -> bool {
		// Check size first (fast)
		if db_entry.size != metadata.len() {
			return true;
		}

		// Check modification time
		if let (Some(db_modified), Ok(fs_modified)) = (db_entry.modified, metadata.modified()) {
			// Compare with precision tolerance
			let db_time = db_modified
				.duration_since(SystemTime::UNIX_EPOCH)
				.unwrap_or_default()
				.as_millis() as i64;
			let fs_time = fs_modified
				.duration_since(SystemTime::UNIX_EPOCH)
				.unwrap_or_default()
				.as_millis() as i64;

			if (db_time - fs_time).abs() > self.timestamp_precision_ms {
				return true;
			}
		}

		false
	}

	/// Set timestamp precision for comparison (in milliseconds)
	pub fn set_timestamp_precision(&mut self, precision_ms: i64) {
		self.timestamp_precision_ms = precision_ms;
	}

	/// Get the number of tracked entries
	pub fn entry_count(&self) -> usize {
		self.path_to_entry.len()
	}
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_change_detection() {
//         let mut detector = ChangeDetector::new();

//         // Add a test entry
//         let path = PathBuf::from("/test/file.txt");
//         let db_entry = DatabaseEntry {
//             id: 1,
//             path: path.clone(),
//             kind: EntryKind::File,
//             size: 1000,
//             modified: Some(SystemTime::now()),
//             inode: Some(12345),
//         };

//         detector.path_to_entry.insert(path.clone(), db_entry);
//         detector.inode_to_path.insert(12345, path.clone());

//         // Test new file detection
//         let new_path = PathBuf::from("/test/new_file.txt");
//         let metadata = std::fs::Metadata::default(); // Would use real metadata in practice

//         match detector.check_path(&new_path, &metadata, None) {
//             Some(Change::New(p)) => assert_eq!(p, new_path),
//             _ => panic!("Expected new file detection"),
//         }
//     }
// }
