//! Volume speed test action
//!
//! This action tests the read/write performance of a volume.

use super::output::VolumeSpeedTestOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSpeedTestInput {
	pub fingerprint: VolumeFingerprint,
}

/// Input for volume speed testing
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct VolumeSpeedTestAction {
	/// The fingerprint of the volume to test
	input: VolumeSpeedTestInput,
}

impl VolumeSpeedTestAction {
	/// Create a new volume speed test action
	pub fn new(input: VolumeSpeedTestInput) -> Self {
		Self { input }
	}
}

// Implement the new modular ActionType trait
impl LibraryAction for VolumeSpeedTestAction {
	type Input = VolumeSpeedTestInput;
	type Output = VolumeSpeedTestOutput;

	fn from_input(input: VolumeSpeedTestInput) -> Result<Self, String> {
		Ok(VolumeSpeedTestAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Run the speed test through the volume manager
		context
			.volume_manager
			.run_speed_test(&self.input.fingerprint)
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Speed test failed: {}", e)))?;

		// Get the updated volume with speed test results
		let volume = context
			.volume_manager
			.get_volume(&self.input.fingerprint)
			.await
			.ok_or_else(|| {
				ActionError::InvalidInput("Volume not found after speed test".to_string())
			})?;

		// Extract speeds (default to 0 if missing)
		let read_speed = volume.read_speed_mbps.unwrap_or(0);
		let write_speed = volume.write_speed_mbps.unwrap_or(0);

		// Return native output directly
		Ok(VolumeSpeedTestOutput::new(
			self.input.fingerprint,
			Some(read_speed as u32),
			Some(write_speed as u32),
		))
	}

	fn action_kind(&self) -> &'static str {
		"volume.speed_test"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Validate volume exists
		let volume = context
			.volume_manager
			.get_volume(&self.input.fingerprint)
			.await
			.ok_or_else(|| ActionError::Validation {
				field: "fingerprint".to_string(),
				message: "Volume not found".to_string(),
			})?;

		// Validate volume is mounted (can't test unmounted volumes)
		if !volume.is_mounted {
			return Err(ActionError::Validation {
				field: "fingerprint".to_string(),
				message: "Cannot test speed of unmounted volume".to_string(),
			});
		}

		Ok(())
	}
}
