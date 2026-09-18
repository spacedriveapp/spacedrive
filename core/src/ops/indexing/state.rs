//! State management and progress tracking for indexer jobs.
//!
//! This module defines the resumable state machine that tracks indexing progress
//! across all phases. The state is automatically serialized during job shutdowns,
//! allowing indexing to resume from the last completed phase rather than starting
//! over from scratch.

use crate::domain::addressing::SdPath;

use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	collections::{HashMap, HashSet, VecDeque},
	path::PathBuf,
	time::{Duration, Instant},
};
use uuid::Uuid;

/// Progress information sent to UI during indexing operations.
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
	pub action_context: Option<crate::infra::action::context::ActionContext>,
	/// Total volume capacity in bytes (for calculating accurate progress percentage)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub volume_total_capacity: Option<u64>,
}

/// Cumulative statistics tracked throughout the indexing process.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type)]
pub struct IndexerStats {
	pub files: u64,
	pub dirs: u64,
	pub bytes: u64,
	pub symlinks: u64,
	pub skipped: u64,
	pub errors: u64,
}

/// Public-facing phase information exposed to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexPhase {
	Discovery { dirs_queued: usize },
	Processing { batch: usize, total_batches: usize },
	ContentIdentification { current: usize, total: usize },
	Finalizing { processed: usize, total: usize },
}

/// Internal phase enum used by the indexer state machine.
///
/// The state machine progresses linearly through these phases. Each phase
/// completes atomically before transitioning to the next, ensuring the job
/// can resume from a clean checkpoint if interrupted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Phase {
	Discovery,
	Processing,
	Complete,
}

/// Filesystem entry discovered during the discovery phase.
///
/// These are lightweight representations of files and directories found on disk.
/// They're collected in batches before being processed into full database entries,
/// allowing discovery to run ahead of persistence without blocking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
	pub path: PathBuf,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<std::time::SystemTime>,
	/// Birth time. Unrecoverable once a file has been copied, so it is read
	/// during the walk that already stat'd the entry rather than by a later
	/// pass that would find it gone.
	pub created: Option<std::time::SystemTime>,
	pub accessed: Option<std::time::SystemTime>,
	pub inode: Option<u64>,
	/// Unix permission bits.
	pub permissions: Option<u32>,
	pub uid: Option<u32>,
	pub gid: Option<u32>,
	/// Where a symlink points, verbatim from `readlink`.
	pub link_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EntryKind {
	File,
	Directory,
	Symlink,
}

/// Errors encountered during indexing that don't halt the entire job.
///
/// These errors are logged and accumulated but don't cause job failure. This allows
/// indexing to continue even when individual files are inaccessible due to permissions,
/// file locks, or I/O errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexError {
	ReadDir { path: String, error: String },
	CreateEntry { path: String, error: String },
	ContentId { path: String, error: String },
	FilterCheck { path: String, error: String },
}

/// Complete state for a resumable indexer job.
///
/// This struct holds all data needed to resume indexing from any phase. The state
/// is automatically serialized when the job system shuts down, allowing long-running
/// indexing operations to survive app restarts without losing progress.
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerState {
	pub(crate) phase: Phase,
	#[serde(skip, default = "Instant::now")]
	pub(crate) started_at: Instant,
	pub(crate) dirs_to_walk: VecDeque<PathBuf>,
	pub(crate) pending_entries: Vec<DirEntry>,
	pub(crate) seen_paths: HashSet<PathBuf>,
	/// Discovery order, consumed from the front.
	///
	/// A directory is always discovered before its contents, and the store
	/// binds a parent before it can link a child, so consuming these back to
	/// front leaves almost every record unparented.
	pub(crate) entry_batches: VecDeque<Vec<DirEntry>>,
	pub(crate) stats: IndexerStats,
	pub(crate) errors: Vec<IndexError>,
	/// A sweep is open on the durable store, so this walk's absences become
	/// deletions when it closes. Serialized with the rest of the state: a walk
	/// resumed in this process must not open a second sweep over its own first
	/// half, and one resumed in a new process finds a ledger with no sweep
	/// running and closes nothing.
	#[serde(default)]
	pub(crate) sweep_open: bool,
	#[serde(skip, default = "Instant::now")]
	pub(crate) last_progress_time: Instant,
	pub(crate) items_since_last_update: u64,
	pub(crate) batch_size: usize,
	pub(crate) discovery_concurrency: usize,
	pub(crate) dirs_channel_capacity: usize,
	pub(crate) entries_channel_capacity: usize,
	/// Total volume capacity for progress percentage calculation (volume indexing only)
	#[serde(skip)]
	pub(crate) volume_total_capacity: Option<u64>,
	/// Directories the rules turned back at, and what counting found beneath
	/// them. The entry is kept and its contents are not, so these totals are
	/// the only record that the subtree is there at all.
	#[serde(default)]
	pub(crate) summaries: Vec<(PathBuf, u64, u32)>,
}

impl IndexerState {
	pub fn new(root_path: &SdPath) -> Self {
		let mut dirs_to_walk = VecDeque::new();
		if let Some(path) = root_path.as_local_path() {
			dirs_to_walk.push_back(path.to_path_buf());
		}

		let discovery_concurrency = std::thread::available_parallelism()
			.map(|n| usize::max(n.get() / 2, 1))
			.unwrap_or(4);

		Self {
			phase: Phase::Discovery,
			started_at: Instant::now(),
			dirs_to_walk,
			pending_entries: Vec::new(),
			seen_paths: HashSet::new(),
			entry_batches: VecDeque::new(),
			stats: Default::default(),
			errors: Vec::new(),
			sweep_open: false,
			last_progress_time: Instant::now(),
			items_since_last_update: 0,
			batch_size: 1000,
			discovery_concurrency,
			dirs_channel_capacity: 4096,
			entries_channel_capacity: 16384,
			volume_total_capacity: None,
			summaries: Vec::new(),
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
		None
	}

	/// Paths the walk failed to read. Absence under one of these means the walk
	/// did not look, which is not evidence that anything was deleted.
	pub(crate) fn unreachable_paths(&self) -> Vec<PathBuf> {
		self.errors
			.iter()
			.filter_map(|error| match error {
				IndexError::ReadDir { path, .. } | IndexError::FilterCheck { path, .. } => {
					Some(PathBuf::from(path))
				}
				_ => None,
			})
			.collect()
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
}
