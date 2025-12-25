//! Cloud storage backend implementation using OpenDAL
//!
//! This module provides cloud storage support for S3, Google Drive, Dropbox,
//! OneDrive, and 40+ other services via Apache OpenDAL.

use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use opendal::Lister;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::debug;

use super::{BackendType, CloudServiceType, RawDirEntry, RawMetadata, VolumeBackend};
use crate::ops::indexing::state::EntryKind;
use crate::volume::error::VolumeError;

/// Cloud storage backend powered by OpenDAL
///
/// Provides unified access to S3, Google Drive, Dropbox, OneDrive, and 40+ other
/// cloud services. Uses OpenDAL's Operator abstraction for consistent I/O operations.
#[derive(Debug, Clone)]
pub struct CloudBackend {
	/// OpenDAL operator for cloud I/O
	operator: opendal::Operator,

	/// Cloud service type for metadata
	service_type: CloudServiceType,

	/// Root path within the cloud storage (e.g., bucket prefix)
	root: PathBuf,
}

impl CloudBackend {
	/// Create a new cloud backend for S3
	///
	/// # Example
	/// ```ignore
	/// let backend = CloudBackend::new_s3(
	///     "my-bucket",
	///     "us-west-2",
	///     "access_key_id",
	///     "secret_access_key",
	///     None, // Custom endpoint (None for AWS)
	/// ).await?;
	/// ```
	pub async fn new_s3(
		bucket: impl AsRef<str>,
		region: impl AsRef<str>,
		access_key_id: impl AsRef<str>,
		secret_access_key: impl AsRef<str>,
		endpoint: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::S3::default()
			.bucket(bucket.as_ref())
			.region(region.as_ref())
			.access_key_id(access_key_id.as_ref())
			.secret_access_key(secret_access_key.as_ref());

		if let Some(ep) = endpoint {
			builder = builder.endpoint(&ep);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| VolumeError::Platform(format!("Failed to create S3 operator: {}", e)))?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::S3,
			root: PathBuf::from("/"),
		})
	}

	/// Create a new cloud backend for Google Drive
	pub async fn new_google_drive(
		access_token: impl AsRef<str>,
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Gdrive::default()
			.access_token(access_token.as_ref())
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Google Drive operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::GoogleDrive,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a new cloud backend for OneDrive
	pub async fn new_onedrive(
		access_token: impl AsRef<str>,
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Onedrive::default()
			.access_token(access_token.as_ref())
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create OneDrive operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::OneDrive,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a new cloud backend for Dropbox
	pub async fn new_dropbox(
		access_token: impl AsRef<str>,
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Dropbox::default()
			.access_token(access_token.as_ref())
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Dropbox operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::Dropbox,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a new cloud backend for Azure Blob Storage
	pub async fn new_azure_blob(
		container: impl AsRef<str>,
		account_name: impl AsRef<str>,
		account_key: impl AsRef<str>,
		endpoint: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Azblob::default()
			.container(container.as_ref())
			.account_name(account_name.as_ref())
			.account_key(account_key.as_ref());

		if let Some(ep) = endpoint {
			builder = builder.endpoint(&ep);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Azure Blob operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::AzureBlob,
			root: PathBuf::from("/"),
		})
	}

	/// Create a new cloud backend for Google Cloud Storage
	pub async fn new_google_cloud_storage(
		bucket: impl AsRef<str>,
		credential: impl AsRef<str>,
		root: Option<String>,
		endpoint: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Gcs::default()
			.bucket(bucket.as_ref())
			.credential(credential.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		if let Some(ep) = endpoint {
			builder = builder.endpoint(&ep);
		}

		let operator = opendal::Operator::new(builder)
			.map_err(|e| VolumeError::Platform(format!("Failed to create GCS operator: {}", e)))?
			.finish();

		Ok(Self {
			operator,
			service_type: CloudServiceType::GoogleCloudStorage,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a cloud backend from a pre-configured OpenDAL operator
	pub fn from_operator(operator: opendal::Operator, service_type: CloudServiceType) -> Self {
		Self {
			operator,
			service_type,
			root: PathBuf::from("/"),
		}
	}
}

impl CloudBackend {
	/// Convert path to cloud storage path (removes leading /)
	fn to_cloud_path(&self, path: &Path) -> String {
		// Cloud storage paths should not have leading /
		path.to_str()
			.unwrap_or("")
			.trim_start_matches('/')
			.to_string()
	}
}

#[async_trait]
impl VolumeBackend for CloudBackend {
	async fn read(&self, path: &Path) -> Result<Bytes, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::read: {}", cloud_path);

		let data = self
			.operator
			.read(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		Ok(data.to_bytes())
	}

	async fn read_range(&self, path: &Path, range: Range<u64>) -> Result<Bytes, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!(
			"CloudBackend::read_range: {} ({}..{})",
			cloud_path, range.start, range.end
		);

		let data = self
			.operator
			.read_with(&cloud_path)
			.range(range.start..range.end)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		Ok(data.to_bytes())
	}

	async fn write(&self, path: &Path, data: Bytes) -> Result<(), VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::write: {} ({} bytes)", cloud_path, data.len());

		self.operator
			.write(&cloud_path, data)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		Ok(())
	}

	async fn read_dir(&self, path: &Path) -> Result<Vec<RawDirEntry>, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::read_dir: {}", cloud_path);

		let mut entries = Vec::new();
		let lister = self
			.operator
			.lister(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		// Collect entries from async iterator

		let mut lister = lister;
		while let Some(entry_result) = lister.try_next().await.transpose() {
			let entry = entry_result
				.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

			let metadata = entry.metadata();
			let name = entry
				.name()
				.trim_end_matches('/')
				.split('/')
				.last()
				.unwrap_or(entry.name())
				.to_string();

			let kind = if metadata.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			};

			entries.push(RawDirEntry {
				name,
				kind,
				size: metadata.content_length(),
				modified: metadata.last_modified().map(|t| {
					// Convert chrono::DateTime to SystemTime
					SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t.timestamp() as u64)
				}),
				inode: None, // Cloud storage doesn't have inodes
			});
		}

		Ok(entries)
	}

	async fn metadata(&self, path: &Path) -> Result<RawMetadata, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::metadata: {}", cloud_path);

		let metadata = self
			.operator
			.stat(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		let kind = if metadata.is_dir() {
			EntryKind::Directory
		} else {
			EntryKind::File
		};

		let modified = metadata.last_modified().map(|t| {
			// Convert chrono::DateTime to SystemTime
			SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t.timestamp() as u64)
		});

		Ok(RawMetadata {
			kind,
			size: metadata.content_length(),
			modified,
			created: None, // Most cloud services don't provide creation time
			accessed: None,
			inode: None,       // Cloud storage doesn't have inodes
			permissions: None, // Cloud backends don't have Unix permissions
		})
	}

	async fn exists(&self, path: &Path) -> Result<bool, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		// OpenDAL doesn't have a direct exists() method, use stat() instead
		match self.operator.stat(&cloud_path).await {
			Ok(_) => Ok(true),
			Err(_) => Ok(false),
		}
	}

	async fn delete(&self, path: &Path) -> Result<(), VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::delete: {}", cloud_path);

		// Check if it's a directory
		let metadata = self
			.operator
			.stat(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		if metadata.is_dir() {
			// Delete directory recursively
			self.operator
				.remove_all(&cloud_path)
				.await
				.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		} else {
			// Delete file
			self.operator
				.delete(&cloud_path)
				.await
				.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		}

		Ok(())
	}

	async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError> {
		let mut cloud_path = self.to_cloud_path(path);
		debug!(
			"CloudBackend::create_directory: {} (recursive: {})",
			cloud_path, recursive
		);

		// Cloud storage directories are implicit, created by writing a marker object
		// Ensure path ends with / to indicate directory
		if !cloud_path.ends_with('/') {
			cloud_path.push('/');
		}

		// OpenDAL's create_dir creates the directory (some backends need explicit creation)
		self.operator
			.create_dir(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		Ok(())
	}

	fn is_local(&self) -> bool {
		false
	}

	fn backend_type(&self) -> BackendType {
		BackendType::Cloud(self.service_type)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: These tests require actual cloud credentials and are disabled by default
	// They serve as examples of how to use the CloudBackend

	#[tokio::test]
	#[ignore]
	async fn test_cloud_backend_s3() {
		// This test requires actual S3 credentials
		// Set these environment variables to run:
		// AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_BUCKET, AWS_REGION

		let bucket = std::env::var("AWS_BUCKET").unwrap();
		let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
		let access_key = std::env::var("AWS_ACCESS_KEY_ID").unwrap();
		let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap();

		let backend = CloudBackend::new_s3(&bucket, &region, &access_key, &secret_key, None)
			.await
			.unwrap();

		// Test write
		let test_data = Bytes::from("Hello, cloud!");
		backend
			.write(Path::new("test.txt"), test_data.clone())
			.await
			.unwrap();

		// Test read
		let read_data = backend.read(Path::new("test.txt")).await.unwrap();
		assert_eq!(test_data, read_data);

		// Test metadata
		let metadata = backend.metadata(Path::new("test.txt")).await.unwrap();
		assert_eq!(metadata.size, test_data.len() as u64);
		assert_eq!(metadata.kind, EntryKind::File);

		// Test exists
		assert!(backend.exists(Path::new("test.txt")).await.unwrap());
	}
}
