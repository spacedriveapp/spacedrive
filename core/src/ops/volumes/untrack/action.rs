//! Untrack volume action
//!
//! This action removes volume tracking from a library.

use super::output::VolumeUntrackOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeUntrackInput {
	pub fingerprint: VolumeFingerprint,
}

/// Input for untracking a volume
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeUntrackAction {
	/// The fingerprint of the volume to untrack
	input: VolumeUntrackInput,
}

impl VolumeUntrackAction {
	/// Create a new volume untrack action
	pub fn new(input: VolumeUntrackInput) -> Self {
		Self { input }
	}
}

// Implement the unified ActionTrait (following VolumeTrackAction model)
impl LibraryAction for VolumeUntrackAction {
	type Input = VolumeUntrackInput;
	type Output = VolumeUntrackOutput;

	fn from_input(input: VolumeUntrackInput) -> Result<Self, String> {
		Ok(VolumeUntrackAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Untrack the volume from the database
		context
			.volume_manager
			.untrack_volume(&library, &self.input.fingerprint)
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Volume untracking failed: {}", e)))?;

		// Return native output directly
		Ok(VolumeUntrackOutput::new(self.input.fingerprint))
	}

	fn action_kind(&self) -> &'static str {
		"volumes.untrack"
	}
}

// Register action
crate::register_library_action!(VolumeUntrackAction, "volumes.untrack");
