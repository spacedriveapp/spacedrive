use crate::{
	domain::addressing::SdPath,
	infra::db::entities::{entry, sync_conduit, sync_generation},
};
use anyhow::Result;
use sea_orm::{prelude::*, DatabaseConnection, QueryOrder, QuerySelect};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

/// Calculates sync operations from index queries
pub struct SyncResolver {
	db: Arc<DatabaseConnection>,
}

/// Entry with its materialized path relative to sync root
#[derive(Debug, Clone)]
pub struct EntryWithPath {
	pub entry: entry::Model,
	pub relative_path: PathBuf,
	pub full_path: PathBuf,
}

impl EntryWithPath {
	/// Convert to SdPath for job operations
	pub fn to_sdpath(&self, device_slug: String) -> SdPath {
		SdPath::physical(device_slug, self.full_path.clone())
	}
}

/// Operations for a single sync direction
#[derive(Debug, Default, Clone)]
pub struct DirectionalOps {
	pub to_copy: Vec<EntryWithPath>,
	pub to_delete: Vec<EntryWithPath>,
}

/// Complete sync operations (supports bidirectional)
#[derive(Debug, Default)]
pub struct SyncOperations {
	/// Source → target operations
	pub source_to_target: DirectionalOps,

	/// Target → source operations (only for bidirectional mode)
	pub target_to_source: Option<DirectionalOps>,

	/// Conflicts that need resolution
	pub conflicts: Vec<SyncConflict>,
}

#[derive(Debug)]
pub struct SyncConflict {
	pub relative_path: PathBuf,
	pub source_entry: entry::Model,
	pub target_entry: entry::Model,
	pub conflict_type: ConflictType,
}

#[derive(Debug, Clone, Copy)]
pub enum ConflictType {
	BothModified,
	DeletedVsModified,
	TypeMismatch,
}

impl SyncResolver {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Calculate sync operations for a conduit
	pub async fn calculate_operations(
		&self,
		conduit: &sync_conduit::Model,
	) -> Result<SyncOperations> {
		// Get source and target root entries
		let source_root = entry::Entity::find_by_id(conduit.source_entry_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Source entry not found"))?;

		let target_root = entry::Entity::find_by_id(conduit.target_entry_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Target entry not found"))?;

		// Load all entries under each root
		let source_entries = self
			.get_entries_recursive(conduit.source_entry_id, &source_root)
			.await?;
		let target_entries = self
			.get_entries_recursive(conduit.target_entry_id, &target_root)
			.await?;

		// Build path maps
		let source_map = self.build_path_map(&source_entries);
		let target_map = self.build_path_map(&target_entries);

		let mode = sync_conduit::SyncMode::from_str(&conduit.sync_mode)
			.ok_or_else(|| anyhow::anyhow!("Invalid sync mode"))?;

		match mode {
			sync_conduit::SyncMode::Mirror => Ok(self.resolve_mirror(&source_map, &target_map)),
			sync_conduit::SyncMode::Bidirectional => {
				self.resolve_bidirectional(&source_map, &target_map, conduit)
					.await
			}
			sync_conduit::SyncMode::Selective => Ok(self.resolve_mirror(&source_map, &target_map)),
		}
	}

	/// Get all entries under a directory recursively
	/// This is a simplified implementation - in a real implementation,
	/// we'd need to reconstruct full paths by walking parent relationships
	async fn get_entries_recursive(
		&self,
		root_id: i32,
		root_entry: &entry::Model,
	) -> Result<Vec<EntryWithPath>> {
		let mut results = Vec::new();

		// Simple recursive query - find all entries with this root as ancestor
		// This is a simplified approach. In production, we'd need proper path reconstruction
		let entries = self.find_children_recursive(root_id).await?;

		// For MVP, we'll use a simple relative path construction
		// In production, this should walk parent links to build full paths
		for entry in entries {
			let relative_path = PathBuf::from(&entry.name);
			let full_path = PathBuf::from(&entry.name); // Simplified

			results.push(EntryWithPath {
				entry,
				relative_path,
				full_path,
			});
		}

		Ok(results)
	}

	/// Find all children of an entry recursively using parent_id relationship
	async fn find_children_recursive(&self, parent_id: i32) -> Result<Vec<entry::Model>> {
		let mut all_children = Vec::new();
		let mut to_process = vec![parent_id];

		while let Some(current_parent) = to_process.pop() {
			let children = entry::Entity::find()
				.filter(entry::Column::ParentId.eq(current_parent))
				.all(&*self.db)
				.await?;

			for child in children {
				to_process.push(child.id);
				all_children.push(child);
			}
		}

		Ok(all_children)
	}

	/// Build map of relative path -> entry with path
	fn build_path_map(&self, entries: &[EntryWithPath]) -> HashMap<PathBuf, EntryWithPath> {
		entries
			.iter()
			.map(|e| (e.relative_path.clone(), e.clone()))
			.collect()
	}

	/// Resolve mirror mode: source -> target (one-way)
	fn resolve_mirror(
		&self,
		source_map: &HashMap<PathBuf, EntryWithPath>,
		target_map: &HashMap<PathBuf, EntryWithPath>,
	) -> SyncOperations {
		let mut operations = SyncOperations::default();

		// Files in source but not target, or files that differ -> copy
		for (path, source_entry_with_path) in source_map {
			if let Some(target_entry_with_path) = target_map.get(path) {
				// File exists in both - check if content differs
				if self
					.content_differs(&source_entry_with_path.entry, &target_entry_with_path.entry)
				{
					operations
						.source_to_target
						.to_copy
						.push(source_entry_with_path.clone());
				}
			} else {
				// File only in source - copy it
				operations
					.source_to_target
					.to_copy
					.push(source_entry_with_path.clone());
			}
		}

		// Files in target but not source -> delete
		for (path, target_entry_with_path) in target_map {
			if !source_map.contains_key(path) {
				operations
					.source_to_target
					.to_delete
					.push(target_entry_with_path.clone());
			}
		}

		operations
	}

	/// Resolve bidirectional mode with conflict detection
	async fn resolve_bidirectional(
		&self,
		source_map: &HashMap<PathBuf, EntryWithPath>,
		target_map: &HashMap<PathBuf, EntryWithPath>,
		conduit: &sync_conduit::Model,
	) -> Result<SyncOperations> {
		let mut operations = SyncOperations::default();
		operations.target_to_source = Some(DirectionalOps::default());

		// Get last sync generation for change detection
		let last_gen = self.get_last_completed_generation(conduit.id).await?;

		// Detect changes since last sync
		let source_changes = self.detect_changes(source_map, last_gen.as_ref());
		let target_changes = self.detect_changes(target_map, last_gen.as_ref());

		// Check each file in both locations
		let all_paths: HashSet<_> = source_map
			.keys()
			.chain(target_map.keys())
			.cloned()
			.collect();

		for path in all_paths {
			let in_source = source_map.get(&path);
			let in_target = target_map.get(&path);

			match (in_source, in_target) {
				(Some(source_entry_with_path), Some(target_entry_with_path)) => {
					// File in both locations
					let source_changed = source_changes.contains(&path);
					let target_changed = target_changes.contains(&path);

					if source_changed && target_changed {
						// Conflict: both modified
						operations.conflicts.push(SyncConflict {
							relative_path: path.clone(),
							source_entry: source_entry_with_path.entry.clone(),
							target_entry: target_entry_with_path.entry.clone(),
							conflict_type: ConflictType::BothModified,
						});
					} else if source_changed {
						// Source changed, target unchanged -> copy to target
						operations
							.source_to_target
							.to_copy
							.push(source_entry_with_path.clone());
					} else if target_changed {
						// Target changed, source unchanged -> copy to source
						if let Some(ref mut target_to_source) = operations.target_to_source {
							target_to_source
								.to_copy
								.push(target_entry_with_path.clone());
						}
					}
				}
				(Some(source_entry_with_path), None) => {
					// Only in source -> copy to target
					operations
						.source_to_target
						.to_copy
						.push(source_entry_with_path.clone());
				}
				(None, Some(target_entry_with_path)) => {
					// Only in target -> copy to source
					if let Some(ref mut target_to_source) = operations.target_to_source {
						target_to_source
							.to_copy
							.push(target_entry_with_path.clone());
					}
				}
				(None, None) => unreachable!(),
			}
		}

		Ok(operations)
	}

	fn content_differs(&self, entry1: &entry::Model, entry2: &entry::Model) -> bool {
		// Compare content identity
		match (entry1.content_id, entry2.content_id) {
			(Some(c1), Some(c2)) => c1 != c2,
			// If either doesn't have content_id, compare by size and modified time
			_ => entry1.size != entry2.size || entry1.modified_at != entry2.modified_at,
		}
	}

	fn detect_changes(
		&self,
		entries: &HashMap<PathBuf, EntryWithPath>,
		last_gen: Option<&sync_generation::Model>,
	) -> HashSet<PathBuf> {
		let mut changed = HashSet::new();

		if let Some(gen) = last_gen {
			let last_sync_time = gen.completed_at.unwrap_or(gen.started_at);
			for (path, entry_with_path) in entries {
				// Check if entry was modified after last sync
				if let Some(indexed_at) = entry_with_path.entry.indexed_at {
					if indexed_at > last_sync_time {
						changed.insert(path.clone());
					}
				}
			}
		} else {
			// No previous sync - all files are "changes"
			changed.extend(entries.keys().cloned());
		}

		changed
	}

	async fn get_last_completed_generation(
		&self,
		conduit_id: i32,
	) -> Result<Option<sync_generation::Model>> {
		Ok(sync_generation::Entity::find()
			.filter(sync_generation::Column::ConduitId.eq(conduit_id))
			.filter(sync_generation::Column::CompletedAt.is_not_null())
			.order_by_desc(sync_generation::Column::Generation)
			.one(&*self.db)
			.await?)
	}
}
