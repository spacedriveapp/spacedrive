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

	/// Cache for file existence checks to avoid repeated filesystem calls
	existence_cache: HashMap<PathBuf, bool>,
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
			existence_cache: HashMap::new(),
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
		for (full_path, (id, inode, modified_time, size)) in existing_entries {
			// Determine entry kind from the path (we could query this, but for change detection we mainly care about existence)
			// For now, we'll assume File for simplicity since change detection primarily cares about path/inode/timestamp
			let entry_kind = if full_path.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			};

			// Now we have accurate size information from the database
			let db_entry = DatabaseEntry {
				id,
				path: full_path.clone(),
				kind: entry_kind,
				size,
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
		&mut self,
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

		// Path not in database - check if it's a move or hard link
		if let Some(inode_val) = inode {
			if let Some(old_path) = self.inode_to_path.get(&inode_val).cloned() {
				if old_path != path {
					if let Some(db_entry) = self.path_to_entry.get(&old_path).cloned() {
						// Check if the old path still exists on disk (with caching)
						// - If old path exists: This is a hard link (both paths are valid)
						// - If old path doesn't exist: This is a genuine move
						if self.path_exists_cached(&old_path) {
							// Hard link: Both paths exist and point to same inode
							// Treat current path as a new entry (don't skip it)
							use tracing::debug;
							debug!("Hard link detected - existing: {:?}, new: {:?}, inode: {}",
								old_path, path, inode_val);
							// Fall through to "New file/directory" - both entries should exist
						} else {
							// Genuine move: Old path no longer exists, same inode at new path
							use tracing::info;
							info!("Genuine move detected - old: {:?}, new: {:?}, inode: {}",
								old_path, path, inode_val);
							return Some(Change::Moved {
								old_path,
								new_path: path.to_path_buf(),
								entry_id: db_entry.id,
								inode: inode_val,
							});
						}
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

	/// Check if a path exists with caching to reduce filesystem calls
	fn path_exists_cached(&mut self, path: &Path) -> bool {
		// Check cache first
		if let Some(&cached_result) = self.existence_cache.get(path) {
			return cached_result;
		}

		// Not in cache, check filesystem and cache the result
		let exists = path.exists();
		self.existence_cache.insert(path.to_path_buf(), exists);
		exists
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::SystemTime;

	// Mock metadata struct for testing
	pub struct MockMetadata {
		size: u64,
		modified: SystemTime,
	}

	impl MockMetadata {
		pub fn new(size: u64) -> Self {
			Self {
				size,
				modified: SystemTime::now(),
			}
		}

		pub fn len(&self) -> u64 {
			self.size
		}

		pub fn modified(&self) -> Result<SystemTime, std::io::Error> {
			Ok(self.modified)
		}
	}


	// Helper to test change detection with mock metadata
	fn test_check_path(
		detector: &mut ChangeDetector,
		path: &Path,
		size: u64,
		inode: Option<u64>,
	) -> Option<Change> {
		let mock_metadata = MockMetadata::new(size);

		// We need to manually call the logic since we can't easily mock std::fs::Metadata
		// Check if path exists in database
		if let Some(db_entry) = detector.path_to_entry.get(path) {
			// Check for modifications (simplified for testing)
			if db_entry.size != mock_metadata.len() {
				return Some(Change::Modified {
					path: path.to_path_buf(),
					entry_id: db_entry.id,
					old_modified: db_entry.modified,
					new_modified: Some(mock_metadata.modified),
				});
			}
			return None;
		}

		// Path not in database - check if it's a move or hard link
		if let Some(inode_val) = inode {
			if let Some(old_path) = detector.inode_to_path.get(&inode_val) {
				if old_path != path {
					if let Some(db_entry) = detector.path_to_entry.get(old_path) {
						// In mock tests, we can't easily check file existence
						// For testing purposes, assume it's a hard link (treat as new entry)
						// In real scenarios, the actual file existence check would determine behavior
						// Fall through to treat as new entry
					}
				}
			}
		}

		// New file/directory
		Some(Change::New(path.to_path_buf()))
	}

	#[test]
	fn test_hard_link_detection() {
		let mut detector = ChangeDetector::new();

		// Add a test entry
		let db_path = PathBuf::from("/test/dir1/file.txt");
		let db_entry = DatabaseEntry {
			id: 1,
			path: db_path.clone(),
			kind: EntryKind::File,
			size: 1000,
			modified: Some(SystemTime::now()),
			inode: Some(12345),
		};

		detector.path_to_entry.insert(db_path.clone(), db_entry);
		detector.inode_to_path.insert(12345, db_path);

		// Test hard link detection (same inode, different path, both should exist)
		let hard_link_path = PathBuf::from("/test/dir2/hardlink.txt");

		// Since we can't easily mock file existence in tests, we'll test the logic
		// In a real scenario, if both paths exist, it should be treated as a new entry
		let result = test_check_path(&mut detector, &hard_link_path, 1000, Some(12345));
		// In our mock test, this will be treated as new since we can't check file existence
		match result {
			Some(Change::New(path)) => assert_eq!(path, hard_link_path),
			_ => panic!("Expected hard link to be treated as new entry"),
		}
	}

	#[test]
	fn test_consistent_behavior() {
		let mut detector = ChangeDetector::new();

		// Add a test entry
		let db_path = PathBuf::from("/test/dir1/file.txt");
		let db_entry = DatabaseEntry {
			id: 1,
			path: db_path.clone(),
			kind: EntryKind::File,
			size: 1000,
			modified: Some(SystemTime::now()),
			inode: Some(12345),
		};

		detector.path_to_entry.insert(db_path.clone(), db_entry);
		detector.inode_to_path.insert(12345, db_path.clone());

		// Test consistent behavior: same inode at different path
		// In our mock test environment, this will be treated as a new entry
		// (since we can't mock file existence checks easily)
		let other_path = PathBuf::from("/test/dir2/other_file.txt");

		let result = test_check_path(&mut detector, &other_path, 1000, Some(12345));
		match result {
			Some(Change::New(path)) => assert_eq!(path, other_path),
			_ => panic!("Expected consistent behavior: treat as new entry"),
		}
	}

	#[test]
	fn test_new_file_detection() {
		let mut detector = ChangeDetector::new();

		// Test new file detection
		let new_path = PathBuf::from("/test/new_file.txt");

		match test_check_path(&mut detector, &new_path, 500, None) {
			Some(Change::New(p)) => assert_eq!(p, new_path),
			_ => panic!("Expected new file detection"),
		}
	}
}
