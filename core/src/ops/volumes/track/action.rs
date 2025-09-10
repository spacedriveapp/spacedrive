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
use uuid::Uuid;

/// Input for tracking a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackAction {
	/// The fingerprint of the volume to track
	pub fingerprint: VolumeFingerprint,

	/// The library ID to track the volume in
	pub library_id: Uuid,

	/// Optional name for the tracked volume
	pub name: Option<String>,
}

impl VolumeTrackAction {
	pub fn new(fingerprint: VolumeFingerprint, library_id: Uuid, name: Option<String>) -> Self {
		Self {
			fingerprint,
			library_id,
			name,
		}
	}

	/// Create a volume track action with a name
	pub fn with_name(fingerprint: VolumeFingerprint, library_id: Uuid, name: String) -> Self {
		Self::new(fingerprint, library_id, Some(name))
	}

	/// Create a volume track action without a name
	pub fn without_name(fingerprint: VolumeFingerprint, library_id: Uuid) -> Self {
		Self::new(fingerprint, library_id, None)
	}
}

impl LibraryAction for VolumeTrackAction {
	type Output = VolumeTrackOutput;

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Check if volume exists
		let volume = context
			.volume_manager
			.get_volume(&self.fingerprint)
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
			.track_volume(&library, &self.fingerprint, self.name.clone())
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Volume tracking failed: {}", e)))?;

		Ok(VolumeTrackOutput::new(
			self.fingerprint,
			self.library_id,
			tracked.display_name.unwrap_or(volume.name),
		))
	}

	fn action_kind(&self) -> &'static str {
		"volume.track"
	}

	fn library_id(&self) -> Uuid {
		self.library_id
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Validate volume exists
		let volume = context
			.volume_manager
			.get_volume(&self.fingerprint)
			.await
			.ok_or_else(|| ActionError::Validation {
				field: "fingerprint".to_string(),
				message: "Volume not found".to_string(),
			})?;

		// Validate volume is mounted
		if !volume.is_mounted {
			return Err(ActionError::Validation {
				field: "fingerprint".to_string(),
				message: "Cannot track unmounted volume".to_string(),
			});
		}

		// Validate name if provided
		if let Some(name) = &self.name {
			if name.trim().is_empty() {
				return Err(ActionError::Validation {
					field: "name".to_string(),
					message: "Volume name cannot be empty".to_string(),
				});
			}
		}

		Ok(())
	}
}
