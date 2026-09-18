//! Volume indexing action - map an entire volume into the volume index

use super::{map::MapOptions, map_volume, IndexVolumeInput, IndexVolumeOutput};
use crate::ops::indexing::summary::Retention;
use crate::{
	context::CoreContext,
	infra::{
		action::{context::ActionContext, error::ActionError, LibraryAction},
		job::types::JobPriority,
	},
	library::Library,
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
			"Starting indexing for volume: {} ({})",
			volume.name, fingerprint.0
		);

		// 2. Dispatch the walk. Doing it here rather than by hand keeps this on
		// the same path as the background map, which is the only way the two
		// stay in step on what is easy to forget: seeding from the snapshot,
		// and clearing what the new pass will not revisit.
		let action_context = ActionContext::new(
			"volumes.index",
			json!({
				"fingerprint": fingerprint.to_string(),
				"scope": format!("{:?}", self.input.scope),
			}),
			json!({
				"volume_fingerprint": fingerprint.to_string(),
				"volume_name": volume.name,
			}),
		);

		let job_id = map_volume(
			&library,
			&context,
			&volume,
			MapOptions {
				scope: self.input.scope,
				priority: JobPriority::NORMAL,
				reindex: true,
				// Asking for a drive to be indexed is asking for all of it,
				// including an accounting for what the rules hold back. The
				// background map is the one that trades detail for memory.
				retention: Retention::source(),
				announce: true,
			},
			Some(action_context),
		)
		.await?;

		info!(
			"Dispatched indexing job {} for volume {}",
			job_id, volume.name
		);

		// 3. Save the drive's totals once the walk reports them.
		let library_clone = library.clone();
		let context_clone = context.clone();
		let fingerprint_clone = fingerprint.clone();
		let mount_point_clone = volume.mount_point.clone();
		let volume_name = volume.name.clone();
		let job_id_str = job_id.to_string();

		tokio::spawn(async move {
			let mut event_rx = context_clone.events.subscribe();

			while let Ok(event) = event_rx.recv().await {
				match event {
					crate::infra::event::Event::JobCompleted {
						job_id: event_job_id,
						output,
						..
					} => {
						if event_job_id == job_id_str {
							// Extract stats from job output
							if let crate::infra::job::output::JobOutput::Indexed { stats, .. } =
								output
							{
								info!(
									"Volume indexing complete: {} files, {} directories",
									stats.files, stats.dirs
								);

								// Save stats to database
								if let Err(e) = Self::save_volume_stats_static(
									&library_clone,
									&fingerprint_clone,
									stats.files,
									stats.dirs,
								)
								.await
								{
									error!("Failed to save volume stats: {}", e);
								}

								// Mark as indexed and register for watching
								let volume_index = context_clone.volume_index();
								volume_index.mark_indexing_complete(&mount_point_clone);
								let _ =
									volume_index.register_for_watching(mount_point_clone.clone());

								// Emit Refresh so frontend invalidates directory listing cache
								context_clone
									.events
									.emit(crate::infra::event::Event::Refresh);
							}
							break;
						}
					}
					crate::infra::event::Event::JobFailed {
						job_id: event_job_id,
						error,
						..
					} => {
						if event_job_id == job_id_str {
							error!("Volume indexing job failed: {}", error);
							break;
						}
					}
					_ => {}
				}
			}
		});

		Ok(IndexVolumeOutput {
			volume_id: volume.id,
			job_id: job_id.into(),
			total_files: None,
			total_directories: None,
			message: format!("Indexing volume '{}' (job {})", volume_name, job_id),
		})
	}

	fn action_kind(&self) -> &'static str {
		"volumes.index"
	}
}

impl IndexVolumeAction {
	/// Save volume indexing stats to database and trigger sync
	async fn save_volume_stats_static(
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
