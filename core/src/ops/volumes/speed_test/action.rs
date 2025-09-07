//! Volume speed test action
//!
//! This action tests the read/write performance of a volume.

use crate::{
	infra::action::{error::ActionError, output::ActionOutput},
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

		// Extract speeds (default to 0 if missing)
		let read_speed = volume.read_speed_mbps.unwrap_or(0);
		let write_speed = volume.write_speed_mbps.unwrap_or(0);

		// Persist results to all open libraries where this volume is tracked
		let libraries = core.libraries.get_open_libraries().await;
		if let Err(e) = core
			.volumes
			.save_speed_test_results(&self.fingerprint, read_speed, write_speed, &libraries)
			.await
		{
			// Log error but don't fail the action since the speed test itself succeeded
			tracing::warn!("Failed to save speed test results to database: {}", e);
		}

		Ok(ActionOutput::VolumeSpeedTested {
			fingerprint: self.fingerprint.clone(),
			read_speed_mbps: Some(read_speed as u32),
			write_speed_mbps: Some(write_speed as u32),
		})
	}
}
