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

// Re-export IndexMode from domain for backwards compatibility
pub use crate::domain::location::IndexMode;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use super::{
	ephemeral::EphemeralIndex,
	metrics::{IndexerMetrics, PhaseTimer},
	phases,
	state::{IndexError, IndexPhase, IndexerProgress, IndexerState, IndexerStats, Phase},
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

/// Whether to write indexing results to the database or keep them in memory.
///
/// Ephemeral persistence allows users to browse external drives and network shares
/// without adding them as managed locations. The in-memory index survives for the
/// session duration and provides the same API surface as persistent entries, enabling
/// features like search and navigation to work identically for both modes. If an
/// ephemeral path is later promoted to a managed location, UUIDs are preserved to
/// maintain continuity for user metadata.
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

/// Configuration for an indexer job, supporting both persistent and ephemeral indexing.
///
/// Persistent jobs require a location_id to identify which managed location they're
/// indexing. Ephemeral jobs (browsing unmanaged paths) use location_id = None and
/// store results in memory instead of the database.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerJobConfig {
	pub location_id: Option<Uuid>,
	pub path: SdPath,
	pub mode: IndexMode,
	pub scope: IndexScope,
	pub persistence: IndexPersistence,
	pub max_depth: Option<u32>,
	#[serde(default)]
	pub rule_toggles: super::rules::RuleToggles,
	/// Whether to run this job in the background (not persisted to database, no UI updates)
	#[serde(default)]
	pub run_in_background: bool,
}

impl IndexerJobConfig {
	pub fn new(location_id: Uuid, path: SdPath, mode: IndexMode) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode,
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
			run_in_background: false,
		}
	}

	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self {
			location_id: Some(location_id),
			path,
			mode: IndexMode::Shallow,
			scope: IndexScope::Current,
			persistence: IndexPersistence::Persistent,
			max_depth: Some(1),
			rule_toggles: Default::default(),
			run_in_background: false,
		}
	}

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
			run_in_background: false,
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

/// Orchestrates multi-phase file indexing for both persistent and ephemeral modes.
///
/// The job executes as a state machine progressing through Discovery, Processing,
/// Aggregation, and ContentIdentification phases. State is automatically serialized
/// between phases, allowing the job to survive app restarts and resume from the last
/// completed phase. Ephemeral jobs (browsing unmanaged paths) skip aggregation and
/// content identification, storing results in memory via `EphemeralIndex`.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IndexerJob {
	pub config: IndexerJobConfig,
	state: Option<IndexerState>,
	#[serde(skip)]
	ephemeral_index: Option<Arc<RwLock<EphemeralIndex>>>,
	#[serde(skip)]
	timer: Option<PhaseTimer>,
	#[serde(skip)]
	db_operations: (u64, u64),
	#[serde(skip)]
	batch_info: (u64, usize),
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

	fn should_persist(&self) -> bool {
		!self.config.is_ephemeral() && !self.config.run_in_background
	}
}

impl JobProgress for IndexerProgress {}

impl IndexerJob {
	async fn run_job_phases(&mut self, ctx: &JobContext<'_>) -> JobResult<IndexerOutput> {
		if self.state.is_none() {
			ctx.log(format!(
				"Starting new indexer job (scope: {}, persistence: {:?})",
				self.config.scope, self.config.persistence
			));
			info!("INDEXER_STATE: Job starting with NO saved state - creating new state");
			self.state = Some(IndexerState::new(&self.config.path));
		} else {
			ctx.log("Resuming indexer from saved state");
			info!("INDEXER_STATE: Job resuming with saved state - phase: {:?}, entry_batches: {}, entries_for_content: {}, seen_paths: {}",
				self.state.as_ref().unwrap().phase,
				self.state.as_ref().unwrap().entry_batches.len(),
				self.state.as_ref().unwrap().entries_for_content.len(),
				self.state.as_ref().unwrap().seen_paths.len());
		}

		let state = self.state.as_mut().unwrap();

		let root_path_buf = if let Some(p) = self.config.path.as_local_path() {
			p.to_path_buf()
		} else if let Some(cloud_path) = self.config.path.cloud_path() {
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
					if self.config.is_ephemeral() {
						let ephemeral_index = self.ephemeral_index.clone().ok_or_else(|| {
							JobError::execution("Ephemeral index not initialized".to_string())
						})?;
						Self::run_ephemeral_processing_static(
							state,
							&ctx,
							ephemeral_index,
							root_path,
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

						self.db_operations.1 += state.entry_batches.len() as u64 * 100;
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
						ctx.log("Skipping aggregation and content phases for ephemeral job (content kind identified by extension)");
						state.phase = Phase::Complete;
						continue;
					}

					if let Some(timer) = &mut self.timer {
						timer.start_content();
					}
				}

				Phase::ContentIdentification => {
					if self.config.mode >= IndexMode::Content {
						if self.config.is_ephemeral() {
							ctx.log("Skipping content identification for ephemeral job");
							state.phase = Phase::Complete;
							continue;
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
			persistence: None,
			is_ephemeral: false,
			action_context: None,
		};
		ctx.progress(Progress::generic(final_progress.to_generic_progress()));

		let metrics = if let Some(timer) = &self.timer {
			IndexerMetrics::calculate(&state.stats, timer, self.db_operations, self.batch_info)
		} else {
			IndexerMetrics::default()
		};

		ctx.log(&metrics.format_summary());

		#[cfg(feature = "ffmpeg")]
		if self.config.mode == IndexMode::Deep && !self.config.is_ephemeral() {
			use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

			ctx.log("Deep mode enabled - dispatching thumbnail generation job");

			// Query entry UUIDs for this location to avoid processing all database entries
			let entry_uuids = if let Some(location_id) = self.config.location_id {
				use crate::infra::db::entities::{entry, location};

				// Find the location's entry_id (root entry)
				let db = ctx.library_db();
				let location_record = location::Entity::find()
					.filter(location::Column::Uuid.eq(location_id))
					.one(db)
					.await;

				match location_record {
					Ok(Some(loc)) => {
						if let Some(root_entry_id) = loc.entry_id {
							// Query all entry IDs that are descendants of this location's root entry
							// using the entry_closure table
							let entry_ids_result: Result<Vec<i32>, _> = db
								.query_all(Statement::from_sql_and_values(
									sea_orm::DbBackend::Sqlite,
									"SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?",
									vec![root_entry_id.into()],
								))
								.await
								.map(|rows| {
									rows.iter()
										.filter_map(|row| row.try_get_by_index::<i32>(0).ok())
										.collect()
								});

							match entry_ids_result {
								Ok(entry_ids) => {
									if entry_ids.is_empty() {
										ctx.log(
											"No entries found in location for thumbnail generation",
										);
										None
									} else {
										// Now get the UUIDs for these entry IDs
										let entries_result = entry::Entity::find()
											.filter(entry::Column::Id.is_in(entry_ids))
											.all(db)
											.await;

										match entries_result {
											Ok(entry_models) => {
												let uuids: Vec<Uuid> = entry_models
													.into_iter()
													.filter_map(|e| e.uuid)
													.collect();

												if !uuids.is_empty() {
													ctx.log(format!(
														"Found {} entries in location {} for thumbnail generation",
														uuids.len(),
														location_id
													));
													Some(uuids)
												} else {
													ctx.log("No entry UUIDs found in location for thumbnail generation");
													None
												}
											}
											Err(e) => {
												ctx.log(format!(
													"Warning: Failed to query entry UUIDs for location: {}",
													e
												));
												None
											}
										}
									}
								}
								Err(e) => {
									ctx.log(format!(
										"Warning: Failed to query entry closure for location: {}",
										e
									));
									None
								}
							}
						} else {
							ctx.log("Location has no root entry, skipping thumbnail generation");
							None
						}
					}
					Ok(None) => {
						ctx.log(format!(
							"Warning: Location {} not found, dispatching thumbnail job for all entries",
							location_id
						));
						None
					}
					Err(e) => {
						ctx.log(format!(
							"Warning: Failed to query location: {}, dispatching thumbnail job for all entries",
							e
						));
						None
					}
				}
			} else {
				ctx.log("No location_id in config, dispatching thumbnail job for all entries");
				None
			};

			let mut thumbnail_config = ThumbnailJobConfig::default();
			// Inherit background flag from the indexer job
			thumbnail_config.run_in_background = self.config.run_in_background;

			let thumbnail_job = if let Some(uuids) = entry_uuids {
				ThumbnailJob::for_entries(uuids, thumbnail_config)
			} else {
				ThumbnailJob::new(thumbnail_config)
			};

			match ctx.library().jobs().dispatch(thumbnail_job).await {
				Ok(_handle) => {
					ctx.log("Successfully dispatched thumbnail generation job");
				}
				Err(e) => {
					ctx.log(format!("Warning: Failed to dispatch thumbnail job: {}", e));
				}
			}
		}

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
}

// JobHandler trait implementation
#[async_trait::async_trait]
impl JobHandler for IndexerJob {
	type Output = IndexerOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if self.timer.is_none() {
			self.timer = Some(PhaseTimer::new());
		}

		if self.config.is_ephemeral() && self.ephemeral_index.is_none() {
			let index = EphemeralIndex::new()
				.map_err(|e| JobError::Other(format!("Failed to create ephemeral index: {}", e)))?;
			self.ephemeral_index = Some(Arc::new(RwLock::new(index)));
			ctx.log("Initialized ephemeral index for non-persistent job");
		}

		let result = self.run_job_phases(&ctx).await;

		// Mark ephemeral indexing complete even on failure to prevent the indexing
		// flag from being stuck forever. Without this, a failed ephemeral job would
		// block all future indexing attempts for that path until app restart.
		if self.config.is_ephemeral() {
			if let Some(local_path) = self.config.path.as_local_path() {
				ctx.library()
					.core_context()
					.ephemeral_cache()
					.mark_indexing_complete(local_path);
				match &result {
					Ok(_) => {
						ctx.log(format!(
							"Marked ephemeral indexing complete for: {}",
							local_path.display()
						));

						// Automatically add filesystem watch for successfully indexed ephemeral paths
						// This enables real-time updates when files change in browsed directories
						if let Some(watcher) = ctx.library().core_context().get_fs_watcher().await {
							if let Err(e) = watcher.watch_ephemeral(local_path.to_path_buf()).await
							{
								ctx.log(format!(
									"Warning: Failed to add ephemeral watch for {}: {}",
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
						"Marked ephemeral indexing complete (job failed: {}) for: {}",
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
			timer: None,
			db_operations: (0, 0),
			batch_info: (0, 0),
		}
	}

	pub fn from_location(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
		Self::new(IndexerJobConfig::new(location_id, root_path, mode))
	}

	pub fn shallow(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Shallow)
	}

	pub fn with_content(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Content)
	}

	pub fn deep(location_id: Uuid, root_path: SdPath) -> Self {
		Self::from_location(location_id, root_path, IndexMode::Deep)
	}

	pub fn ui_navigation(location_id: Uuid, path: SdPath) -> Self {
		Self::new(IndexerJobConfig::ui_navigation(location_id, path))
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

	pub fn ephemeral_browse(path: SdPath, scope: IndexScope) -> Self {
		Self::new(IndexerJobConfig::ephemeral_browse(path, scope))
	}

	async fn run_current_scope_discovery_static(
		state: &mut IndexerState,
		ctx: &JobContext<'_>,
		root_path: &std::path::Path,
	) -> JobResult<()> {
		use super::database_storage::DatabaseStorage;
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

			let dir_entry = DirEntry {
				path: path.clone(),
				kind: entry_kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				inode: DatabaseStorage::get_inode(&metadata),
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
			state.entry_batches.push(batch);
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
		root_path: &Path,
		_volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
	) -> JobResult<()> {
		use super::persistence::PersistenceFactory;

		ctx.log("Starting ephemeral processing");

		let event_bus = Some(ctx.library().event_bus().clone());

		let persistence = PersistenceFactory::ephemeral(
			ephemeral_index.clone(),
			event_bus,
			root_path.to_path_buf(),
		);

		while let Some(batch) = state.entry_batches.pop() {
			for entry in batch {
				let _entry_id = persistence.store_entry(&entry, None, root_path).await?;
			}
		}

		state.phase = Phase::Complete;

		ctx.log("Ephemeral processing complete");
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
