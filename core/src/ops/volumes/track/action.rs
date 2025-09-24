//! Track volume action
//!
//! This action tracks a volume within a library, allowing Spacedrive to monitor
//! and index files on the volume.

use super::output::VolumeTrackOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeTrackInput {
	pub fingerprint: VolumeFingerprint,
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackAction {
	input: VolumeTrackInput,
}

impl VolumeTrackAction {
	pub fn new(input: VolumeTrackInput) -> Self {
		Self { input }
	}

	/// Create a volume track action with a name
	pub fn with_name(fingerprint: VolumeFingerprint, name: String) -> Self {
		Self::new(VolumeTrackInput {
			fingerprint,
			name: Some(name),
		})
	}

	/// Create a volume track action without a name
	pub fn without_name(fingerprint: VolumeFingerprint) -> Self {
		Self::new(VolumeTrackInput {
			fingerprint,
			name: None,
		})
	}
}

impl LibraryAction for VolumeTrackAction {
	type Input = VolumeTrackInput;
	type Output = VolumeTrackOutput;

	fn from_input(input: VolumeTrackInput) -> Result<Self, String> {
		Ok(VolumeTrackAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Check if volume exists
		let volume = context
			.volume_manager
			.get_volume(&self.input.fingerprint)
			.await
			.ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;

		if !volume.is_mounted {
			return Err(ActionError::InvalidInput(
				"Cannot track unmounted volume".to_string(),
			));
		}

		// Track the volume in the database
		let tracked = context
			.volume_manager
			.track_volume(&library, &self.input.fingerprint, self.input.name.clone())
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Volume tracking failed: {}", e)))?;

		Ok(VolumeTrackOutput::new(
			self.input.fingerprint,
			tracked.display_name.unwrap_or(volume.name),
		))
	}

	fn action_kind(&self) -> &'static str {
		"volumes.track"
	}

}

// Register action
crate::register_library_action!(VolumeTrackAction, "volumes.track");
