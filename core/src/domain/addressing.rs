//! Core addressing data structures for the Virtual Distributed File System
//!
//! This module contains the fundamental "nouns" of the addressing system -
//! the data structures that represent paths in Spacedrive's distributed
//! file system.

use crate::device::get_current_device_id;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// A path within the Spacedrive Virtual Distributed File System
///
/// This is the core abstraction that enables cross-device operations.
/// An SdPath can represent:
/// - A physical file at a specific path on a specific device
/// - A content-addressed file that can be sourced from any device
///
/// This enum-based approach enables resilient file operations by allowing
/// content-based paths to be resolved to optimal physical locations at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Type)]
pub enum SdPath {
	/// A direct pointer to a file at a specific path on a specific device
	Physical {
		/// The device where this file exists
		device_id: Uuid,
		/// The local path on that device
		path: PathBuf,
	},
	/// A cloud storage path within a cloud volume
	Cloud {
		/// The cloud volume fingerprint for direct HashMap lookup
		volume_fingerprint: crate::volume::VolumeFingerprint,
		/// The cloud-native path (e.g., "bucket/key" for S3)
		path: String,
	},
	/// An abstract, location-independent handle that refers to file content
	Content {
		/// The unique content identifier
		content_id: Uuid,
	},
}

impl<'de> Deserialize<'de> for SdPath {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct SdPathPhysicalHelper {
			device_id: String,
			path: String,
		}

		#[derive(Deserialize)]
		struct SdPathCloudHelper {
			volume_fingerprint: String,
			path: String,
		}

		#[derive(Deserialize)]
		struct SdPathContentHelper {
			content_id: String,
		}

		#[derive(Deserialize)]
		#[serde(untagged)]
		enum SdPathHelper {
			Physical { Physical: SdPathPhysicalHelper },
			Cloud { Cloud: SdPathCloudHelper },
			Content { Content: SdPathContentHelper },
		}

		let helper = SdPathHelper::deserialize(deserializer)?;

		match helper {
			SdPathHelper::Physical { Physical: physical } => {
				// Useful helper for clients to avoid passing the device id if it's the local device
				let device_id = if physical.device_id == "local-device" {
					// Use the global current device ID for the local Mac device
					get_current_device_id()
				} else {
					Uuid::parse_str(&physical.device_id).map_err(serde::de::Error::custom)?
				};
				Ok(SdPath::Physical {
					device_id,
					path: PathBuf::from(physical.path),
				})
			}
			SdPathHelper::Cloud { Cloud: cloud } => {
				let volume_fingerprint = crate::volume::VolumeFingerprint(cloud.volume_fingerprint);
				Ok(SdPath::Cloud {
					volume_fingerprint,
					path: cloud.path,
				})
			}
			SdPathHelper::Content { Content: content } => {
				let content_id =
					Uuid::parse_str(&content.content_id).map_err(serde::de::Error::custom)?;
				Ok(SdPath::Content { content_id })
			}
		}
	}
}

impl SdPath {
	/// Create a new physical SdPath
	pub fn new(device_id: Uuid, path: impl Into<PathBuf>) -> Self {
		Self::physical(device_id, path)
	}

	/// Create a physical SdPath with specific device and path
	pub fn physical(device_id: Uuid, path: impl Into<PathBuf>) -> Self {
		Self::Physical {
			device_id,
			path: path.into(),
		}
	}

	/// Create a cloud storage SdPath
	pub fn cloud(
		volume_fingerprint: crate::volume::VolumeFingerprint,
		path: impl Into<String>,
	) -> Self {
		Self::Cloud {
			volume_fingerprint,
			path: path.into(),
		}
	}

	/// Create a content-addressed SdPath
	pub fn content(content_id: Uuid) -> Self {
		Self::Content { content_id }
	}

	/// Create an SdPath for a local file on this device
	pub fn local(path: impl Into<PathBuf>) -> Self {
		Self::Physical {
			device_id: crate::device::get_current_device_id(),
			path: path.into(),
		}
	}

	/// Check if this path is on the current device
	pub fn is_local(&self) -> bool {
		match self {
			Self::Physical { device_id, .. } => {
				*device_id == crate::device::get_current_device_id()
			}
			Self::Cloud { .. } => false,   // Cloud paths are never local
			Self::Content { .. } => false, // Content paths are abstract, not inherently local
		}
	}

	/// Get the local PathBuf if this is a local path
	pub fn as_local_path(&self) -> Option<&Path> {
		match self {
			Self::Physical { device_id, path } => {
				if *device_id == crate::device::get_current_device_id() {
					Some(path)
				} else {
					None
				}
			}
			Self::Cloud { .. } => None, // Cloud paths don't have local paths
			Self::Content { .. } => None,
		}
	}

	/// Convert to a display string
	pub fn display(&self) -> String {
		match self {
			Self::Physical { device_id, path } => {
				if *device_id == crate::device::get_current_device_id() {
					path.display().to_string()
				} else {
					format!("sd://{}/{}", device_id, path.display())
				}
			}
			Self::Cloud {
				volume_fingerprint,
				path,
			} => {
				format!("sd://cloud/{}/{}", volume_fingerprint.0, path)
			}
			Self::Content { content_id } => {
				format!("sd://content/{}", content_id)
			}
		}
	}

	/// Get just the file name
	pub fn file_name(&self) -> Option<&str> {
		match self {
			Self::Physical { path, .. } => path.file_name()?.to_str(),
			Self::Cloud { path, .. } => path.split('/').last(),
			Self::Content { .. } => None, // Content paths don't have filenames
		}
	}

	/// Get the parent directory as an SdPath
	pub fn parent(&self) -> Option<SdPath> {
		match self {
			Self::Physical { device_id, path } => path.parent().map(|p| Self::Physical {
				device_id: *device_id,
				path: p.to_path_buf(),
			}),
			Self::Cloud {
				volume_fingerprint,
				path,
			} => {
				let parent_path = path.trim_end_matches('/');
				parent_path.rfind('/').map(|idx| Self::Cloud {
					volume_fingerprint: volume_fingerprint.clone(),
					path: parent_path[..idx].to_string(),
				})
			}
			Self::Content { .. } => None, // Content paths don't have parents
		}
	}

	/// Join with another path component
	/// Panics if called on a Content variant
	pub fn join(&self, path: impl AsRef<Path>) -> SdPath {
		match self {
			Self::Physical {
				device_id,
				path: base_path,
			} => Self::Physical {
				device_id: *device_id,
				path: base_path.join(path),
			},
			Self::Cloud {
				volume_fingerprint,
				path: base_path,
			} => {
				let path_str = path.as_ref().to_string_lossy();
				let separator = if base_path.ends_with('/') || path_str.starts_with('/') {
					""
				} else {
					"/"
				};
				Self::Cloud {
					volume_fingerprint: volume_fingerprint.clone(),
					path: format!("{}{}{}", base_path, separator, path_str),
				}
			}
			Self::Content { .. } => panic!("Cannot join paths to content addresses"),
		}
	}

	/// Get the volume that contains this path (if local and volume manager available)
	pub async fn get_volume(
		&self,
		volume_manager: &crate::volume::VolumeManager,
	) -> Option<crate::volume::Volume> {
		match self {
			Self::Physical { .. } => {
				if let Some(local_path) = self.as_local_path() {
					volume_manager.volume_for_path(local_path).await
				} else {
					None
				}
			}
			Self::Cloud {
				volume_fingerprint, ..
			} => {
				// Look up cloud volume by fingerprint
				volume_manager.get_volume(volume_fingerprint).await
			}
			Self::Content { .. } => None, // Content paths don't have volumes until resolved
		}
	}

	/// Check if this path is on the same volume as another path
	pub async fn same_volume(
		&self,
		other: &SdPath,
		volume_manager: &crate::volume::VolumeManager,
	) -> bool {
		match (self, other) {
			(Self::Physical { .. }, Self::Physical { .. }) => {
				if !self.is_local() || !other.is_local() {
					return false;
				}

				if let (Some(self_path), Some(other_path)) =
					(self.as_local_path(), other.as_local_path())
				{
					volume_manager.same_volume(self_path, other_path).await
				} else {
					false
				}
			}
			(
				Self::Cloud {
					volume_fingerprint: fp1,
					..
				},
				Self::Cloud {
					volume_fingerprint: fp2,
					..
				},
			) => {
				// Cloud paths are on the same volume if they have the same fingerprint
				fp1 == fp2
			}
			_ => false, // Content paths or mixed types can't be compared for volume
		}
	}

	/// Parse an SdPath from a URI string
	/// Examples:
	/// - "sd://device_id/path/to/file" -> Physical path
	/// - "sd://cloud/volume_id/path/to/file" -> Cloud path
	/// - "sd://content/content_id" -> Content path
	/// - "/local/path" -> Local physical path
	pub fn from_uri(uri: &str) -> Result<Self, SdPathParseError> {
		if uri.starts_with("sd://") {
			let uri = &uri[5..]; // Strip "sd://"

			if let Some(content_id_str) = uri.strip_prefix("content/") {
				// Parse content path
				let content_id = Uuid::parse_str(content_id_str)
					.map_err(|_| SdPathParseError::InvalidContentId)?;
				Ok(Self::Content { content_id })
			} else if let Some(cloud_path) = uri.strip_prefix("cloud/") {
				// Parse cloud path
				let parts: Vec<&str> = cloud_path.splitn(2, '/').collect();
				if parts.is_empty() {
					return Err(SdPathParseError::InvalidFormat);
				}

				// Parse fingerprint as a hex string
				let volume_fingerprint = crate::volume::VolumeFingerprint(parts[0].to_string());
				let path = parts.get(1).unwrap_or(&"").to_string();

				Ok(Self::Cloud {
					volume_fingerprint,
					path,
				})
			} else {
				// Parse physical path
				let parts: Vec<&str> = uri.splitn(2, '/').collect();
				if parts.len() != 2 {
					return Err(SdPathParseError::InvalidFormat);
				}

				let device_id =
					Uuid::parse_str(parts[0]).map_err(|_| SdPathParseError::InvalidDeviceId)?;
				let path = PathBuf::from("/").join(parts[1]);

				Ok(Self::Physical { device_id, path })
			}
		} else {
			// Assume local path
			Ok(Self::local(uri))
		}
	}

	/// Convert to a URI string
	pub fn to_uri(&self) -> String {
		self.display()
	}

	/// Get the device ID if this is a Physical path
	pub fn device_id(&self) -> Option<Uuid> {
		match self {
			Self::Physical { device_id, .. } => Some(*device_id),
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Get the path if this is a Physical path
	pub fn path(&self) -> Option<&PathBuf> {
		match self {
			Self::Physical { path, .. } => Some(path),
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Get the content ID if this is a Content path
	pub fn content_id(&self) -> Option<Uuid> {
		match self {
			Self::Content { content_id } => Some(*content_id),
			Self::Physical { .. } => None,
			Self::Cloud { .. } => None,
		}
	}

	/// Get the volume fingerprint if this is a Cloud path
	pub fn volume_fingerprint(&self) -> Option<&crate::volume::VolumeFingerprint> {
		match self {
			Self::Cloud {
				volume_fingerprint, ..
			} => Some(volume_fingerprint),
			Self::Physical { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Get the cloud path if this is a Cloud path
	pub fn cloud_path(&self) -> Option<&str> {
		match self {
			Self::Cloud { path, .. } => Some(path),
			Self::Physical { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Check if this is a Physical path
	pub fn is_physical(&self) -> bool {
		matches!(self, Self::Physical { .. })
	}

	/// Check if this is a Cloud path
	pub fn is_cloud(&self) -> bool {
		matches!(self, Self::Cloud { .. })
	}

	/// Check if this is a Content path
	pub fn is_content(&self) -> bool {
		matches!(self, Self::Content { .. })
	}

	/// Try to get as a Physical path, returning device_id and path
	pub fn as_physical(&self) -> Option<(Uuid, &PathBuf)> {
		match self {
			Self::Physical { device_id, path } => Some((*device_id, path)),
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Try to get as a Cloud path, returning volume_fingerprint and path
	pub fn as_cloud(&self) -> Option<(&crate::volume::VolumeFingerprint, &str)> {
		match self {
			Self::Cloud {
				volume_fingerprint,
				path,
			} => Some((volume_fingerprint, path)),
			Self::Physical { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Resolve this path to an optimal physical location
	/// This is the entry point for path resolution that will use the PathResolver service
	pub async fn resolve(
		&self,
		context: &crate::context::CoreContext,
	) -> Result<SdPath, PathResolutionError> {
		let resolver = crate::ops::addressing::PathResolver;
		resolver.resolve(self, context).await
	}

	/// Resolve this path using a JobContext
	pub async fn resolve_in_job<'a>(
		&self,
		job_ctx: &crate::infra::job::context::JobContext<'a>,
	) -> Result<SdPath, PathResolutionError> {
		// For now, if it's already physical or cloud, just return it
		// TODO: Implement proper resolution using job context's library and networking
		match self {
			Self::Physical { .. } => Ok(self.clone()),
			Self::Cloud { .. } => Ok(self.clone()), // Cloud paths are already resolved
			Self::Content { content_id } => {
				// In the future, use job_ctx.library_db() to query for content instances
				Err(PathResolutionError::NoOnlineInstancesFound(*content_id))
			}
		}
	}
}

/// Error type for path resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolutionError {
	NoOnlineInstancesFound(Uuid),
	DeviceOffline(Uuid),
	NoActiveLibrary,
	DatabaseError(String),
}

impl fmt::Display for PathResolutionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoOnlineInstancesFound(id) => {
				write!(f, "No online instances found for content: {}", id)
			}
			Self::DeviceOffline(id) => write!(f, "Device is offline: {}", id),
			Self::NoActiveLibrary => write!(f, "No active library"),
			Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
		}
	}
}

impl std::error::Error for PathResolutionError {}

impl From<sea_orm::DbErr> for PathResolutionError {
	fn from(err: sea_orm::DbErr) -> Self {
		PathResolutionError::DatabaseError(err.to_string())
	}
}

/// Error type for SdPath parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdPathParseError {
	InvalidFormat,
	InvalidDeviceId,
	InvalidVolumeId,
	InvalidContentId,
}

impl fmt::Display for SdPathParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidFormat => write!(f, "Invalid SdPath URI format"),
			Self::InvalidDeviceId => write!(f, "Invalid device ID in SdPath URI"),
			Self::InvalidVolumeId => write!(f, "Invalid volume ID in SdPath URI"),
			Self::InvalidContentId => write!(f, "Invalid content ID in SdPath URI"),
		}
	}
}

impl std::error::Error for SdPathParseError {}

impl fmt::Display for SdPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.display())
	}
}

/// A batch of SdPaths, useful for operations on multiple files
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Type)]
pub struct SdPathBatch {
	pub paths: Vec<SdPath>,
}

impl SdPathBatch {
	/// Create a new batch
	pub fn new(paths: Vec<SdPath>) -> Self {
		Self { paths }
	}

	/// Filter to only local paths
	pub fn local_only(&self) -> Vec<&Path> {
		self.paths
			.iter()
			.filter_map(|p| p.as_local_path())
			.collect()
	}

	/// Group by device
	pub fn by_device(&self) -> std::collections::HashMap<Uuid, Vec<&SdPath>> {
		let mut map = std::collections::HashMap::new();
		for path in &self.paths {
			if let Some(device_id) = path.device_id() {
				map.entry(device_id).or_insert_with(Vec::new).push(path);
			}
		}
		map
	}

	/// add multiple paths
	pub fn extend(&mut self, paths: Vec<SdPath>) {
		self.paths.extend(paths);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sdpath_physical_creation() {
		let device_id = Uuid::new_v4();
		let path = SdPath::new(device_id, "/home/user/file.txt");

		match path {
			SdPath::Physical {
				device_id: did,
				path: p,
			} => {
				assert_eq!(did, device_id);
				assert_eq!(p, PathBuf::from("/home/user/file.txt"));
			}
			_ => panic!("Expected Physical variant"),
		}
	}

	#[test]
	fn test_sdpath_content_creation() {
		let content_id = Uuid::new_v4();
		let path = SdPath::content(content_id);

		match path {
			SdPath::Content { content_id: cid } => {
				assert_eq!(cid, content_id);
			}
			_ => panic!("Expected Content variant"),
		}
	}

	#[test]
	fn test_sdpath_display() {
		let device_id = Uuid::new_v4();
		let path = SdPath::new(device_id, "/home/user/file.txt");

		let display = path.display();
		assert!(display.contains(&device_id.to_string()));
		assert!(display.contains("/home/user/file.txt"));
	}

	#[test]
	fn test_sdpath_uri_parsing() {
		// Test content URI
		let content_id = Uuid::new_v4();
		let uri = format!("sd://content/{}", content_id);
		let path = SdPath::from_uri(&uri).unwrap();
		match path {
			SdPath::Content { content_id: cid } => assert_eq!(cid, content_id),
			_ => panic!("Expected Content variant"),
		}

		// Test physical URI
		let device_id = Uuid::new_v4();
		let uri = format!("sd://{}/home/user/file.txt", device_id);
		let path = SdPath::from_uri(&uri).unwrap();
		match path {
			SdPath::Physical {
				device_id: did,
				path: p,
			} => {
				assert_eq!(did, device_id);
				assert_eq!(p, PathBuf::from("/home/user/file.txt"));
			}
			_ => panic!("Expected Physical variant"),
		}

		// Test local path
		let path = SdPath::from_uri("/local/path").unwrap();
		assert!(path.is_local());
	}
}
