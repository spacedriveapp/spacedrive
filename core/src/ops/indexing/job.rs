//! Main indexer job implementation

use crate::{
	domain::addressing::SdPath,
	infra::db::entities,
	infra::job::{prelude::*, traits::DynJob},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use super::{
	entry::EntryMetadata,
	metrics::{IndexerMetrics, PhaseTimer},
	phases,
	state::{IndexError, IndexPhase, IndexerProgress, IndexerState, IndexerStats, Phase},
	PathResolver,
};

/// Indexing mode determines the depth of indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub enum IndexMode {
	/// Just filesystem metadata (fastest)
	Shallow,
	/// Generate content identities (moderate)
	Content,
	/// Full indexing with thumbnails and text extraction (slowest)
	Deep,
}

/// Indexing scope determines how much of the directory tree to process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IndexScope {
	/// Index only the current directory (single level)
	Current,
	/// Index recursively through all subdirectories
	Recursive,
}

impl Default for IndexScope {
	fn default() -> Self {
		IndexScope::Recursive
	}
}

impl From<&str> for IndexScope {
	fn from(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"current" => IndexScope::Current,
			"recursive" => IndexScope::Recursive,
			_ => IndexScope::Recursive,
		}
	}
}

impl std::fmt::Display for IndexScope {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IndexScope::Current => write!(f, "current"),
			IndexScope::Recursive => write!(f, "recursive"),
		}
	}
}

/// Determines whether indexing results are persisted to database or kept in memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IndexPersistence {
	/// Write all results to database (normal operation)
	Persistent,
	/// Keep results in memory only (for unmanaged paths)
	Ephemeral,
}

impl Default for IndexPersistence {
	fn default() -> Self {
		IndexPersistence::Persistent
	}
}

/// Enhanced configuration for indexer jobs
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
	pub location_id: Option<Uuid>, // None for ephemeral indexing
	pub path: SdPath,
	pub mode: IndexMode,
	pub scope: IndexScope,
	pub persistence: IndexPersistence,
	pub max_depth: Option<u32>, // Override for Current scope or depth limiting
	#[serde(default)]
	pub rule_toggles: super::rules::RuleToggles,
}

impl IndexerJobConfig {
	/// Create a new configuration for persistent recursive indexing (traditional)
	pub fn new(location_id: Uuid, path: SdPath, mode: IndexMode) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode,
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		}
	}

	/// Create configuration for UI directory navigation (quick current scan)
	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode: IndexMode::Shallow,
			scope: IndexScope::Current,
			persistence: IndexPersistence::Persistent,
			max_depth: Some(1),
			rule_toggles: Default::default(),
		}
	}

	/// Create configuration for ephemeral path browsing (outside managed locations)
	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self {
			location_id: None,
			path,
			mode: IndexMode::Shallow,
			scope,
			persistence: IndexPersistence::Ephemeral,
			max_depth: if scope == IndexScope::Current {
				Some(1)
			} else {
				None
			},
			rule_toggles: Default::default(),
		}
	}

	/// Check if this is an ephemeral (non-persistent) job
	pub fn is_ephemeral(&self) -> bool {
		self.persistence == IndexPersistence::Ephemeral
	}

	/// Check if this is a current scope (single level) job
	pub fn is_current_scope(&self) -> bool {
		self.scope == IndexScope::Current
	}
}

/// In-memory storage for ephemeral indexing results
#[derive(Debug)]
pub struct EphemeralIndex {
	pub entries: HashMap<PathBuf, EntryMetadata>,
	pub content_identities: HashMap<String, EphemeralContentIdentity>,
	pub created_at: std::time::Instant,
	pub last_accessed: std::time::Instant,
	pub root_path: PathBuf,
	pub stats: IndexerStats,
}

/// Simplified content identity for ephemeral storage
#[derive(Debug, Clone)]
pub struct EphemeralContentIdentity {
	pub cas_id: String,
	pub mime_type: Option<String>,
	pub file_size: u64,
	pub entry_count: u32,
}

impl EphemeralIndex {
	pub fn new(root_path: PathBuf) -> Self {
		let now = std::time::Instant::now();
		Self {
			entries: HashMap::new(),
			content_identities: HashMap::new(),
			created_at: now,
			last_accessed: now,
			root_path,
			stats: IndexerStats::default(),
		}
	}

	pub fn add_entry(&mut self, path: PathBuf, metadata: EntryMetadata) {
		self.entries.insert(path, metadata);
		self.last_accessed = std::time::Instant::now();
	}

	pub fn get_entry(&mut self, path: &PathBuf) -> Option<&EntryMetadata> {
		self.last_accessed = std::time::Instant::now();
		self.entries.get(path)
	}

	pub fn add_content_identity(&mut self, cas_id: String, content: EphemeralContentIdentity) {
		self.content_identities.insert(cas_id, content);
		self.last_accessed = std::time::Instant::now();
	}

	pub fn age(&self) -> Duration {
		self.created_at.elapsed()
	}

	pub fn idle_time(&self) -> Duration {
		self.last_accessed.elapsed()
	}
}

/// Indexer job - discovers and indexes files in a location
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IndexerJob {
	pub config: IndexerJobConfig,

	// Resumable state
	state: Option<IndexerState>,

	// Ephemeral storage for non-persistent jobs
	#[serde(skip)]
	ephemeral_index: Option<Arc<RwLock<EphemeralIndex>>>,

	// Performance tracking
	#[serde(skip)]
	timer: Option<PhaseTimer>,
	#[serde(skip)]
	db_operations: (u64, u64), // (reads, writes)
	#[serde(skip)]
	batch_info: (u64, usize), // (count, total_size)
}

impl Job for IndexerJob {
	const NAME: &'static str = "indexer";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Index files in a location");
}

impl DynJob for IndexerJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

impl JobProgress for IndexerProgress {}

#[async_trait::async_trait]
impl JobHandler for IndexerJob {
	type Output = IndexerOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Initialize timer
		if self.timer.is_none() {
			self.timer = Some(PhaseTimer::new());
		}

		// Initialize ephemeral index if needed
		if self.config.is_ephemeral() && self.ephemeral_index.is_none() {
			let root_path =
				self.config.path.as_local_path().ok_or_else(|| {
					JobError::execution("Path not accessible locally".to_string())
				})?;
			self.ephemeral_index = Some(Arc::new(RwLock::new(EphemeralIndex::new(
				root_path.to_path_buf(),
			))));
			ctx.log("Initialized ephemeral index for non-persistent job");
		}

		// Initialize or restore state
		// Ensure state is always created early to avoid serialization issues
		if self.state.is_none() {
			ctx.log(format!(
				"Starting new indexer job (scope: {}, persistence: {:?})",
				self.config.scope, self.config.persistence
			));
			info!("INDEXER_STATE: Job starting with NO saved state - creating new state");
			self.state = Some(IndexerState::new(&self.config.path));
		} else {
			ctx.log("Resuming indexer from saved state");
			let state = self.state.as_ref().unwrap();
			info!("INDEXER_STATE: Job resuming with saved state - phase: {:?}, entry_batches: {}, entries_for_content: {}, seen_paths: {}",
				state.phase,
				state.entry_batches.len(),
				state.entries_for_content.len(),
				state.seen_paths.len());
			warn!(
				"DEBUG: Resumed state - phase: {:?}, entry_batches: {}, entries_for_content: {}",
				state.phase,
				state.entry_batches.len(),
				state.entries_for_content.len()
			);
		}

		let state = self.state.as_mut().unwrap();

		// Get root path ONCE for the entire job
		// For cloud volumes, we use the path component from the SdPath (e.g., "/" or "folder/")
		// since discovery operates through the volume backend (not direct filesystem access)
		let root_path_buf = if let Some(p) = self.config.path.as_local_path() {
			p.to_path_buf()
		} else if let Some(cloud_path) = self.config.path.cloud_path() {
			// Cloud path - use the path component within the cloud volume
			// The actual I/O will go through the volume backend
			PathBuf::from(cloud_path)
		} else if !self.config.is_ephemeral() {
			let loc_uuid = self
				.config
				.location_id
				.ok_or_else(|| JobError::execution("Missing location id".to_string()))?;
			let db = ctx.library().db();
			let location = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.eq(loc_uuid))
				.one(db.conn())
				.await
				.map_err(|e| JobError::execution(e.to_string()))?
				.ok_or_else(|| JobError::execution("Location not found".to_string()))?;
			let entry_id = location
				.entry_id
				.ok_or_else(|| JobError::execution("Location has no entry_id".to_string()))?;
			let path_str = PathResolver::get_directory_path(db.conn(), entry_id)
				.await
				.map_err(|e| JobError::execution(e.to_string()))?;
			std::path::PathBuf::from(path_str)
		} else {
			return Err(JobError::execution(
				"Location root path is not local".to_string(),
			));
		};
		let root_path = root_path_buf.as_path();

		// Get volume backend for the entire job
		let volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>> =
			if let Some(vm) = ctx.volume_manager() {
				match vm
					.resolve_volume_for_sdpath(&self.config.path, ctx.library())
					.await
				{
					Ok(Some(mut volume)) => {
						ctx.log(format!(
							"Using volume backend: {} for path: {}",
							volume.name, self.config.path
						));
						Some(vm.backend_for_volume(&mut volume))
					}
					Ok(None) => {
						// For cloud paths, we MUST have a volume - can't fall back to local
						if self.config.path.is_cloud() {
							ctx.log(format!(
								"Cloud volume not found for path: {}",
								self.config.path
							));
							return Err(JobError::execution(format!(
								"Cloud volume not found for path: {}. The cloud volume may not be registered yet.",
								self.config.path
							)));
						}

						// For local paths, we can fall back to LocalBackend
						ctx.log(format!(
							"No volume found for path: {}, will use LocalBackend fallback",
							self.config.path
						));
						None
					}
					Err(e) => {
						ctx.log(format!("Failed to resolve volume: {}", e));
						return Err(JobError::execution(format!(
							"Failed to resolve volume: {}",
							e
						)));
					}
				}
			} else {
				ctx.log("No volume manager available, will use LocalBackend fallback");
				None
			};

		// Seed discovery queue if it wasn't initialized due to device-id timing
		if state.dirs_to_walk.is_empty() {
			state.dirs_to_walk.push_back(root_path.to_path_buf());
		}

		// Main state machine loop
		loop {
			ctx.check_interrupt().await?;

			let current_phase = state.phase.clone();
			warn!("DEBUG: IndexerJob entering phase: {:?}", current_phase);
			match current_phase {
				Phase::Discovery => {
					// Use scope-aware discovery
					if self.config.is_current_scope() {
						Self::run_current_scope_discovery_static(state, &ctx, root_path).await?;
					} else {
						phases::run_discovery_phase(
							state,
							&ctx,
							root_path,
							self.config.rule_toggles.clone(),
							volume_backend.as_ref(),
						)
						.await?;
					}

					// Track batch info
					self.batch_info.0 = state.entry_batches.len() as u64;
					self.batch_info.1 = state.entry_batches.iter().map(|b| b.len()).sum();

					// Start processing timer
					if let Some(timer) = &mut self.timer {
						timer.start_processing();
					}
				}

				Phase::Processing => {
					warn!("DEBUG: IndexerJob starting Processing phase");
					if self.config.is_ephemeral() {
						let ephemeral_index = self.ephemeral_index.clone().ok_or_else(|| {
							JobError::execution("Ephemeral index not initialized".to_string())
						})?;
						Self::run_ephemeral_processing_static(
							state,
							&ctx,
							ephemeral_index,
							volume_backend.as_ref(),
						)
						.await?;
					} else {
						phases::run_processing_phase(
							self.config
								.location_id
								.expect("Location ID required for persistent jobs"),
							state,
							&ctx,
							self.config.mode,
							root_path,
							volume_backend.as_ref(),
						)
						.await?;

						// Update DB operation counts
						self.db_operations.1 += state.entry_batches.len() as u64 * 100; // Estimate
					}
				}

				Phase::Aggregation => {
					if !self.config.is_ephemeral() {
						phases::run_aggregation_phase(
							self.config
								.location_id
								.expect("Location ID required for persistent jobs"),
							state,
							&ctx,
						)
						.await?;
					} else {
						// Skip aggregation for ephemeral jobs
						ctx.log("Skipping aggregation phase for ephemeral job");
						state.phase = Phase::ContentIdentification;
						continue;
					}

					// Start content timer
					if let Some(timer) = &mut self.timer {
						timer.start_content();
					}
				}

				Phase::ContentIdentification => {
					if self.config.mode >= IndexMode::Content {
						if self.config.is_ephemeral() {
							let ephemeral_index =
								self.ephemeral_index.clone().ok_or_else(|| {
									JobError::execution(
										"Ephemeral index not initialized".to_string(),
									)
								})?;
							Self::run_ephemeral_content_phase_static(state, &ctx, ephemeral_index)
								.await?;
						} else {
							let library_id = ctx.library().id();
							phases::run_content_phase(
								state,
								&ctx,
								library_id,
								volume_backend.as_ref(),
							)
							.await?;
							self.db_operations.1 += state.entries_for_content.len() as u64;
						}
					} else {
						ctx.log("Skipping content identification phase (mode=Shallow)");
						state.phase = Phase::Complete;
					}
				}

				Phase::Complete => break,
			}

			// State is automatically saved during job serialization on shutdown
			warn!(
				"DEBUG: IndexerJob completed phase: {:?}, next phase will be: {:?}",
				current_phase, state.phase
			);
		}

		// Send final progress update
		let final_progress = IndexerProgress {
			phase: IndexPhase::Finalizing {
				processed: 0,
				total: 0,
			},
			current_path: "Completed".to_string(),
			total_found: state.stats,
			processing_rate: 0.0,
			estimated_remaining: None,
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None, // TODO: Pass action context from job state
		};
		ctx.progress(Progress::generic(final_progress.to_generic_progress()));

		// Calculate final metrics
		let metrics = if let Some(timer) = &self.timer {
			IndexerMetrics::calculate(&state.stats, timer, self.db_operations, self.batch_info)
		} else {
			IndexerMetrics::default()
		};

		// Log summary
		ctx.log(&metrics.format_summary());

		// If Deep mode, dispatch thumbnail generation job after indexing completes
		if self.config.mode == IndexMode::Deep && !self.config.is_ephemeral() {
			use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

			ctx.log("Deep mode enabled - dispatching thumbnail generation job");

			// Dispatch thumbnail job for all entries in this location
			let thumbnail_config = ThumbnailJobConfig::default();
			let thumbnail_job = ThumbnailJob::new(thumbnail_config);

			match ctx.library().jobs().dispatch(thumbnail_job).await {
				Ok(_handle) => {
					ctx.log("Successfully dispatched thumbnail generation job");
				}
				Err(e) => {
					ctx.log(format!("Warning: Failed to dispatch thumbnail job: {}", e));
					// Don't fail the indexing job if thumbnail dispatch fails
				}
			}
		}

		// Generate final output
		Ok(IndexerOutput {
			location_id: self.config.location_id,
			stats: state.stats,
			duration: state.started_at.elapsed(),
			errors: state.errors.clone(),
			metrics: Some(metrics),
			ephemeral_results: if self.config.is_ephemeral() {
				self.ephemeral_index.clone()
			} else {
				None
			},
		})
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		// State is already loaded from serialization
		warn!("DEBUG: IndexerJob on_resume called");
		if let Some(state) = &self.state {
			warn!(
				"DEBUG: IndexerJob has state, resuming in {:?} phase",
				state.phase
			);
			ctx.log(format!("Resuming indexer in {:?} phase", state.phase));
			ctx.log(format!(
				"Progress: {} files, {} dirs, {} errors so far",
				state.stats.files, state.stats.dirs, state.stats.errors
			));

			// Reinitialize timer for resumed job
			self.timer = Some(PhaseTimer::new());
		} else {
			warn!("DEBUG: IndexerJob has no state during resume - creating new state!");
			// If state is missing, create it now (this shouldn't happen in normal operation)
			self.state = Some(IndexerState::new(&self.config.path));
		}
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing indexer job - state will be preserved");
		Ok(())
	}

	async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Cancelling indexer job");
		if let Some(state) = &self.state {
			ctx.log(format!(
				"Final stats: {} files, {} dirs indexed before cancellation",
				state.stats.files, state.stats.dirs
			));
		}
		Ok(())
	}

	fn is_resuming(&self) -> bool {
		// If we have existing state, we're resuming
		self.state.is_some()
	}
}

impl IndexerJob {
	/// Create a new indexer job with enhanced configuration
	pub fn new(config: IndexerJobConfig) -> Self {
		Self {
			config,
			state: None,
			ephemeral_index: None,
			timer: None,
			db_operations: (0, 0),
			batch_info: (0, 0),
		}
	}

	/// Create a traditional persistent recursive indexer job
	pub fn from_location(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
		Self::new(IndexerJobConfig::new(location_id, root_path, mode))
	}

	/// Create a shallow indexer job (metadata only)
	pub fn shallow(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Shallow)
	}

	/// Create a content indexer job (with CAS IDs)
	pub fn with_content(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Content)
	}

	/// Create a deep indexer job (full processing)
	pub fn deep(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Deep)
	}

	/// Create a UI navigation job (current scope, quick scan)
	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self::new(IndexerJobConfig::ui_navigation(location_id, path))
	}

	/// Set the ephemeral index storage (must be called before dispatching for ephemeral jobs)
	/// This allows external code to maintain a reference to the same storage the job uses
	pub fn set_ephemeral_index(&mut self, index: Arc<RwLock<EphemeralIndex>>) {
		self.ephemeral_index = Some(index);
	}

	/// Create an ephemeral browsing job (no database writes)
	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self::new(IndexerJobConfig::ephemeral_browse(path, scope))
	}

	/// Run current scope discovery (single level only)
	async fn run_current_scope_discovery_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		root_path: &std::path::Path,
	) -> JobResult<()> {
		use super::entry::EntryProcessor;
		use super::state::{DirEntry, EntryKind};
		use tokio::fs;

		ctx.log("Starting current scope discovery (single level)");

		let mut entries = fs::read_dir(root_path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to read directory: {}", e)))?;

		while let Some(entry) = entries
			.next_entry()
			.await
			.map_err(|e| JobError::execution(format!("Failed to read directory entry: {}", e)))?
		{
			let path = entry.path();
			let metadata = entry
				.metadata()
				.await
				.map_err(|e| JobError::execution(format!("Failed to read metadata: {}", e)))?;

			let entry_kind = if metadata.is_dir() {
				EntryKind::Directory
			} else if metadata.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			};

			let dir_entry = DirEntry {
				path: path.clone(),
				kind: entry_kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				inode: EntryProcessor::get_inode(&metadata),
			};

			state.pending_entries.push(dir_entry);
			state.items_since_last_update += 1;

			// Update stats
			match entry_kind {
				EntryKind::File => state.stats.files += 1,
				EntryKind::Directory => state.stats.dirs += 1,
				EntryKind::Symlink => state.stats.symlinks += 1,
			}
		}

		// Create single batch and move to processing
		if !state.pending_entries.is_empty() {
			let batch = state.create_batch();
			state.entry_batches.push(batch);
		}

		state.phase = Phase::Processing;
		ctx.log(format!(
			"Current scope discovery complete: {} entries found",
			state.stats.files + state.stats.dirs
		));

		Ok(())
	}

	/// Run ephemeral processing (store in memory instead of database)
	async fn run_ephemeral_processing_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		ephemeral_index: Arc<RwLock<EphemeralIndex>>,
		volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	) -> JobResult<()> {
		use super::persistence::PersistenceFactory;

		ctx.log("Starting ephemeral processing");

		// Get root path from ephemeral index
		let root_path = {
			let index = ephemeral_index.read().await;
			index.root_path.clone()
		};

		// Get event bus from library
		let event_bus = Some(ctx.library().event_bus().clone());

		// Create ephemeral persistence layer (emits events as entries are stored)
		let persistence = PersistenceFactory::ephemeral(
			ephemeral_index.clone(),
			event_bus,
			root_path.clone(),
		);

		// Process all batches through persistence layer
		while let Some(batch) = state.entry_batches.pop() {
			for entry in batch {
				// Store entry (this will emit ResourceChanged events)
				persistence
					.store_entry(&entry, None, &root_path)
					.await?;
			}
		}

		state.phase = Phase::ContentIdentification;

		ctx.log("Ephemeral processing complete");
		Ok(())
	}

	/// Run ephemeral content identification
	async fn run_ephemeral_content_phase_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		_ephemeral_index: Arc<RwLock<EphemeralIndex>>,
	) -> JobResult<()> {
		ctx.log("Starting ephemeral content identification");

		// For ephemeral jobs, we can skip heavy content processing or do it lightly
		// This is mainly for demonstration - in practice you might generate CAS IDs

		state.phase = Phase::Complete;
		ctx.log("Ephemeral content identification complete");

		Ok(())
	}
}

/// Job output with comprehensive results
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
	pub location_id: Option<Uuid>,
	pub stats: IndexerStats,
	pub duration: Duration,
	pub errors: Vec<IndexError>,
	pub metrics: Option<IndexerMetrics>,
	#[serde(skip)]
	pub ephemeral_results: Option<Arc<RwLock<EphemeralIndex>>>,
}

impl From<IndexerOutput> for JobOutput {
	fn from(output: IndexerOutput) -> Self {
		JobOutput::Indexed {
			stats: output.stats,
			metrics: output.metrics.unwrap_or_default(),
		}
	}
}
