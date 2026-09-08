//! Change detector for batch indexing scans.
//!
//! The `ChangeDetector` compares database state against filesystem state
//! during indexer job scans. It identifies:
//! - New files/directories (not in database)
//! - Modified entries (size or mtime changed)
//! - Moved entries (same inode, different path)
//! - Deleted entries (in database but not on disk)

use super::types::Change;
use crate::infra::db::entities;
use crate::infra::job::prelude::{JobContext, JobError, JobResult};
use crate::ops::indexing::state::EntryKind;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	time::SystemTime,
};

/// Tracks changes between database state and filesystem during batch scans.
///
/// Used by the indexer job to efficiently detect what needs to be created,
/// updated, moved, or deleted. Loads existing entries from the database,
/// then compares against filesystem walks.
pub struct ChangeDetector {
	/// Maps paths to their database entries
	path_to_entry: HashMap<PathBuf, DatabaseEntry>,

	/// Maps inodes to paths (for detecting moves)
	inode_to_path: HashMap<u64, PathBuf>,

	/// Precision for timestamp comparison (some filesystems have lower precision)
	timestamp_precision_ms: i64,

	/// Cache for file existence checks to avoid repeated filesystem calls
	existence_cache: HashMap<PathBuf, bool>,

	/// Entry IDs that have been processed (moved, modified, etc.) and should not be deleted
	processed_entry_ids: std::collections::HashSet<i32>,
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
			processed_entry_ids: std::collections::HashSet::new(),
		}
	}

	/// Load existing entries from database for a location, scoped to indexing path
	pub async fn load_existing_entries(
		&mut self,
		ctx: &JobContext<'_>,
		location_id: i32,
		indexing_path: &Path,
	) -> Result<(), crate::infra::job::prelude::JobError> {
		use crate::infra::db::entities;
		use crate::infra::job::prelude::JobError;
		use sea_orm::EntityTrait;

		let location_record = entities::location::Entity::find_by_id(location_id)
			.one(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
			.ok_or_else(|| JobError::execution("Location not found".to_string()))?;

		if location_record.volume_id.is_none() {
			return Err(JobError::execution(
				"Location has no volume_id - volume must be detected before change detection",
			));
		}

		let existing_entries =
			existing_entries(ctx, location_record.entry_id, indexing_path).await?;

		// Process the results into our internal data structures
		for (full_path, (id, inode, modified_time, size)) in existing_entries {
			let entry_kind = if full_path.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			};

			let db_entry = DatabaseEntry {
				id,
				path: full_path.clone(),
				kind: entry_kind,
				size,
				modified: modified_time,
				inode,
			};

			self.path_to_entry.insert(full_path.clone(), db_entry);

			if let Some(inode_val) = inode {
				self.inode_to_path.insert(inode_val, full_path);
			}
		}

		ctx.log(format!(
			"Loaded {} existing entries for change detection",
			self.path_to_entry.len()
		));

		use tracing::warn;
		if self.path_to_entry.is_empty() {
			warn!("ChangeDetector loaded 0 entries - database may be locked or empty");
		} else {
			warn!(
				"ChangeDetector loaded {} entries successfully",
				self.path_to_entry.len()
			);
		}

		Ok(())
	}

	/// Check if a path represents a change.
	///
	/// Returns Some(Change) if the path is new, modified, or moved.
	/// Returns None if the path exists in database with same metadata.
	pub fn check_path(
		&mut self,
		path: &Path,
		metadata: &std::fs::Metadata,
		inode: Option<u64>,
	) -> Option<Change> {
		// Check if path exists in database
		if let Some(db_entry) = self.path_to_entry.get(path) {
			// Mark as processed so it won't be considered for deletion
			self.processed_entry_ids.insert(db_entry.id);

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
						// Mark as processed so it won't be considered for deletion
						self.processed_entry_ids.insert(db_entry.id);

						// Check if the old path still exists on disk (with caching)
						if self.path_exists_cached(&old_path) {
							// Hard link: Both paths exist and point to same inode
							use tracing::debug;
							debug!(
								"Hard link detected - existing: {:?}, new: {:?}, inode: {}",
								old_path, path, inode_val
							);
							// Fall through to "New" - both entries should exist
						} else {
							// Genuine move: Old path no longer exists
							use tracing::info;
							info!(
								"Move detected - old: {:?}, new: {:?}, inode: {}",
								old_path, path, inode_val
							);
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

	/// Find deleted entries (in DB but not seen during scan).
	pub fn find_deleted(&self, seen_paths: &std::collections::HashSet<PathBuf>) -> Vec<Change> {
		self.path_to_entry
			.iter()
			.filter(|(path, entry)| {
				// Exclude if path was seen during scan
				if seen_paths.contains(*path) {
					return false;
				}
				// Exclude if entry was already processed (moved, modified, etc.)
				if self.processed_entry_ids.contains(&entry.id) {
					return false;
				}
				true
			})
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
		if let Some(&cached_result) = self.existence_cache.get(path) {
			return cached_result;
		}

		let exists = path.exists();
		self.existence_cache.insert(path.to_path_buf(), exists);
		exists
	}
}

impl Default for ChangeDetector {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_file_detection() {
		let mut detector = ChangeDetector::new();
		let new_path = PathBuf::from("/test/new_file.txt");

		// Create a temporary file for testing
		let temp_dir = tempfile::tempdir().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, "test content").unwrap();
		let metadata = std::fs::metadata(&test_file).unwrap();

		let result = detector.check_path(&new_path, &metadata, None);
		match result {
			Some(Change::New(path)) => assert_eq!(path, new_path),
			_ => panic!("Expected new file detection"),
		}
	}

	#[test]
	fn test_entry_count() {
		let detector = ChangeDetector::new();
		assert_eq!(detector.entry_count(), 0);
	}
}

/// Every entry the database already holds under `indexing_path`, keyed by path.
///
/// What change detection compares the filesystem against: an entry that is here
/// and not on disk was deleted, and one whose inode turns up elsewhere moved.
async fn existing_entries(
	ctx: &JobContext<'_>,
	location_root_entry_id: Option<i32>,
	indexing_path: &Path,
) -> JobResult<HashMap<PathBuf, (i32, Option<u64>, Option<SystemTime>, u64)>> {
	use crate::infra::db::entities::{directory_paths, entry_closure};
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	// A location whose root has no entry row has nothing indexed to compare
	// against, so everything the walk finds is new.
	if location_root_entry_id.is_none() {
		return Ok(HashMap::new());
	}

	let indexing_path_str = indexing_path.to_string_lossy().to_string();
	let indexing_path_entry_id = match directory_paths::Entity::find()
		.filter(directory_paths::Column::Path.eq(&indexing_path_str))
		.one(ctx.library_db())
		.await
	{
		Ok(Some(dir_record)) => dir_record.entry_id,
		Ok(None) => {
			// Path not found in database - this is either a new directory or a moved one.
			// Return empty to let inode-based move detection handle it, rather than
			// incorrectly loading entries from the entire location root.
			ctx.log(format!(
				"Indexing path not found in database: {}, treating as new (move detection via inode)",
				indexing_path_str
			));
			return Ok(HashMap::new());
		}
		Err(e) => {
			return Err(JobError::execution(format!(
				"Failed to query directory_paths: {}",
				e
			)));
		}
	};

	let descendant_ids = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(indexing_path_entry_id))
		.all(ctx.library_db())
		.await
		.map_err(|e| JobError::execution(format!("Failed to query closure table: {}", e)))?
		.into_iter()
		.map(|ec| ec.descendant_id)
		.collect::<Vec<i32>>();

	let mut all_entry_ids = vec![indexing_path_entry_id];
	all_entry_ids.extend(descendant_ids);

	let mut existing_entries: Vec<entities::entry::Model> = Vec::new();
	let chunk_size: usize = 900;
	for chunk in all_entry_ids.chunks(chunk_size) {
		let mut batch = entities::entry::Entity::find()
			.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
			.all(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to query existing entries: {}", e)))?;
		existing_entries.append(&mut batch);
	}

	let mut result = HashMap::new();

	ctx.log(format!(
		"Loading {} existing entries",
		existing_entries.len()
	));

	for entry in existing_entries {
		let full_path =
			crate::ops::indexing::PathResolver::get_full_path(ctx.library_db(), entry.id)
				.await
				.unwrap_or_else(|_| PathBuf::from(&entry.name));

		let modified_time = entry
			.modified_at
			.timestamp()
			.try_into()
			.ok()
			.and_then(|secs: u64| {
				std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
			});

		result.insert(
			full_path,
			(
				entry.id,
				entry.inode.map(|i| i as u64),
				modified_time,
				entry.size as u64,
			),
		);
	}

	Ok(result)
}
