//! Indexer job implementation.
//!
//! This module contains the main `IndexerJob` struct that orchestrates the multi-phase
//! indexing pipeline. The job supports both persistent indexing (for managed locations)
//! and ephemeral indexing (for external drives, network shares, and temporary browsing).
//!

use crate::{
	domain::addressing::SdPath,
	infra::db::entities,
	infra::job::{prelude::*, traits::DynJob},
};

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use super::{
	ephemeral::{ArenaWriter, EphemeralIndex, Notify, Rollup, Seen},
	metrics::{IndexerMetrics, PhaseTimer},
	phases,
	state::{IndexError, IndexPhase, IndexerProgress, IndexerState, IndexerStats, Phase},
	summary::Retention,
	PathResolver,
};

/// Whether to index just one directory level or recurse through subdirectories.
///
/// Current scope is used for UI navigation where users expand folders on-demand,
/// while Recursive scope is used for full location indexing. Current scope with
/// persistent storage enables progressive indexing where the UI drives which
/// directories get indexed based on user interaction.
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

/// Configuration for an indexer job.
///
/// Every walk fills the volume index for the drive it is on. What varies is
/// how much of the drive it reaches, what it keeps of what it sees, and
/// whether anyone is watching it happen.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
	pub path: SdPath,
	pub scope: IndexScope,
	pub max_depth: Option<u32>,
	#[serde(default)]
	pub rule_toggles: super::rules::RuleToggles,
	/// Whether to run this job in the background (not persisted to database, no UI updates)
	#[serde(default)]
	pub run_in_background: bool,
	/// Whether this is indexing a full volume (for progress tracking)
	#[serde(default)]
	pub is_volume_indexing: bool,
	/// What the walk keeps of what it visits. Everything by default; the
	/// background map of a drive keeps structure and counts the rest.
	#[serde(default)]
	pub retention: super::summary::Retention,
}

impl IndexerJobConfig {
	pub fn ephemeral_browse(path: SdPath, scope: IndexScope, is_volume: bool) -> Self {
		Self {
			path,
			scope,
			max_depth: if scope == IndexScope::Current {
				Some(1)
			} else {
				None
			},
			rule_toggles: Default::default(),
			run_in_background: false,
			is_volume_indexing: is_volume,
			retention: Retention::everything(),
		}
	}

	/// Whether this walk enumerates the whole source with nothing filtered out.
	///
	/// Only such a walk may open a sweep on the durable store. A sweep reads
	/// absence as deletion, which is sound only when the walk would have seen
	/// the file had it been there — so it rules out a browse, which stops at
	/// one directory, and every walk that applies rules, which hide files on
	/// purpose. In practice this is archiving a removable drive, and that is
	/// also the one case where the origin goes in a drawer and absence is the
	/// only signal there will ever be.
	pub fn enumerates_whole_source(&self) -> bool {
		self.is_volume_indexing
			&& self.scope == IndexScope::Recursive
			&& self.max_depth.is_none()
			&& self.rule_toggles == super::rules::RuleToggles::none()
	}

	/// Check if this is a current scope (single level) job
	pub fn is_current_scope(&self) -> bool {
		self.scope == IndexScope::Current
	}
}

/// Walks a path and fills the volume index for the drive it is on.
///
/// The job is a state machine over Discovery and Processing: discovery reads
/// the filesystem in batches, processing applies each batch to the arena
/// through `ArenaWriter`. State is serialized between phases, so a job that is
/// interrupted resumes where it stopped rather than re-reading the tree.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IndexerJob {
	pub config: IndexerJobConfig,
	state: Option<IndexerState>,
	#[serde(skip)]
	ephemeral_index: Option<Arc<RwLock<EphemeralIndex>>>,
	#[serde(skip)]
	source_store: Option<Arc<crate::ops::indexing::ephemeral::SourceStore>>,
	#[serde(skip)]
	timer: Option<PhaseTimer>,
	#[serde(skip)]
	db_operations: (u64, u64),
	#[serde(skip)]
	batch_info: (u64, usize),
}

impl Job for IndexerJob {
	const NAME: &'static str = "indexer";
	// A walk is a sweep, and a sweep's observation set lives in memory: what
	// this run has seen so far dies with the process. A resumed walk that
	// finished its sweep would delete everything the interrupted half
	// observed, which is exactly what happened the one time it ran. A fresh
	// walk is cheap and correct; there is nothing here worth resuming.
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Index files in a location");
}

impl DynJob for IndexerJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One walk per root at a time. A second track of the same root while a
	/// walk is running would race the sweep it is part of.
	fn dedup_key(&self) -> Option<String> {
		Some(self.config.path.display())
	}

	fn should_persist(&self) -> bool {
		!self.config.run_in_background
	}

	fn should_emit_events(&self) -> bool {
		self.config.is_volume_indexing || self.should_persist()
	}
}

impl JobProgress for IndexerProgress {}

impl IndexerJob {
	async fn run_job_phases(&mut self, ctx: &JobContext<'_>) -> JobResult<IndexerOutput> {
		if self.state.is_none() {
			ctx.log(format!(
				"Starting new indexer job (scope: {})",
				self.config.scope
			));
			ctx.log_debug("Job starting with no saved state - creating new state");
			self.state = Some(IndexerState::new(&self.config.path));
		} else {
			ctx.log("Resuming indexer from saved state");
			ctx.log_debug(format!(
				"Job resuming with saved state - phase: {:?}, entry_batches: {}, entries_for_content: {}, seen_paths: {}",
				self.state.as_ref().unwrap().phase,
				self.state.as_ref().unwrap().entry_batches.len(),
				self.state.as_ref().unwrap().entries_for_content.len(),
				self.state.as_ref().unwrap().seen_paths.len()
			));
		}

		let state = self.state.as_mut().unwrap();

		let root_path_buf = if let Some(p) = self.config.path.as_local_path() {
			p.to_path_buf()
		} else if let Some(cloud_path) = self.config.path.cloud_path() {
			PathBuf::from(cloud_path)
		} else {
			return Err(JobError::execution(
				"Index root path is not local".to_string(),
			));
		};
		let root_path = root_path_buf.as_path();

		// Resolve volume backend for I/O operations and get capacity for progress
		let mut volume_total_capacity: Option<u64> = None;
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

						// Store volume capacity for progress calculations if indexing full volume
						if self.config.is_volume_indexing {
							volume_total_capacity = Some(volume.total_capacity);
							ctx.log(format!(
								"Volume indexing: total capacity {} GB",
								volume.total_capacity / (1024 * 1024 * 1024)
							));
						}

						Some(vm.backend_for_volume(&mut volume))
					}
					Ok(None) => {
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

		// Store volume capacity in state for progress calculations
		state.volume_total_capacity = volume_total_capacity;

		if state.dirs_to_walk.is_empty() {
			state.dirs_to_walk.push_back(root_path.to_path_buf());
		}

		loop {
			ctx.check_interrupt().await?;

			let current_phase = state.phase.clone();
			match current_phase {
				Phase::Discovery => {
					let cloud_url_base =
						if let Some((service, identifier, _)) = self.config.path.as_cloud() {
							Some(format!("{}://{}/", service.scheme(), identifier))
						} else {
							None
						};

					if self.config.is_current_scope() {
						Self::run_current_scope_discovery_static(state, &ctx, root_path).await?;
					} else {
						phases::run_discovery_phase(
							state,
							&ctx,
							root_path,
							self.config.rule_toggles.clone(),
							self.config.retention.clone(),
							volume_backend.as_ref(),
							cloud_url_base,
						)
						.await?;
					}

					self.batch_info.0 = state.entry_batches.len() as u64;
					self.batch_info.1 = state.entry_batches.iter().map(|b| b.len()).sum();

					if let Some(timer) = &mut self.timer {
						timer.start_processing();
					}
				}

				Phase::Processing => {
					let ephemeral_index = self.ephemeral_index.clone().ok_or_else(|| {
						JobError::execution("Volume index not initialized".to_string())
					})?;

					// Discovery has finished by now, so the walk's full reach,
					// and everything it failed to read, is known.
					if let Some(store) = &self.source_store {
						if self.config.enumerates_whole_source() && !state.sweep_open {
							store.begin_sweep().await;
							state.sweep_open = true;
						}
					}

					Self::run_ephemeral_processing_static(
						state,
						&ctx,
						ephemeral_index,
						self.source_store.clone(),
						root_path,
						volume_backend.as_ref(),
						self.config.is_volume_indexing,
					)
					.await?;

					// Only reached when every batch landed. An interrupt
					// returns above, leaving the sweep open for the resume
					// rather than closing it over half a walk.
					if let Some(store) = &self.source_store {
						if state.sweep_open {
							store.finish_sweep(&state.unreachable_paths()).await;
							state.sweep_open = false;
						}
					}

					state.phase = Phase::Complete;
				}

				Phase::Complete => break,
			}
		}

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
			is_ephemeral: false,
			action_context: None,
			volume_total_capacity,
		};
		ctx.progress(Progress::generic(final_progress.to_generic_progress()));

		let metrics = if let Some(timer) = &self.timer {
			IndexerMetrics::calculate(&state.stats, timer, self.db_operations, self.batch_info)
		} else {
			IndexerMetrics::default()
		};

		ctx.log(&metrics.format_summary());

		Ok(IndexerOutput {
			stats: state.stats,
			duration: state.started_at.elapsed(),
			errors: state.errors.clone(),
			metrics: Some(metrics),
			ephemeral_results: self.ephemeral_index.clone(),
		})
	}
}

// JobHandler trait implementation
#[async_trait::async_trait]
impl JobHandler for IndexerJob {
	type Output = IndexerOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if self.timer.is_none() {
			self.timer = Some(PhaseTimer::new());
		}

		if self.ephemeral_index.is_none() {
			// Try to load from snapshot first
			let cache = ctx.library().core_context().ephemeral_cache();
			let snapshot_loaded = if let Some(local_path) = self.config.path.as_local_path() {
				match cache.try_load_snapshot_or_create(local_path).await {
					Ok(true) => {
						ctx.log(format!(
							"Loaded ephemeral index from snapshot for: {}",
							local_path.display()
						));
						true
					}
					Ok(false) => {
						ctx.log("No snapshot found, will perform full index");
						false
					}
					Err(e) => {
						ctx.log(format!(
							"Failed to load snapshot, will perform full index: {}",
							e
						));
						false
					}
				}
			} else {
				false
			};

			// If snapshot not loaded, create new index for indexing
			if !snapshot_loaded {
				let index = EphemeralIndex::new().map_err(|e| {
					JobError::Other(format!("Failed to create ephemeral index: {}", e))
				})?;
				self.ephemeral_index = Some(Arc::new(RwLock::new(index)));
				ctx.log("Initialized ephemeral index for non-persistent job");
			}
		}

		let result = self.run_job_phases(&ctx).await;

		// Settle the ephemeral flags either way: leaving a path in progress
		// blocks every future attempt at it, and recording a failed run as
		// indexed serves a partial arena as if it were complete.
		{
			if let Some(local_path) = self.config.path.as_local_path() {
				let cache = ctx.library().core_context().ephemeral_cache();
				match &result {
					Ok(_) => cache.mark_indexing_complete(local_path),
					Err(_) => cache.mark_indexing_failed(local_path),
				}
				match &result {
					Ok(_) => {
						ctx.log(format!(
							"Marked ephemeral indexing complete for: {}",
							local_path.display()
						));

						// Save snapshot for fast restoration next time
						if let Err(e) = ctx
							.library()
							.core_context()
							.ephemeral_cache()
							.save_snapshot(local_path)
							.await
						{
							ctx.log(format!(
								"Warning: Failed to save snapshot for {}: {}",
								local_path.display(),
								e
							));
						} else {
							ctx.log(format!("Saved snapshot for: {}", local_path.display()));
						}

						// Automatically add filesystem watch for successfully indexed ephemeral paths
						// This enables real-time updates when files change in browsed directories
						if let Some(watcher) = ctx.library().core_context().get_fs_watcher().await {
							if let Err(e) = watcher.watch_ephemeral(local_path.to_path_buf()).await
							{
								ctx.add_warning(format!(
									"Failed to add ephemeral watch for {}: {}",
									local_path.display(),
									e
								));
							} else {
								ctx.log(format!(
									"Added ephemeral watch for: {}",
									local_path.display()
								));
							}
						}
					}
					Err(e) => ctx.log(format!(
						"Ephemeral indexing failed ({}) for {}; cleared so the next \
						 browse re-dispatches",
						e,
						local_path.display()
					)),
				}
			}
		}

		result
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		if let Some(state) = &self.state {
			ctx.log(format!("Resuming indexer in {:?} phase", state.phase));
			ctx.log(format!(
				"Progress: {} files, {} dirs, {} errors so far",
				state.stats.files, state.stats.dirs, state.stats.errors
			));

			self.timer = Some(PhaseTimer::new());
		} else {
			self.state = Some(IndexerState::new(&self.config.path));
		}
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing indexer job");
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
		self.state.is_some()
	}
}

impl IndexerJob {
	pub fn new(config: IndexerJobConfig) -> Self {
		Self {
			config,
			state: None,
			ephemeral_index: None,
			source_store: None,
			timer: None,
			db_operations: (0, 0),
			batch_info: (0, 0),
		}
	}

	/// Sets the ephemeral index storage that the job will use.
	///
	/// This must be called before dispatching ephemeral jobs. It allows external code
	/// (like the ephemeral cache manager) to maintain a reference to the same storage
	/// the job uses, enabling direct access to indexing results without job-to-caller
	/// communication overhead.
	pub fn set_ephemeral_index(&mut self, index: Arc<RwLock<EphemeralIndex>>) {
		self.ephemeral_index = Some(index);
	}

	/// Sets the durable store the walk writes alongside the arena.
	///
	/// Absent for a partition with no identity to key a store on, in which
	/// case the walk fills the arena and nothing outlives the session.
	pub fn set_source_store(&mut self, store: Arc<crate::ops::indexing::ephemeral::SourceStore>) {
		self.source_store = Some(store);
	}

	pub fn ephemeral_browse(path: SdPath, scope: IndexScope, is_volume: bool) -> Self {
		Self::new(IndexerJobConfig::ephemeral_browse(path, scope, is_volume))
	}

	async fn run_current_scope_discovery_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		root_path: &std::path::Path,
	) -> JobResult<()> {
		use super::metadata;
		use super::state::{DirEntry, EntryKind};
		use tokio::fs;

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

			#[cfg(unix)]
			let (permissions, uid, gid) = {
				use std::os::unix::fs::MetadataExt;
				(
					Some(metadata.mode()),
					Some(metadata.uid()),
					Some(metadata.gid()),
				)
			};
			#[cfg(not(unix))]
			let (permissions, uid, gid) = (None, None, None);

			let link_target = if matches!(entry_kind, EntryKind::Symlink) {
				tokio::fs::read_link(&path)
					.await
					.ok()
					.map(|t| t.to_string_lossy().into_owned())
			} else {
				None
			};

			let dir_entry = DirEntry {
				path: path.clone(),
				kind: entry_kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				created: crate::ops::indexing::metadata::birth_time(&path, &metadata),
				accessed: metadata.accessed().ok(),
				inode: crate::ops::indexing::metadata::get_inode(&path, &metadata),
				permissions,
				uid,
				gid,
				link_target,
			};

			state.pending_entries.push(dir_entry);
			state.items_since_last_update += 1;

			match entry_kind {
				EntryKind::File => state.stats.files += 1,
				EntryKind::Directory => state.stats.dirs += 1,
				EntryKind::Symlink => state.stats.symlinks += 1,
			}
		}

		if !state.pending_entries.is_empty() {
			let batch = state.create_batch();
			state.entry_batches.push_back(batch);
		}

		state.phase = Phase::Processing;
		ctx.log(format!(
			"Current scope discovery complete: {} entries found",
			state.stats.files + state.stats.dirs
		));

		Ok(())
	}

	async fn run_ephemeral_processing_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		ephemeral_index: Arc<RwLock<EphemeralIndex>>,
		source_store: Option<Arc<crate::ops::indexing::ephemeral::SourceStore>>,
		root_path: &Path,
		_volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
		is_volume_indexing: bool,
	) -> JobResult<()> {
		use super::metadata::EntryMetadata;

		ctx.log("Starting ephemeral processing");

		// Mapping a drive is not something anyone asked to watch happen, and
		// eleven million notices would be its own denial of service. A walk of
		// a folder someone opened answers with one event per batch.
		let writer = ArenaWriter::new(
			ephemeral_index.clone(),
			ctx.library().event_bus().clone(),
			source_store.clone(),
		)
		.notifying(if is_volume_indexing {
			Notify::Silent
		} else {
			Notify::Batched
		});

		let total_batches = state.entry_batches.len();
		let mut batch_number = 0;

		while let Some(batch) = state.entry_batches.pop_front() {
			ctx.check_interrupt().await?;

			batch_number += 1;

			// Emit progress
			let indexer_progress = IndexerProgress {
				phase: IndexPhase::Processing {
					batch: batch_number,
					total_batches,
				},
				current_path: format!("Batch {}/{}", batch_number, total_batches),
				total_found: state.stats,
				processing_rate: state.calculate_rate(),
				estimated_remaining: state.estimate_remaining(),
				scope: None,
				is_ephemeral: false,
				action_context: None,
				volume_total_capacity: state.volume_total_capacity,
			};
			ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

			// One call, and everything that has to know does. Identity comes
			// from the store's ledger, which is where the batch is taken in;
			// the arena is written; clients hear one event for the batch, or
			// nothing at all when the walk is mapping a drive nobody asked to
			// see.
			writer
				.apply(Seen::Entries(
					batch.into_iter().map(EntryMetadata::from).collect(),
				))
				.await;
		}

		// The directories the walk turned back at are in the arena by now, so
		// the counts taken at the time can be attached to them. Last, because a
		// summary written before its own entry arrived would have nothing to
		// attach to, and one written before a child arrived would be undone by
		// it.
		if !state.summaries.is_empty() {
			let summaries = std::mem::take(&mut state.summaries);
			let summarised = summaries.len();
			for (path, bytes, files) in summaries {
				writer
					.apply(Seen::Counted {
						path,
						totals: Rollup { bytes, files },
					})
					.await;
			}
			ctx.log(format!("Summarised {summarised} directories"));
		}

		state.phase = Phase::Complete;

		ctx.log("Ephemeral processing complete");
		Ok(())
	}
}

/// Job output with comprehensive results
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::rules::RuleToggles;

	fn archival_walk() -> IndexerJobConfig {
		let mut config = IndexerJobConfig::ephemeral_browse(
			SdPath::local(std::path::PathBuf::from("/Volumes/Archive")),
			IndexScope::Recursive,
			true,
		);
		config.rule_toggles = RuleToggles::none();
		config
	}

	#[test]
	fn only_a_whole_unfiltered_walk_may_sweep() {
		assert!(archival_walk().enumerates_whole_source());
	}

	#[test]
	fn a_browse_may_not_sweep() {
		// Every directory listing dispatches one of these. A sweep after one
		// would read the rest of the source as deleted.
		let config = IndexerJobConfig::ephemeral_browse(
			SdPath::local(std::path::PathBuf::from("/Volumes/Archive/photos")),
			IndexScope::Current,
			false,
		);
		assert!(!config.enumerates_whole_source());
	}

	#[test]
	fn rules_bar_a_sweep() {
		// Rules hide files on purpose, so what the walk did not see is not
		// what is not there.
		let mut config = archival_walk();
		config.rule_toggles = RuleToggles::default();
		assert!(!config.enumerates_whole_source());

		config.rule_toggles = RuleToggles::none();
		config.rule_toggles.no_git = true;
		assert!(!config.enumerates_whole_source());
	}

	#[test]
	fn a_depth_limit_bars_a_sweep() {
		let mut config = archival_walk();
		config.max_depth = Some(3);
		assert!(!config.enumerates_whole_source());
	}
}
