//! Volume indexing action - ephemeral index entire volumes

use super::{IndexVolumeInput, IndexVolumeOutput};
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::indexing::job::{IndexerJob, IndexerJobConfig},
	volume::VolumeFingerprint,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexVolumeAction {
	input: IndexVolumeInput,
}

impl IndexVolumeAction {
	pub fn new(input: IndexVolumeInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for IndexVolumeAction {
	type Input = IndexVolumeInput;
	type Output = IndexVolumeOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(IndexVolumeAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// 1. Parse fingerprint and find volume
		let fingerprint = VolumeFingerprint(self.input.fingerprint.clone());

		let volume = context
			.volume_manager
			.get_volume(&fingerprint)
			.await
			.ok_or_else(|| ActionError::Internal(format!("Volume not found: {}", fingerprint.0)))?;

		info!(
			"Starting ephemeral indexing for volume: {} ({})",
			volume.name, fingerprint.0
		);

		// 2. Get device info for SdPath construction
		let device_uuid = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("Failed to get device ID: {}", e)))?;

		// Get device slug from database
		let db = library.db().conn();
		let device_record = crate::infra::db::entities::device::Entity::find()
			.filter(crate::infra::db::entities::device::Column::Uuid.eq(device_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal(format!("Device not found: {}", device_uuid)))?;

		// 3. Construct SdPath for the volume's mount point
		let sd_path = if let Some((service, identifier)) = volume.parse_cloud_identity() {
			// Cloud volume
			SdPath::Cloud {
				service,
				identifier,
				path: String::new(), // Root of cloud volume
			}
		} else {
			// Local volume - use mount point
			SdPath::Physical {
				device_slug: device_record.slug,
				path: volume.mount_point.clone(),
			}
		};

		// 4. Create ephemeral indexing job
		let indexer_config = IndexerJobConfig::ephemeral_browse(sd_path, self.input.scope);
		let mut indexer_job = IndexerJob::new(indexer_config);

		// 5. Get ephemeral cache and create/reuse index for this volume
		let ephemeral_cache = context.ephemeral_cache();
		let index = ephemeral_cache.create_for_indexing(volume.mount_point.clone());
		indexer_job.set_ephemeral_index(index.clone());

		// 6. Clear stale entries if this volume was previously indexed
		let cleared = ephemeral_cache.clear_for_reindex(&volume.mount_point).await;
		if cleared > 0 {
			info!(
				"Cleared {} stale entries before re-indexing volume",
				cleared
			);
		}

		// 7. Dispatch job
		let job_handle = library
			.jobs()
			.dispatch(indexer_job)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to dispatch job: {}", e)))?;

		let job_id = job_handle.id();
		info!(
			"Dispatched ephemeral indexing job {} for volume {}",
			job_id, volume.name
		);

		// 8. Wait for job completion to extract stats
		let output = match job_handle.wait().await {
			Ok(output) => output,
			Err(e) => {
				error!("Ephemeral indexing job failed: {}", e);
				return Err(ActionError::Internal(format!("Indexing failed: {}", e)));
			}
		};

		// 9. Extract stats from job output
		let (file_count, dir_count) = match output {
			crate::infra::job::output::JobOutput::Indexed { stats, .. } => {
				(stats.files, stats.dirs)
			}
			_ => {
				return Err(ActionError::Internal(
					"Expected Indexed output from indexer job".to_string(),
				));
			}
		};

		info!(
			"Volume indexing complete: {} files, {} directories",
			file_count, dir_count
		);

		// 10. Save stats to database
		self.save_volume_stats(&library, &fingerprint, file_count, dir_count)
			.await?;

		// 11. Mark as indexed and register for watching
		ephemeral_cache.mark_indexing_complete(&volume.mount_point);
		let _ = ephemeral_cache.register_for_watching(volume.mount_point.clone());

		Ok(IndexVolumeOutput {
			volume_id: volume.id,
			job_id: job_id.into(),
			total_files: Some(file_count),
			total_directories: Some(dir_count),
			message: format!(
				"Indexed {} files and {} directories on volume '{}'",
				file_count, dir_count, volume.name
			),
		})
	}

	fn action_kind(&self) -> &'static str {
		"volumes.index"
	}
}

impl IndexVolumeAction {
	/// Save volume indexing stats to database and trigger sync
	async fn save_volume_stats(
		&self,
		library: &Library,
		fingerprint: &VolumeFingerprint,
		file_count: u64,
		dir_count: u64,
	) -> Result<(), ActionError> {
		use crate::infra::db::entities;
		use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

		let db = library.db().conn();
		let now = chrono::Utc::now();

		// Update volume stats
		let update_result = entities::volume::Entity::update_many()
			.filter(entities::volume::Column::Fingerprint.eq(&fingerprint.0))
			.set(entities::volume::ActiveModel {
				total_file_count: Set(Some(file_count as i64)),
				total_directory_count: Set(Some(dir_count as i64)),
				last_indexed_at: Set(Some(now.into())),
				..Default::default()
			})
			.exec(db)
			.await
			.map_err(ActionError::SeaOrm)?;

		if update_result.rows_affected == 0 {
			return Err(ActionError::Internal(
				"Volume not found in database".to_string(),
			));
		}

		info!(
			"Saved volume stats to database: {} files, {} dirs (will sync to other devices)",
			file_count, dir_count
		);

		Ok(())
	}
}
