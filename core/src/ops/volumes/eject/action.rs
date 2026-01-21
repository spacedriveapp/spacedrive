//! Volume eject action

use super::{VolumeEjectInput, VolumeEjectOutput};
use crate::{
	context::CoreContext,
	infra::action::error::ActionError,
	volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEjectAction {
	input: VolumeEjectInput,
}

impl VolumeEjectAction {
	pub fn new(input: VolumeEjectInput) -> Self {
		Self { input }
	}
}

crate::register_library_action!(VolumeEjectAction, "volumes.eject");

impl crate::infra::action::LibraryAction for VolumeEjectAction {
	type Input = VolumeEjectInput;
	type Output = VolumeEjectOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(VolumeEjectAction::new(input))
	}

	async fn execute(
		self,
		_library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let fingerprint = VolumeFingerprint(self.input.fingerprint.clone());

		info!("Ejecting volume with fingerprint: {}", fingerprint);

		// Get the volume from the volume manager
		let volume = context
			.volume_manager
			.get_volume(&fingerprint)
			.await
			.ok_or_else(|| {
				ActionError::Internal(format!("Volume not found: {}", fingerprint))
			})?;

		// Check if volume is mounted
		if !volume.is_mounted {
			return Ok(VolumeEjectOutput {
				fingerprint: self.input.fingerprint,
				success: false,
				message: Some("Volume is not mounted".to_string()),
			});
		}

		// Platform-specific eject
		let result = eject_volume_platform(&volume.mount_point.to_string_lossy()).await;

		match result {
			Ok(message) => {
				info!("Successfully ejected volume: {}", fingerprint);
				Ok(VolumeEjectOutput {
					fingerprint: self.input.fingerprint,
					success: true,
					message: Some(message),
				})
			}
			Err(e) => {
				error!("Failed to eject volume {}: {}", fingerprint, e);
				Ok(VolumeEjectOutput {
					fingerprint: self.input.fingerprint,
					success: false,
					message: Some(format!("Eject failed: {}", e)),
				})
			}
		}
	}

	fn action_kind(&self) -> &'static str {
		"volumes.eject"
	}
}

/// Platform-specific volume ejection
#[cfg(target_os = "macos")]
async fn eject_volume_platform(mount_point: &str) -> Result<String, String> {
	use tokio::process::Command as TokioCommand;

	info!("Ejecting volume at mount point: {}", mount_point);

	// Use diskutil eject on macOS
	let output = TokioCommand::new("diskutil")
		.args(["eject", mount_point])
		.output()
		.await
		.map_err(|e| format!("Failed to execute diskutil: {}", e))?;

	if output.status.success() {
		let stdout = String::from_utf8_lossy(&output.stdout);
		Ok(stdout.trim().to_string())
	} else {
		let stderr = String::from_utf8_lossy(&output.stderr);
		Err(stderr.trim().to_string())
	}
}

/// Platform-specific volume ejection
#[cfg(target_os = "linux")]
async fn eject_volume_platform(mount_point: &str) -> Result<String, String> {
	use tokio::process::Command as TokioCommand;

	info!("Unmounting volume at mount point: {}", mount_point);

	// Use umount on Linux
	let output = TokioCommand::new("umount")
		.arg(mount_point)
		.output()
		.await
		.map_err(|e| format!("Failed to execute umount: {}", e))?;

	if output.status.success() {
		Ok(format!("Successfully unmounted {}", mount_point))
	} else {
		let stderr = String::from_utf8_lossy(&output.stderr);
		Err(format!("Failed to unmount: {}", stderr.trim()))
	}
}

/// Platform-specific volume ejection
#[cfg(target_os = "windows")]
async fn eject_volume_platform(_mount_point: &str) -> Result<String, String> {
	// Windows requires Win32 API calls or PowerShell
	Err("Volume ejection is not yet implemented on Windows. Please use Windows Explorer to safely remove the device.".to_string())
}

/// Fallback for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
async fn eject_volume_platform(_mount_point: &str) -> Result<String, String> {
	Err("Volume ejection is not supported on this platform".to_string())
}
