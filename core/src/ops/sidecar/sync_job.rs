use crate::{
	domain::{addressing::SdPath, ResourceManager, SdPathBatch},
	infra::{
		db::entities::sidecar_availability,
		job::prelude::*,
	},
	ops::files::copy::strategy::CopyStrategy,
	service::sidecar_sync::{
		SidecarSyncCoordinator, SidecarSyncFilters, SidecarSyncMode, SidecarTransferPlan,
	},
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
use tracing::{debug, error, info};

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct SidecarSyncJob {
	pub filters: SidecarSyncFilters,
	pub mode: SidecarSyncMode,
	pub completed_indices: Vec<usize>,
}

impl Job for SidecarSyncJob {
	const NAME: &'static str = "sidecar_sync";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Sync sidecar files across devices");
}

impl crate::infra::job::traits::DynJob for SidecarSyncJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SidecarSyncOutput {
	pub discovered: usize,
	pub transferred: usize,
	pub failed: usize,
	pub total_bytes: u64,
	pub duration: Duration,
}

impl From<SidecarSyncOutput> for crate::infra::job::output::JobOutput {
	fn from(output: SidecarSyncOutput) -> Self {
		Self::SidecarSync {
			discovered: output.discovered,
			transferred: output.transferred,
			failed: output.failed,
			total_bytes: output.total_bytes,
		}
	}
}

#[async_trait::async_trait]
impl JobHandler for SidecarSyncJob {
	type Output = SidecarSyncOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let started_at = Instant::now();

		// Phase 1: Discovery
		ctx.progress(Progress::indeterminate("Discovering missing sidecars"));

		let networking = ctx
			.networking_service()
			.ok_or_else(|| JobError::execution("Networking service not available"))?;

		let sidecar_manager = ctx
			.sidecar_manager()
			.await
			.ok_or_else(|| JobError::execution("Sidecar manager not available"))?;

		let coordinator = SidecarSyncCoordinator::new(
			ctx.library_arc(),
			networking.clone(),
			sidecar_manager.clone(),
		);

		let missing = coordinator
			.discover_missing_sidecars(self.filters.clone())
			.await
			.map_err(|e| JobError::execution(format!("Discovery failed: {}", e)))?;

		info!("Discovered {} missing sidecars", missing.len());
		ctx.progress(Progress::count(missing.len(), missing.len()));

		if missing.is_empty() {
			return Ok(SidecarSyncOutput {
				discovered: 0,
				transferred: 0,
				failed: 0,
				total_bytes: 0,
				duration: started_at.elapsed(),
			});
		}

		// Phase 2: Query Availability
		ctx.progress(Progress::indeterminate("Planning transfers"));

		let sources = coordinator
			.query_remote_availability(&missing)
			.await
			.map_err(|e| JobError::execution(format!("Availability query failed: {}", e)))?;

		debug!("Found sources for {} sidecars", sources.len());
		ctx.progress(Progress::indeterminate(format!("Found sources for {} sidecars", sources.len())));

		// Phase 3: Plan Transfers
		let plans = coordinator.plan_transfers(missing.clone(), sources);

		info!("Planned {} transfers", plans.len());

		if plans.is_empty() {
			info!("No online sources available for any sidecars");
			return Ok(SidecarSyncOutput {
				discovered: missing.len(),
				transferred: 0,
				failed: missing.len(),
				total_bytes: 0,
				duration: started_at.elapsed(),
			});
		}

		// Phase 4: Prepare destination
		ctx.progress(Progress::indeterminate("Preparing transfers"));

		use crate::device::get_current_device_slug;
		let destination_base = ctx.library().path().join("sidecars");
		tokio::fs::create_dir_all(&destination_base)
			.await
			.map_err(|e| JobError::execution(format!("Failed to create sidecars directory: {}", e)))?;

		// Phase 5: Execute Transfers using RemoteTransferStrategy directly
		ctx.progress(Progress::count(0, plans.len()));

		use crate::ops::files::copy::strategy::RemoteTransferStrategy;
		let strategy = RemoteTransferStrategy;

		let mut transferred = 0;
		let mut failed = 0;
		let mut total_bytes = 0u64;

		use crate::device::get_current_device_id;
		let db = ctx.library().db();
		let device_uuid = get_current_device_id();

		for (idx, plan) in plans.iter().enumerate() {
			// Skip if already completed
			if self.completed_indices.contains(&idx) {
				continue;
			}

			// Build source and destination paths
			let source = SdPath::Sidecar {
				content_id: plan.sidecar.content_uuid,
				kind: plan.sidecar.kind.clone(),
				variant: plan.sidecar.variant.clone(),
				format: plan.sidecar.format.clone(),
			};

			// Compute destination filename
			let filename = format!(
				"{}_{}_{}.{}",
				plan.sidecar.content_uuid,
				plan.sidecar.kind,
				plan.sidecar.variant,
				plan.sidecar.format
			);

			let destination = SdPath::Physical {
				device_slug: get_current_device_slug(),
				path: destination_base.join(&filename),
			};

			// Execute transfer
			match strategy.execute(&ctx, &source, &destination, true, None).await {
				Ok(bytes) => {
					total_bytes += bytes;
					transferred += 1;

					// Update availability record
					if let Err(e) = sidecar_availability::Entity::update_or_insert(
						db.conn(),
						&plan.sidecar.content_uuid,
						plan.sidecar.kind.as_str(),
						plan.sidecar.variant.as_str(),
						&device_uuid,
						true, // has = true
					)
					.await
					{
						error!("Failed to update availability for sidecar {}: {}", plan.sidecar.sidecar_uuid, e);
					}

					// Mark as completed
					self.completed_indices.push(idx);
				}
				Err(e) => {
					error!("Failed to transfer sidecar {}: {}", plan.sidecar.sidecar_uuid, e);
					failed += 1;
				}
			}

			// Update progress
			ctx.progress(Progress::count(transferred + failed, plans.len()));
		}

		// Phase 6: Emit Resource Events
		ctx.progress(Progress::indeterminate("Emitting events"));

		let resource_manager = ResourceManager::new(
			Arc::new(db.conn().clone()),
			ctx.library().event_bus().clone(),
		);

		// Collect sidecar UUIDs for successfully transferred items
		let sidecar_uuids: Vec<_> = plans
			.iter()
			.enumerate()
			.filter(|(idx, _)| self.completed_indices.contains(idx))
			.map(|(_, p)| p.sidecar.sidecar_uuid)
			.collect();

		if !sidecar_uuids.is_empty() {
			resource_manager
				.emit_resource_events("sidecar", sidecar_uuids)
				.await
				.map_err(|e| JobError::execution(format!("Failed to emit events: {}", e)))?;
		}

		info!(
			"Sync complete: {} transferred, {} failed",
			transferred, failed
		);

		Ok(SidecarSyncOutput {
			discovered: missing.len(),
			transferred,
			failed,
			total_bytes,
			duration: started_at.elapsed(),
		})
	}
}
