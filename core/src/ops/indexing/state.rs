//! Indexer state management and progress tracking

use crate::domain::addressing::SdPath;

use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	collections::{HashMap, HashSet, VecDeque},
	path::PathBuf,
	time::{Duration, Instant},
};

/// Indexer progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerProgress {
	pub phase: IndexPhase,
	pub current_path: String,
	pub total_found: IndexerStats,
	pub processing_rate: f32,
	pub estimated_remaining: Option<Duration>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub scope: Option<super::job::IndexScope>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub persistence: Option<super::job::IndexPersistence>,
	pub is_ephemeral: bool,

	/// Action context that spawned this job (if available)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub action_context: Option<crate::infra::action::context::ActionContext>,
}

/// Statistics collected during indexing
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
pub struct IndexerStats {
	pub files: u64,
	pub dirs: u64,
	pub bytes: u64,
	pub symlinks: u64,
	pub skipped: u64,
	pub errors: u64,
}

/// Current phase of the indexing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexPhase {
	Discovery { dirs_queued: usize },
	Processing { batch: usize, total_batches: usize },
	ContentIdentification { current: usize, total: usize },
	Finalizing { processed: usize, total: usize },
}

/// Internal phases for state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Phase {
	Discovery,
	Processing,
	Aggregation,
	ContentIdentification,
	Complete,
}

/// Directory entry found during discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
	pub path: PathBuf,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<std::time::SystemTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub inode: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EntryKind {
	File,
	Directory,
	Symlink,
}

/// Errors that occur during indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexError {
	ReadDir { path: String, error: String },
	CreateEntry { path: String, error: String },
	ContentId { path: String, error: String },
	FilterCheck { path: String, error: String },
}

/// Resumable indexer state
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerState {
	pub(crate) phase: Phase,
	#[serde(skip, default = "Instant::now")]
	pub(crate) started_at: Instant,

	// Discovery phase
	pub(crate) dirs_to_walk: VecDeque<PathBuf>,
	pub(crate) pending_entries: Vec<DirEntry>,
	pub(crate) seen_paths: HashSet<PathBuf>,

	// Processing phase
	pub(crate) entry_batches: Vec<Vec<DirEntry>>,

	// Content phase
	pub(crate) entries_for_content: Vec<(i32, PathBuf)>, // (entry_id, path)

	// Database operations
	pub(crate) entry_id_cache: HashMap<PathBuf, i32>, // path -> entry_id for parent lookups

	// Change detection
	pub(crate) existing_entries:
		HashMap<PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>)>, // path -> (id, inode, modified)

	// Statistics
	pub(crate) stats: IndexerStats,
	pub(crate) errors: Vec<IndexError>,

	// Performance tracking
	#[serde(skip, default = "Instant::now")]
	pub(crate) last_progress_time: Instant,
	pub(crate) items_since_last_update: u64,

	// Configuration
	pub(crate) batch_size: usize,
	// Discovery config (Phase 2)
	pub(crate) discovery_concurrency: usize,
	pub(crate) dirs_channel_capacity: usize,
	pub(crate) entries_channel_capacity: usize,
}

impl IndexerState {
	pub fn new(root_path: &SdPath) -> Self {
		let mut dirs_to_walk = VecDeque::new();
		if let Some(path) = root_path.as_local_path() {
			dirs_to_walk.push_back(path.to_path_buf());
		}

		Self {
			phase: Phase::Discovery,
			started_at: Instant::now(),
			dirs_to_walk,
			pending_entries: Vec::new(),
			seen_paths: HashSet::new(),
			entry_batches: Vec::new(),
			entries_for_content: Vec::new(),
			entry_id_cache: HashMap::new(),
			existing_entries: HashMap::new(),
			stats: Default::default(),
			errors: Vec::new(),
			last_progress_time: Instant::now(),
			items_since_last_update: 0,
			batch_size: 1000,
			discovery_concurrency: 1,
			dirs_channel_capacity: 4096,
			entries_channel_capacity: 16384,
		}
	}

	pub fn calculate_rate(&mut self) -> f32 {
		let elapsed = self.last_progress_time.elapsed();
		if elapsed.as_secs() > 0 {
			let rate = self.items_since_last_update as f32 / elapsed.as_secs_f32();
			self.last_progress_time = Instant::now();
			self.items_since_last_update = 0;
			rate
		} else {
			0.0
		}
	}

	pub fn estimate_remaining(&self) -> Option<Duration> {
		// TODO: Implement based on current rate and remaining work
		None
	}

	pub fn add_error(&mut self, error: IndexError) {
		self.stats.errors += 1;
		self.errors.push(error);
	}

	pub fn should_create_batch(&self) -> bool {
		self.pending_entries.len() >= self.batch_size
	}

	pub fn create_batch(&mut self) -> Vec<DirEntry> {
		std::mem::take(&mut self.pending_entries)
	}

	/// Seed the entry ID cache with all ancestor directories from location root to target path
	/// This prevents the ghost folder bug where subpath reindexing creates entries with wrong parent_id
	pub async fn seed_ancestor_cache<'a>(
		&mut self,
		db: &sea_orm::DatabaseConnection,
		location_root_path: &std::path::Path,
		location_entry_id: i32,
		target_path: &std::path::Path,
	) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		use crate::infra::db::entities::directory_paths;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Seed location root
		self.entry_id_cache
			.insert(location_root_path.to_path_buf(), location_entry_id);

		// Seed all intermediate ancestors between location root and target path
		if let Ok(relative_path) = target_path.strip_prefix(location_root_path) {
			let mut current_path = location_root_path.to_path_buf();

			for component in relative_path.components() {
				current_path.push(component);

				// Look up this ancestor in directory_paths table
				if let Ok(Some(dir_record)) = directory_paths::Entity::find()
					.filter(
						directory_paths::Column::Path.eq(current_path.to_string_lossy().to_string()),
					)
					.one(db)
					.await
				{
					self.entry_id_cache
						.insert(current_path.clone(), dir_record.entry_id);
				}
			}
		}

		Ok(())
	}
}
