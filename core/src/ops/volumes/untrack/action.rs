//! Untrack volume action
//!
//! This action removes volume tracking from a library.

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

/// Input for untracking a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUntrackAction {
	/// The fingerprint of the volume to untrack
	pub fingerprint: VolumeFingerprint,

	/// The library ID to untrack the volume from
	pub library_id: Uuid,
}

impl VolumeUntrackAction {
	/// Execute the volume untracking action
	pub async fn execute(&self, core: &crate::Core) -> Result<ActionOutput, ActionError> {
		// Get the library
		let library = core
			.libraries
			.get_library(self.library_id)
			.await
			.ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

		// Untrack the volume from the database
		core.volumes
			.untrack_volume(&library, &self.fingerprint)
			.await
			.map_err(|e| match e {
				crate::volume::VolumeError::NotTracked(_) => {
					ActionError::InvalidInput("Volume is not tracked in this library".to_string())
				}
				crate::volume::VolumeError::Database(msg) => {
					ActionError::Internal(format!("Database error: {}", msg))
				}
				_ => ActionError::Internal(e.to_string()),
			})?;

		Ok(ActionOutput::VolumeUntracked {
			fingerprint: self.fingerprint.clone(),
			library_id: self.library_id,
		})
	}
}

pub struct VolumeUntrackHandler;

impl VolumeUntrackHandler {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait::async_trait]
impl ActionHandler for VolumeUntrackHandler {
	async fn execute(
		&self,
		context: std::sync::Arc<CoreContext>,
		action: Action,
	) -> ActionResult<ActionOutput> {
		match action {
			Action::VolumeUntrack { action } => {
				let library = context
					.library_manager
					.get_library(action.library_id)
					.await
					.ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

				context
					.volume_manager
					.untrack_volume(&library, &action.fingerprint)
					.await
					.map_err(|e| match e {
						crate::volume::VolumeError::NotTracked(_) => ActionError::InvalidInput(
							"Volume is not tracked in this library".to_string(),
						),
						crate::volume::VolumeError::Database(msg) => {
							ActionError::Internal(format!("Database error: {}", msg))
						}
						_ => ActionError::Internal(e.to_string()),
					})?;

				Ok(ActionOutput::VolumeUntracked {
					fingerprint: action.fingerprint,
					library_id: action.library_id,
				})
			}
			_ => Err(ActionError::InvalidActionType),
		}
	}

	fn can_handle(&self, action: &Action) -> bool {
		matches!(action, Action::VolumeUntrack { .. })
	}

	fn supported_actions() -> &'static [&'static str] {
		&["volume.untrack"]
	}
}

register_action_handler!(VolumeUntrackHandler, "volume.untrack");
