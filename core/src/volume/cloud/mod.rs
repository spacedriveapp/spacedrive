use super::{error::VolumeError, types::CloudProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CloudStorageInfo {
	pub total_bytes_capacity: u64,
	pub total_bytes_available: u64,
	pub quota_info: Option<QuotaInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QuotaInfo {
	pub used: u64,
	pub allocated: u64,
	pub max: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum CloudCredentials {
	OAuth {
		access_token: String,
		refresh_token: Option<String>,
		expires_at: Option<i64>,
	},
	ApiKey(String),
	Custom(serde_json::Value),
}

#[async_trait]
pub trait CloudVolumeProvider: Send + Sync {
	/// Get storage capacity and usage information
	async fn get_storage_info(&self) -> Result<CloudStorageInfo, VolumeError>;

	/// Check if the current credentials are valid
	async fn is_authenticated(&self) -> bool;

	/// Attempt to authenticate with the provider
	async fn authenticate(&self) -> Result<(), VolumeError>;

	/// Refresh authentication tokens if needed
	async fn refresh_token(&self) -> Result<(), VolumeError>;
}

// Factory function to create provider implementations
pub fn get_cloud_provider(
	provider: &CloudProvider,
	credentials: CloudCredentials,
) -> Result<Box<dyn CloudVolumeProvider>, VolumeError> {
	match provider {
		// CloudProvider::GoogleDrive => Ok(Box::new(GoogleDriveProvider::new(credentials))),
		// CloudProvider::Dropbox => Ok(Box::new(DropboxProvider::new(credentials))),
		// CloudProvider::OneDrive => Ok(Box::new(OneDriveProvider::new(credentials))),
		// Add other providers as they're implemented
		_ => Err(VolumeError::UnsupportedCloudProvider(provider.clone())),
	}
}
