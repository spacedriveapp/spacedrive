//! # Persistence Abstraction for Indexing
//!
//! `core::ops::indexing::persistence` provides a unified interface for storing
//! indexing results either persistently in the database or ephemerally in memory.
//! This abstraction allows the same indexing pipeline to work for both managed
//! locations (database-backed) and ephemeral browsing (memory-only).
//!
//! For ephemeral storage, use `MemoryAdapter` from `crate::ops::indexing::ephemeral`
//! which implements both `IndexPersistence` and `ChangeHandler`.
//!
//! For persistent storage, use `DatabaseAdapterForJob` from `crate::ops::indexing::change_detection`
//! which implements `IndexPersistence` and delegates to `DBWriter` for database writes.

use crate::infra::job::prelude::{JobError, JobResult};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

use super::{ephemeral::EphemeralIndex, state::DirEntry};

/// Unified storage interface for persistent and ephemeral indexing.
///
/// Implementations handle either database writes (`DatabaseAdapterForJob`) or
/// in-memory storage (`MemoryAdapter`). The indexing pipeline calls
/// these methods without knowing which backend is active.
#[async_trait::async_trait]
pub trait IndexPersistence: Send + Sync {
	/// Stores an entry and returns its ID for linking content identities.
	///
	/// For database persistence, this creates an `entry` row and updates the closure table.
	/// For ephemeral persistence, this adds the entry to the in-memory index and emits
	/// a ResourceChanged event for immediate UI updates.
	async fn store_entry(
		&self,
		entry: &DirEntry,
		location_id: Option<i32>,
		location_root_path: &Path,
	) -> JobResult<i32>;

	/// Links a content identity (hash) to an entry.
	///
	/// For database persistence, this creates or finds a `content_identity` row and updates
	/// the entry's `content_id` foreign key. For ephemeral persistence, this is a no-op since
	/// in-memory indexes don't track content deduplication across sessions.
	async fn store_content_identity(
		&self,
		entry_id: i32,
		path: &Path,
		cas_id: String,
	) -> JobResult<()>;

	/// Retrieves existing entries under a path for change detection.
	///
	/// Returns a map of path -> (entry_id, inode, modified_time, size) for all entries
	/// under the indexing path. Change detection compares this snapshot against the
	/// current filesystem to identify additions, modifications, and deletions. Ephemeral
	/// persistence returns an empty map since it doesn't support incremental indexing.
	async fn get_existing_entries(
		&self,
		indexing_path: &Path,
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	>;

	async fn update_entry(&self, entry_id: i32, entry: &DirEntry) -> JobResult<()>;

	/// Returns true for database persistence, false for ephemeral.
	///
	/// Used by the indexing pipeline to determine whether to perform expensive operations
	/// like change detection (database only) or content hashing (database only).
	fn is_persistent(&self) -> bool;
}

/// Factory for creating appropriate persistence implementations
pub struct PersistenceFactory;

impl PersistenceFactory {
	/// Create a database persistence instance using the unified DatabaseAdapterForJob.
	///
	/// This delegates to `DBWriter` for all database operations, ensuring
	/// consistency between the watcher and indexer pipelines.
	pub fn database<'a>(
		ctx: &'a crate::infra::job::prelude::JobContext<'a>,
		library_id: uuid::Uuid,
		location_root_entry_id: Option<i32>,
		device_id: i32,
	) -> Box<dyn IndexPersistence + 'a> {
		use crate::ops::indexing::change_detection::DatabaseAdapterForJob;

		Box::new(DatabaseAdapterForJob::new(
			ctx,
			library_id,
			location_root_entry_id,
			device_id,
		))
	}

	/// Create an ephemeral persistence instance using the unified MemoryAdapter.
	pub fn ephemeral(
		index: std::sync::Arc<tokio::sync::RwLock<EphemeralIndex>>,
		event_bus: Option<std::sync::Arc<crate::infra::event::EventBus>>,
		root_path: PathBuf,
	) -> Box<dyn IndexPersistence + Send + Sync> {
		use super::ephemeral::MemoryAdapter;

		let event_bus = event_bus
			.unwrap_or_else(|| std::sync::Arc::new(crate::infra::event::EventBus::new(1024)));

		Box::new(MemoryAdapter::new(index, event_bus, root_path))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::Event;
	use crate::ops::indexing::ephemeral::MemoryAdapter;
	use crate::ops::indexing::state::{DirEntry, EntryKind};
	use std::sync::Arc;
	use tempfile::TempDir;
	use tokio::sync::RwLock;

	#[tokio::test]
	async fn test_ephemeral_writer_via_factory() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));

		let event_bus = Arc::new(crate::infra::event::EventBus::new(1024));
		let mut subscriber = event_bus.subscribe();

		let writer = PersistenceFactory::ephemeral(
			index.clone(),
			Some(event_bus),
			temp_dir.path().to_path_buf(),
		);

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		let entry_id = writer
			.store_entry(&dir_entry, None, temp_dir.path())
			.await
			.unwrap();

		assert!(entry_id > 0);
		assert!(!writer.is_persistent());

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv()).await;

		assert!(event.is_ok(), "Should receive an event");
		if let Ok(Ok(Event::ResourceChanged { resource, .. })) = event {
			let uuid = resource["id"].as_str();
			assert!(uuid.is_some(), "Event should have UUID");
		}
	}

	#[tokio::test]
	async fn test_ephemeral_writer_direct() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));
		let event_bus = Arc::new(crate::infra::event::EventBus::new(1024));

		let writer = MemoryAdapter::new(index.clone(), event_bus, temp_dir.path().to_path_buf());

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		let entry_id = writer
			.store_entry(&dir_entry, None, temp_dir.path())
			.await
			.unwrap();

		assert!(entry_id > 0);

		let idx = index.read().await;
		assert!(idx.has_entry(&test_file));
	}
}
