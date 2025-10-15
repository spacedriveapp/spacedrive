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
	GoogleDrive {
		root: Option<String>,
		access_token: String,
		refresh_token: String,
		client_id: String,
		client_secret: String,
	},
	OneDrive {
		root: Option<String>,
		access_token: String,
		refresh_token: String,
		client_id: String,
		client_secret: String,
	},
	Dropbox {
		root: Option<String>,
		access_token: String,
		refresh_token: String,
		client_id: String,
		client_secret: String,
	},
	AzureBlob {
		container: String,
		endpoint: Option<String>,
		account_name: String,
		account_key: String,
	},
	GoogleCloudStorage {
		bucket: String,
		root: Option<String>,
		endpoint: Option<String>,
		credential: String, // Service account JSON
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
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create S3 backend: {}", e))
				})?;

				let credential = CloudCredential::new_access_key(
					CloudServiceType::S3,
					access_key_id.clone(),
					secret_access_key.clone(),
					None,
				);

				let mount_point = PathBuf::from(format!("cloud://s3/{}", bucket));

				(backend, credential, mount_point)
			}
			CloudStorageConfig::GoogleDrive {
				root,
				access_token,
				refresh_token,
				client_id,
				client_secret,
			} => {
				let backend = CloudBackend::new_google_drive(
					access_token,
					refresh_token,
					client_id,
					client_secret,
					root.clone(),
				)
				.await
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create Google Drive backend: {}", e))
				})?;

				let credential = CloudCredential::new_oauth(
					CloudServiceType::GoogleDrive,
					access_token.clone(),
					refresh_token.clone(),
					None, // Google Drive tokens typically don't have a fixed expiry in the refresh flow
				);

				let mount_point = PathBuf::from(format!(
					"cloud://gdrive/{}",
					root.as_deref().unwrap_or("root")
				));

				(backend, credential, mount_point)
			}
			CloudStorageConfig::OneDrive {
				root,
				access_token,
				refresh_token,
				client_id,
				client_secret,
			} => {
				let backend = CloudBackend::new_onedrive(
					access_token,
					refresh_token,
					client_id,
					client_secret,
					root.clone(),
				)
				.await
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create OneDrive backend: {}", e))
				})?;

				let credential = CloudCredential::new_oauth(
					CloudServiceType::OneDrive,
					access_token.clone(),
					refresh_token.clone(),
					None,
				);

				let mount_point = PathBuf::from(format!(
					"cloud://onedrive/{}",
					root.as_deref().unwrap_or("root")
				));

				(backend, credential, mount_point)
			}
			CloudStorageConfig::Dropbox {
				root,
				access_token,
				refresh_token,
				client_id,
				client_secret,
			} => {
				let backend = CloudBackend::new_dropbox(
					access_token,
					refresh_token,
					client_id,
					client_secret,
					root.clone(),
				)
				.await
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create Dropbox backend: {}", e))
				})?;

				let credential = CloudCredential::new_oauth(
					CloudServiceType::Dropbox,
					access_token.clone(),
					refresh_token.clone(),
					None,
				);

				let mount_point = PathBuf::from(format!(
					"cloud://dropbox/{}",
					root.as_deref().unwrap_or("root")
				));

				(backend, credential, mount_point)
			}
			CloudStorageConfig::AzureBlob {
				container,
				endpoint,
				account_name,
				account_key,
			} => {
				let backend = CloudBackend::new_azure_blob(
					container,
					account_name,
					account_key,
					endpoint.clone(),
				)
				.await
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create Azure Blob backend: {}", e))
				})?;

				let credential = CloudCredential::new_access_key(
					CloudServiceType::AzureBlob,
					account_name.clone(),
					account_key.clone(),
					None,
				);

				let mount_point = PathBuf::from(format!("cloud://azblob/{}", container));

				(backend, credential, mount_point)
			}
			CloudStorageConfig::GoogleCloudStorage {
				bucket,
				root,
				endpoint,
				credential: service_account_json,
			} => {
				let backend = CloudBackend::new_google_cloud_storage(
					bucket,
					service_account_json,
					root.clone(),
					endpoint.clone(),
				)
				.await
				.map_err(|e| {
					ActionError::InvalidInput(format!("Failed to create GCS backend: {}", e))
				})?;

				let credential = CloudCredential::new_api_key(
					CloudServiceType::GoogleCloudStorage,
					service_account_json.clone(),
				);

				let mount_point = PathBuf::from(format!("cloud://gcs/{}", bucket));

				(backend, credential, mount_point)
			}
		};

		let fingerprint = VolumeFingerprint::new(
			&self.input.display_name,
			0, // Cloud volumes don't have a fixed size
			&format!("{:?}", self.input.service),
		);

		let backend_arc: Arc<dyn crate::volume::VolumeBackend> = Arc::new(backend);
		let now = chrono::Utc::now();

		let volume = Volume {
			id: Uuid::new_v4(), // Generate UUID for cloud volume
			fingerprint: fingerprint.clone(),
			device_id,
			name: self.input.display_name.clone(),
			library_id: None,
			is_tracked: false,
			mount_point: mount_point.clone(),
			mount_points: vec![mount_point],
			volume_type: crate::volume::types::VolumeType::Network,
			mount_type: crate::volume::types::MountType::Network,
			disk_type: crate::volume::types::DiskType::Unknown,
			file_system: crate::volume::types::FileSystem::Other(format!(
				"{:?}",
				self.input.service
			)),
			total_capacity: 0,
			available_space: 0,
			is_read_only: false,
			is_mounted: true,
			hardware_id: None,
			backend: Some(backend_arc),
			apfs_container: None,
			container_volume_id: None,
			path_mappings: Vec::new(),
			is_user_visible: true,
			auto_track_eligible: false,
			read_speed_mbps: None,
			write_speed_mbps: None,
			created_at: now,
			updated_at: now,
			last_seen_at: now,
			total_files: None,
			total_directories: None,
			last_stats_update: None,
			display_name: Some(self.input.display_name.clone()),
			is_favorite: false,
			color: None,
			icon: None,
			error_message: None,
		};

		let credential_manager = CloudCredentialManager::new(context.library_key_manager.clone());
		credential_manager
			.store_credential(library_id, &fingerprint.0, &credential)
			.map_err(|e| {
				ActionError::InvalidInput(format!("Failed to store credentials: {}", e))
			})?;

		tracing::info!("Successfully stored credentials for cloud volume {} in keyring (library: {}, fingerprint: {})",
			self.input.display_name, library_id, fingerprint.0);

		// Register the cloud volume with the volume manager so it can be tracked
		context
			.volume_manager
			.register_cloud_volume(volume.clone())
			.await;

		let tracked = context
			.volume_manager
			.track_volume(
				&library,
				&fingerprint,
				Some(self.input.display_name.clone()),
			)
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
