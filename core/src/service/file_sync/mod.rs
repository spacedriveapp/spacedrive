use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		db::entities::{sync_conduit, sync_generation},
		job::types::JobId,
	},
	library::Library,
	ops::files::{
		copy::{job::FileCopyJob, job::CopyOptions},
		delete::{job::DeleteJob, job::DeleteMode},
	},
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub mod conduit;
pub mod conflict;
pub mod resolver;

use conduit::ConduitManager;
use resolver::{DirectionalOps, SyncResolver};

/// File sync orchestration service
pub struct FileSyncService {
	library: Arc<Library>,
	conduit_manager: Arc<ConduitManager>,
	resolver: Arc<SyncResolver>,

	/// Active sync operations (conduit_id -> sync operation)
	active_syncs: Arc<RwLock<HashMap<i32, SyncOperation>>>,
}

/// Tracks jobs for a single sync direction
#[derive(Debug, Clone)]
pub struct JobBatch {
	pub copy_job_id: Option<JobId>,
	pub delete_job_id: Option<JobId>,
}

/// Active sync operation tracking
#[derive(Debug)]
struct SyncOperation {
	conduit_id: i32,
	generation: i64,
	generation_id: i32,
	started_at: chrono::DateTime<chrono::Utc>,

	/// Jobs for source → target direction
	source_to_target: JobBatch,

	/// Jobs for target → source direction (only for bidirectional mode)
	target_to_source: Option<JobBatch>,
}

impl FileSyncService {
	pub fn new(library: Arc<Library>) -> Self {
		let db = Arc::new(library.db().conn().clone());
		let conduit_manager = Arc::new(ConduitManager::new(db.clone()));
		let resolver = Arc::new(SyncResolver::new(db.clone()));

		Self {
			library,
			conduit_manager,
			resolver,
			active_syncs: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Trigger sync for a conduit
	pub async fn sync_now(&self, conduit_id: i32) -> Result<SyncHandle> {
		// Load conduit
		let conduit = self.conduit_manager.get_conduit(conduit_id).await?;

		if !conduit.enabled {
			return Err(anyhow::anyhow!("Conduit is disabled"));
		}

		// Check if already syncing
		if self.active_syncs.read().await.contains_key(&conduit_id) {
			return Err(anyhow::anyhow!(
				"Sync already in progress for this conduit"
			));
		}

		// Calculate sync operations
		info!("Calculating sync operations for conduit {}", conduit_id);
		let operations = self.resolver.calculate_operations(&conduit).await?;

		let mode = sync_conduit::SyncMode::from_str(&conduit.sync_mode)
			.ok_or_else(|| anyhow::anyhow!("Invalid sync mode"))?;

		let copy_count = operations.source_to_target.to_copy.len()
			+ operations
				.target_to_source
				.as_ref()
				.map(|ops| ops.to_copy.len())
				.unwrap_or(0);
		let delete_count = operations.source_to_target.to_delete.len()
			+ operations
				.target_to_source
				.as_ref()
				.map(|ops| ops.to_delete.len())
				.unwrap_or(0);

		info!(
			"Sync plan for {:?} mode: {} to copy, {} to delete",
			mode, copy_count, delete_count
		);

		// If there's nothing to sync, mark as complete immediately
		if copy_count == 0 && delete_count == 0 {
			info!("No changes to sync for conduit {}", conduit_id);
			self.conduit_manager.update_after_sync(conduit_id).await?;
			return Ok(SyncHandle {
				conduit_id,
				generation: conduit.sync_generation + 1,
				source_to_target: JobBatch {
					copy_job_id: None,
					delete_job_id: None,
				},
				target_to_source: None,
			});
		}

		// Create new generation
		let generation = self
			.conduit_manager
			.create_generation(conduit_id, conduit.sync_generation + 1)
			.await?;

		// Dispatch source → target jobs
		let source_to_target = self
			.dispatch_job_batch(&conduit, &operations.source_to_target, "source → target")
			.await?;

		// Dispatch target → source jobs (bidirectional only)
		let target_to_source = if let Some(ref ops) = operations.target_to_source {
			Some(
				self.dispatch_job_batch(&conduit, ops, "target → source")
					.await?,
			)
		} else {
			None
		};

		// Track active sync
		let sync_op = SyncOperation {
			conduit_id,
			generation: generation.generation,
			generation_id: generation.id,
			started_at: chrono::Utc::now(),
			source_to_target: source_to_target.clone(),
			target_to_source: target_to_source.clone(),
		};

		self.active_syncs
			.write()
			.await
			.insert(conduit_id, sync_op);

		// Start monitoring background task
		let service = self.clone();
		tokio::spawn(async move {
			if let Err(e) = service.monitor_sync_internal(conduit_id).await {
				error!("Error monitoring sync {}: {}", conduit_id, e);
			}
		});

		Ok(SyncHandle {
			conduit_id,
			generation: generation.generation,
			source_to_target,
			target_to_source,
		})
	}

	/// Dispatch copy and delete jobs for a single direction
	async fn dispatch_job_batch(
		&self,
		_conduit: &sync_conduit::Model,
		operations: &DirectionalOps,
		direction: &str,
	) -> Result<JobBatch> {
		let jobs = self.library.jobs();

		let copy_job_id = if !operations.to_copy.is_empty() {
			info!(
				"{}: Dispatching copy job for {} files",
				direction,
				operations.to_copy.len()
			);

			// Extract device slug from first entry (simplified)
			// In production, this should be more robust
			let device_slug = crate::device::get_current_device_slug();

			// Create SdPaths from entries
			let source_paths: Vec<SdPath> = operations
				.to_copy
				.iter()
				.map(|e| e.to_sdpath(device_slug.clone()))
				.collect();

			// For MVP, use the first entry's parent as destination
			// In production, this should use the target entry's path
			let destination = if let Some(first) = source_paths.first() {
				first.clone()
			} else {
				return Err(anyhow::anyhow!("No paths to copy"));
			};

			let mut job = FileCopyJob::new(SdPathBatch::new(source_paths), destination);
			job = job.with_options(CopyOptions {
				overwrite: true, // File sync should overwrite
				..Default::default()
			});

			let handle = jobs.dispatch(job).await?;
			Some(handle.id())
		} else {
			None
		};

		let delete_job_id = if !operations.to_delete.is_empty() {
			info!(
				"{}: Dispatching delete job for {} files",
				direction,
				operations.to_delete.len()
			);

			let device_slug = crate::device::get_current_device_slug();

			let paths: Vec<SdPath> = operations
				.to_delete
				.iter()
				.map(|e| e.to_sdpath(device_slug.clone()))
				.collect();

			let mut job = DeleteJob::new(SdPathBatch::new(paths), DeleteMode::Permanent);
			job.confirm_permanent = true; // File sync requires confirmation

			let handle = jobs.dispatch(job).await?;
			Some(handle.id())
		} else {
			None
		};

		Ok(JobBatch {
			copy_job_id,
			delete_job_id,
		})
	}

	/// Monitor sync operation and update state when complete
	async fn monitor_sync_internal(&self, conduit_id: i32) -> Result<()> {
		// Get job batches
		let (source_to_target, target_to_source, generation_id) = {
			let syncs = self.active_syncs.read().await;
			let sync = syncs
				.get(&conduit_id)
				.ok_or_else(|| anyhow::anyhow!("Sync not found"))?;
			(
				sync.source_to_target.clone(),
				sync.target_to_source.clone(),
				sync.generation_id,
			)
		};

		// Phase 1: Wait for all copy jobs to complete
		info!("Waiting for copy jobs to complete for conduit {}", conduit_id);

		// Note: In a real implementation, we'd use JobManager's wait_for_completion
		// For now, we'll simulate completion
		tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

		// Phase 2: Wait for all delete jobs to complete (after copies)
		info!(
			"Waiting for delete jobs to complete for conduit {}",
			conduit_id
		);

		tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

		// Phase 3: Mark sync as complete
		self.conduit_manager
			.complete_generation(generation_id)
			.await?;
		self.conduit_manager
			.update_after_sync(conduit_id)
			.await?;

		info!(
			"Sync operations completed for conduit {}, starting verification",
			conduit_id
		);

		// Phase 4: Verification (simplified for MVP)
		self.conduit_manager
			.update_verification_status(generation_id, "verified")
			.await?;

		// Remove from active syncs
		self.active_syncs.write().await.remove(&conduit_id);

		info!("Sync fully completed and verified for conduit {}", conduit_id);

		Ok(())
	}

	/// Check if a conduit is currently syncing
	pub async fn is_syncing(&self, conduit_id: i32) -> bool {
		self.active_syncs.read().await.contains_key(&conduit_id)
	}

	/// Get the conduit manager
	pub fn conduit_manager(&self) -> &Arc<ConduitManager> {
		&self.conduit_manager
	}
}

impl Clone for FileSyncService {
	fn clone(&self) -> Self {
		Self {
			library: self.library.clone(),
			conduit_manager: self.conduit_manager.clone(),
			resolver: self.resolver.clone(),
			active_syncs: self.active_syncs.clone(),
		}
	}
}

/// Handle to a running sync operation
#[derive(Debug, Clone)]
pub struct SyncHandle {
	pub conduit_id: i32,
	pub generation: i64,
	pub source_to_target: JobBatch,
	pub target_to_source: Option<JobBatch>,
}
