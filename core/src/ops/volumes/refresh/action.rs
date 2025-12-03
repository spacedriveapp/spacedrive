//! Volume refresh action
//!
//! This action recalculates unique_bytes for all volumes owned by this device
//! and emits a Refresh event to invalidate all frontend caches.

use super::{VolumeRefreshInput, VolumeRefreshOutput};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities,
		event::Event,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeRefreshAction {
	input: VolumeRefreshInput,
}

impl VolumeRefreshAction {
	pub fn new(input: VolumeRefreshInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for VolumeRefreshAction {
	type Input = VolumeRefreshInput;
	type Output = VolumeRefreshOutput;

	fn from_input(input: VolumeRefreshInput) -> Result<Self, String> {
		Ok(VolumeRefreshAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		info!(
			"Starting volume refresh for library {} (force: {})",
			library.name().await,
			self.input.force
		);

		// Get all tracked volumes owned by this device
		let db = library.db().conn();
		let device_id = context.volume_manager.device_id;
		let owned_volumes = entities::volume::Entity::find()
			.filter(entities::volume::Column::DeviceId.eq(device_id))
			.all(db)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to query volumes: {}", e)))?;

		let total_volumes = owned_volumes.len();
		let mut refreshed = 0;
		let mut failed = 0;

		info!("Found {} volumes to refresh", total_volumes);

		// Calculate unique bytes for each owned volume
		for volume in owned_volumes {
			let fingerprint = crate::volume::VolumeFingerprint(volume.fingerprint.clone());

			match context
				.volume_manager
				.calculate_and_save_unique_bytes(&fingerprint, &[library.clone()])
				.await
			{
				Ok(_) => refreshed += 1,
				Err(e) => {
					tracing::warn!(
						"Failed to calculate unique_bytes for volume {}: {}",
						fingerprint.0,
						e
					);
					failed += 1;
				}
			}
		}

		info!(
			"Volume refresh complete: {} succeeded, {} failed",
			refreshed, failed
		);

		// Emit Refresh event to invalidate all frontend caches
		context.events.emit(Event::Refresh);

		Ok(VolumeRefreshOutput::new(refreshed, failed))
	}

	fn action_kind(&self) -> &'static str {
		"volumes.refresh"
	}
}

// Register action
crate::register_library_action!(VolumeRefreshAction, "volumes.refresh");
