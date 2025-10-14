//! Add cloud volume action
//!
//! This action adds a cloud storage volume (S3, Google Drive, etc.) to a library,
//! storing encrypted credentials and creating a virtual volume for indexing.

use super::output::VolumeAddCloudOutput;
use crate::{
	context::CoreContext,
	crypto::cloud_credentials::{CloudCredential, CloudCredentialManager},
	infra::action::{error::ActionError, LibraryAction},
	volume::{backend::CloudServiceType, CloudBackend, Volume, VolumeFingerprint},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeAddCloudInput {
	pub service: CloudServiceType,
	pub display_name: String,
	pub config: CloudStorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type")]
pub enum CloudStorageConfig {
	S3 {
		bucket: String,
		region: String,
		access_key_id: String,
		secret_access_key: String,
		endpoint: Option<String>,
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAddCloudAction {
	input: VolumeAddCloudInput,
}

impl VolumeAddCloudAction {
	pub fn new(input: VolumeAddCloudInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for VolumeAddCloudAction {
	type Input = VolumeAddCloudInput;
	type Output = VolumeAddCloudOutput;

	fn from_input(input: VolumeAddCloudInput) -> Result<Self, String> {
		Ok(VolumeAddCloudAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let device_id = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::InvalidInput(format!("Failed to get device ID: {}", e)))?;
		let library_id = library.id();

		let (backend, credential, mount_point) = match &self.input.config {
			CloudStorageConfig::S3 {
				bucket,
				region,
				access_key_id,
				secret_access_key,
				endpoint,
			} => {
				let backend = CloudBackend::new_s3(
					bucket,
					region,
					access_key_id,
					secret_access_key,
					endpoint.clone(),
				)
				.await
				.map_err(|e| ActionError::InvalidInput(format!("Failed to create S3 backend: {}", e)))?;

				let credential = CloudCredential::new_access_key(
					CloudServiceType::S3,
					access_key_id.clone(),
					secret_access_key.clone(),
					None,
				);

				let mount_point = PathBuf::from(format!("cloud://s3/{}", bucket));

				(backend, credential, mount_point)
			}
		};

		let fingerprint = VolumeFingerprint::new(
			&self.input.display_name,
			0, // Cloud volumes don't have a fixed size
			&format!("{:?}", self.input.service),
		);

		let backend_arc: Arc<dyn crate::volume::VolumeBackend> = Arc::new(backend);

		let volume = Volume {
			fingerprint: fingerprint.clone(),
			device_id,
			name: self.input.display_name.clone(),
			mount_type: crate::volume::types::MountType::Network,
			volume_type: crate::volume::types::VolumeType::Network,
			mount_point: mount_point.clone(),
			mount_points: vec![mount_point],
			is_mounted: true,
			disk_type: crate::volume::types::DiskType::Unknown,
			file_system: crate::volume::types::FileSystem::Other(format!("{:?}", self.input.service)),
			total_bytes_capacity: 0,
			total_bytes_available: 0,
			read_only: false,
			hardware_id: None,
			error_status: None,
			apfs_container: None,
			container_volume_id: None,
			path_mappings: Vec::new(),
			backend: Some(backend_arc),
			read_speed_mbps: None,
			write_speed_mbps: None,
			auto_track_eligible: false,
			is_user_visible: true,
			last_updated: chrono::Utc::now(),
		};

		let credential_manager = CloudCredentialManager::new(context.library_key_manager.clone());
		credential_manager
			.store_credential(library_id, &fingerprint.0, &credential)
			.map_err(|e| {
				ActionError::InvalidInput(format!("Failed to store credentials: {}", e))
			})?;

		let tracked = context
			.volume_manager
			.track_volume(&library, &fingerprint, Some(self.input.display_name.clone()))
			.await
			.map_err(|e| ActionError::InvalidInput(format!("Volume tracking failed: {}", e)))?;

		Ok(VolumeAddCloudOutput::new(
			fingerprint,
			self.input.display_name,
			self.input.service,
		))
	}

	fn action_kind(&self) -> &'static str {
		"volumes.add_cloud"
	}
}

crate::register_library_action!(VolumeAddCloudAction, "volumes.add_cloud");
