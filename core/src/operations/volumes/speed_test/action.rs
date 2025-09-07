//! Volume speed test action
//!
//! This action tests the read/write performance of a volume.

use crate::{
	infrastructure::actions::{error::ActionError, output::ActionOutput},
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
	/// Execute the volume speed test action
	pub async fn execute(&self, core: &crate::Core) -> Result<ActionOutput, ActionError> {
		// Run the speed test through the volume manager
		core.volumes
			.run_speed_test(&self.fingerprint)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Get the updated volume with speed test results
		let volume = core
			.volumes
			.get_volume(&self.fingerprint)
			.await
			.ok_or_else(|| {
				ActionError::InvalidInput("Volume not found after speed test".to_string())
			})?;

		Ok(ActionOutput::VolumeSpeedTested {
			fingerprint: self.fingerprint.clone(),
			read_speed_mbps: volume.read_speed_mbps.map(|v| v as u32),
			write_speed_mbps: volume.write_speed_mbps.map(|v| v as u32),
		})
	}
}
