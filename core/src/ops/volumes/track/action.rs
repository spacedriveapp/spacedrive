//! Volume track action
//!
//! Tracking a drive indexes it. A tracked volume with no map is a row saying
//! Spacedrive knows about a drive without knowing anything on it, which is not
//! a state anyone asks for, so the two are one gesture.
//!
//! The drive and its index stay separate underneath: this flips `is_tracked`
//! on the volume row and then calls the same `track_and_index` that
//! `sources.track` does. What converges is the click, not the model.

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

		// Indexing is what makes a tracked drive useful, so it starts here
		// rather than waiting for a second gesture. An unmounted drive has
		// nothing to walk; its map arrives when it returns.
		if tracked_volume.is_online {
			if let Some(mount_point) = tracked_volume.mount_point.clone() {
				// An external drive is usually being archived, where
				// completeness is the point, so it records everything readable
				// rather than applying rules that hide files by default.
				let unfiltered =
					volume_to_track.mount_type == crate::domain::volume::MountType::External;
				if let Err(e) = crate::ops::sources::track::track_and_index(
					&library,
					&context,
					std::path::PathBuf::from(mount_point),
					unfiltered,
				)
				.await
				{
					tracing::error!(%e, "tracked the volume but could not start indexing it");
				}
			}
		}

		// Emit ResourceChanged event for the tracked volume using EventEmitter
		let mut vol = volume_to_track.clone();
		vol.is_tracked = true;
		vol.library_id = Some(library.id());

		use crate::domain::resource::EventEmitter;
		vol.emit_changed(&context.events)
			.map_err(|e| ActionError::Internal(format!("Failed to emit volume event: {}", e)))?;

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
