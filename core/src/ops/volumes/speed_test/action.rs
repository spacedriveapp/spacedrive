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
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
		let mut volume = context
			.volume_manager
			.get_volume(&self.input.fingerprint)
			.await
			.ok_or_else(|| {
				ActionError::InvalidInput("Volume not found after speed test".to_string())
			})?;

		// Extract speeds (default to 0 if missing)
		let read_speed = volume.read_speed_mbps.unwrap_or(0);
		let write_speed = volume.write_speed_mbps.unwrap_or(0);

		// Save results to database
		context
			.volume_manager
			.save_speed_test_results(
				&self.input.fingerprint,
				read_speed,
				write_speed,
				&[library.clone()],
			)
			.await
			.map_err(|e| {
				ActionError::InvalidInput(format!("Failed to save speed test results: {}", e))
			})?;

		// Update volume timestamps to match what was saved to database
		volume.updated_at = chrono::Utc::now();

		// Log the volume data before emitting to verify it has speeds
		tracing::info!(
			"Emitting ResourceChanged for volume '{}' with speeds: read={}MB/s write={}MB/s",
			volume.name,
			volume.read_speed_mbps.unwrap_or(0),
			volume.write_speed_mbps.unwrap_or(0)
		);

		// Emit ResourceChanged event for the volume with complete data
		use crate::domain::resource::EventEmitter;
		volume
			.emit_changed(&context.events)
			.map_err(|e| {
				ActionError::Internal(format!("Failed to emit volume event: {}", e))
			})?;

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
}

// Register action
crate::register_library_action!(VolumeSpeedTestAction, "volumes.speed_test");
