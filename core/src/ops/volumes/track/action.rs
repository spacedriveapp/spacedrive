//! Volume track action

use super::{VolumeTrackInput, VolumeTrackOutput};
use crate::{
	context::CoreContext,
	domain::{resource::Identifiable, volume::Volume},
	infra::{action::error::ActionError, event::Event},
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackAction {
	input: VolumeTrackInput,
}

impl VolumeTrackAction {
	pub fn new(input: VolumeTrackInput) -> Self {
		Self { input }
	}
}

crate::register_library_action!(VolumeTrackAction, "volumes.track");

impl crate::infra::action::LibraryAction for VolumeTrackAction {
	type Input = VolumeTrackInput;
	type Output = VolumeTrackOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(VolumeTrackAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let fingerprint = VolumeFingerprint::from_string(&self.input.fingerprint)
			.map_err(|e| ActionError::Internal(format!("Invalid fingerprint: {}", e)))?;

		// Verify the volume is user-visible before tracking
		let volumes = context.volume_manager.get_all_volumes().await;
		let volume_to_track = volumes
			.iter()
			.find(|v| v.fingerprint == fingerprint)
			.ok_or_else(|| ActionError::Internal("Volume not found".to_string()))?;

		if !volume_to_track.is_user_visible {
			return Err(ActionError::Internal(
				"Cannot track system volumes".to_string(),
			));
		}

		// Track the volume
		let tracked_volume = context
			.volume_manager
			.track_volume(&library, &fingerprint, self.input.display_name.clone())
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Emit ResourceChanged event for the tracked volume
		let mut vol = volume_to_track.clone();
		vol.is_tracked = true;
		vol.library_id = Some(library.id());

		context.events.emit(Event::ResourceChanged {
			resource_type: Volume::resource_type().to_string(),
			resource: serde_json::to_value(&vol)
				.map_err(|e| ActionError::Internal(e.to_string()))?,
			metadata: None,
		});

		Ok(VolumeTrackOutput {
			volume_id: tracked_volume.uuid,
			fingerprint: tracked_volume.fingerprint,
			name: tracked_volume
				.display_name
				.unwrap_or_else(|| "Unnamed".to_string()),
			is_online: tracked_volume.is_online,
		})
	}

	fn action_kind(&self) -> &'static str {
		"volumes.track"
	}
}
