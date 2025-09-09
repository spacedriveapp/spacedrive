//! Volume speed test action
//!
//! This action tests the read/write performance of a volume.

use super::output::VolumeSpeedTestOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, ActionTrait},
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};

/// Input for volume speed testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSpeedTestAction {
	/// The fingerprint of the volume to test
	pub fingerprint: VolumeFingerprint,
}

impl VolumeSpeedTestAction {
	/// Create a new volume speed test action
	pub fn new(fingerprint: VolumeFingerprint) -> Self {
		Self { fingerprint }
	}
}

// Implement the new modular ActionType trait
impl ActionTrait for VolumeSpeedTestAction {
	type Output = VolumeSpeedTestOutput;

	async fn execute(self, context: std::sync::Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Run the speed test through the volume manager
		context.volume_manager
			.run_speed_test(&self.fingerprint)
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Speed test failed: {}", e)))?;

		// Get the updated volume with speed test results
		let volume = context
			.volume_manager
			.get_volume(&self.fingerprint)
			.await
			.ok_or_else(|| ActionError::InvalidInput("Volume not found after speed test".to_string()))?;

		// Extract speeds (default to 0 if missing)
		let read_speed = volume.read_speed_mbps.unwrap_or(0);
		let write_speed = volume.write_speed_mbps.unwrap_or(0);

		// Return native output directly
		Ok(VolumeSpeedTestOutput::new(
			self.fingerprint,
			Some(read_speed as u32),
			Some(write_speed as u32),
		))
	}

	fn action_kind(&self) -> &'static str {
		"volume.speed_test"
	}

	async fn validate(&self, context: std::sync::Arc<CoreContext>) -> Result<(), ActionError> {
		// Validate volume exists
		let volume = context
			.volume_manager
			.get_volume(&self.fingerprint)
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
