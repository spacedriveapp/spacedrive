//! Untrack volume action
//!
//! This action removes volume tracking from a library.

use super::output::VolumeUntrackOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		error::ActionError,
		LibraryAction,
	},
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
	/// Create a new volume untrack action
	pub fn new(fingerprint: VolumeFingerprint, library_id: Uuid) -> Self {
		Self {
			fingerprint,
			library_id,
		}
	}
}

// Implement the unified ActionTrait (following VolumeTrackAction model)
impl LibraryAction for VolumeUntrackAction {
	type Output = VolumeUntrackOutput;

	async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: std::sync::Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager - no boilerplate!

		// Untrack the volume from the database
		context
			.volume_manager
			.untrack_volume(&library, &self.fingerprint)
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Volume untracking failed: {}", e)))?;

		// Return native output directly
		Ok(VolumeUntrackOutput::new(self.fingerprint, self.library_id))
	}

	fn action_kind(&self) -> &'static str {
		"volume.untrack"
	}

	fn library_id(&self) -> Uuid {
		self.library_id
	}

	async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: std::sync::Arc<CoreContext>) -> Result<(), ActionError> {
		// Library existence already validated by ActionManager - no boilerplate!

		// Validate volume exists
		let _volume = context
			.volume_manager
			.get_volume(&self.fingerprint)
			.await
			.ok_or_else(|| ActionError::Validation {
				field: "fingerprint".to_string(),
				message: "Volume not found".to_string(),
			})?;

		Ok(())
	}
}