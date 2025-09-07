//! Track volume action
//!
//! This action tracks a volume within a library, allowing Spacedrive to monitor
//! and index files on the volume.

use crate::{
	context::CoreContext,
	infra::action::{
		error::{ActionError, ActionResult},
		handler::ActionHandler,
		output::ActionOutput,
		Action,
	},
	register_action_handler,
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Input for tracking a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackAction {
	/// The fingerprint of the volume to track
	pub fingerprint: VolumeFingerprint,

	/// The library ID to track the volume in
	pub library_id: Uuid,

	/// Optional name for the tracked volume
	pub name: Option<String>,
}

impl VolumeTrackAction {
	/// Execute the volume tracking action
	pub async fn execute(&self, core: &crate::Core) -> Result<ActionOutput, ActionError> {
		// Get the library
		let library = core
			.libraries
			.get_library(self.library_id)
			.await
			.ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

		// Check if volume exists
		let volume = core
			.volumes
			.get_volume(&self.fingerprint)
			.await
			.ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;

		if !volume.is_mounted {
			return Err(ActionError::InvalidInput(
				"Cannot track unmounted volume".to_string(),
			));
		}

		// Track the volume in the database
		let tracked = core
			.volumes
			.track_volume(&library, &self.fingerprint, self.name.clone())
			.await
			.map_err(|e| match e {
				crate::volume::VolumeError::AlreadyTracked(_) => ActionError::InvalidInput(
					"Volume is already tracked in this library".to_string(),
				),
				crate::volume::VolumeError::NotFound(_) => {
					ActionError::InvalidInput("Volume not found".to_string())
				}
				crate::volume::VolumeError::Database(msg) => {
					ActionError::Internal(format!("Database error: {}", msg))
				}
				_ => ActionError::Internal(e.to_string()),
			})?;

		Ok(ActionOutput::VolumeTracked {
			fingerprint: self.fingerprint.clone(),
			library_id: self.library_id,
			volume_name: tracked.display_name.unwrap_or(volume.name),
		})
	}
}

pub struct VolumeTrackHandler;

impl VolumeTrackHandler {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait::async_trait]
impl ActionHandler for VolumeTrackHandler {
	async fn execute(
		&self,
		context: std::sync::Arc<CoreContext>,
		action: Action,
	) -> ActionResult<ActionOutput> {
		match action {
			Action::VolumeTrack { action } => {
				// Execute the same logic as the action above using context components
				let library = context
					.library_manager
					.get_library(action.library_id)
					.await
					.ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

				let volume = context
					.volume_manager
					.get_volume(&action.fingerprint)
					.await
					.ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;

				if !volume.is_mounted {
					return Err(ActionError::InvalidInput(
						"Cannot track unmounted volume".to_string(),
					));
				}

				let tracked = context
					.volume_manager
					.track_volume(&library, &action.fingerprint, action.name.clone())
					.await
					.map_err(|e| match e {
						crate::volume::VolumeError::AlreadyTracked(_) => ActionError::InvalidInput(
							"Volume is already tracked in this library".to_string(),
						),
						crate::volume::VolumeError::NotFound(_) => {
							ActionError::InvalidInput("Volume not found".to_string())
						}
						crate::volume::VolumeError::Database(msg) => {
							ActionError::Internal(format!("Database error: {}", msg))
						}
						_ => ActionError::Internal(e.to_string()),
					})?;

				Ok(ActionOutput::VolumeTracked {
					fingerprint: action.fingerprint,
					library_id: action.library_id,
					volume_name: tracked.display_name.unwrap_or(volume.name),
				})
			}
			_ => Err(ActionError::InvalidActionType),
		}
	}

	fn can_handle(&self, action: &Action) -> bool {
		matches!(action, Action::VolumeTrack { .. })
	}

	fn supported_actions() -> &'static [&'static str] {
		&["volume.track"]
	}
}

register_action_handler!(VolumeTrackHandler, "volume.track");
