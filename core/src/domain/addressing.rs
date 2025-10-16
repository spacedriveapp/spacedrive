//! Core addressing data structures for the Virtual Distributed File System
//!
//! This module contains the fundamental "nouns" of the addressing system -
//! the data structures that represent paths in Spacedrive's distributed
//! file system.

use crate::device::{get_current_device_id, get_current_device_slug};
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
		/// The device slug (e.g., "jamies-macbook")
		device_slug: String,
		/// The local path on that device
		path: PathBuf,
	},
	/// A cloud storage path within a cloud volume
	Cloud {
		/// The cloud service type (S3, GoogleDrive, etc.)
		service: crate::volume::backend::CloudServiceType,
		/// The cloud identifier (bucket name, drive name, etc.)
		identifier: String,
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
			device_slug: String,
			path: String,
		}

		#[derive(Deserialize)]
		struct SdPathCloudHelper {
			service: String,
			identifier: String,
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
			SdPathHelper::Physical { Physical: physical } => Ok(SdPath::Physical {
				device_slug: physical.device_slug,
				path: PathBuf::from(physical.path),
			}),
			SdPathHelper::Cloud { Cloud: cloud } => {
				let service = crate::volume::backend::CloudServiceType::from_scheme(&cloud.service)
					.ok_or_else(|| {
						serde::de::Error::custom(format!(
							"Unknown cloud service: {}",
							cloud.service
						))
					})?;
				Ok(SdPath::Cloud {
					service,
					identifier: cloud.identifier,
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
	pub fn new(device_slug: String, path: impl Into<PathBuf>) -> Self {
		Self::physical(device_slug, path)
	}

	/// Create a physical SdPath with specific device and path
	pub fn physical(device_slug: String, path: impl Into<PathBuf>) -> Self {
		Self::Physical {
			device_slug,
			path: path.into(),
		}
	}

	/// Create a cloud storage SdPath
	pub fn cloud(
		service: crate::volume::backend::CloudServiceType,
		identifier: String,
		path: impl Into<String>,
	) -> Self {
		Self::Cloud {
			service,
			identifier,
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
			device_slug: get_current_device_slug(),
			path: path.into(),
		}
	}

	/// Check if this path is on the current device
	pub fn is_local(&self) -> bool {
		match self {
			Self::Physical { device_slug, .. } => *device_slug == get_current_device_slug(),
			Self::Cloud { .. } => false,   // Cloud paths are never local
			Self::Content { .. } => false, // Content paths are abstract, not inherently local
		}
	}

	/// Get the local PathBuf if this is a local path
	pub fn as_local_path(&self) -> Option<&Path> {
		match self {
			Self::Physical { device_slug, path } => {
				if *device_slug == get_current_device_slug() {
					Some(path)
				} else {
					None
				}
			}
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Convert to a display string in unified addressing format
	/// This uses the identity-based format with no manager lookups needed
	pub fn display(&self) -> String {
		match self {
			Self::Physical { device_slug, path } => {
				format!("local://{}{}", device_slug, path.display())
			}
			Self::Cloud {
				service,
				identifier,
				path,
			} => {
				format!("{}://{}/{}", service.scheme(), identifier, path)
			}
			Self::Content { content_id } => {
				format!("content://{}", content_id)
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
			Self::Physical { device_slug, path } => path.parent().map(|p| Self::Physical {
				device_slug: device_slug.clone(),
				path: p.to_path_buf(),
			}),
			Self::Cloud {
				service,
				identifier,
				path,
			} => {
				let parent_path = path.trim_end_matches('/');
				parent_path.rfind('/').map(|idx| Self::Cloud {
					service: *service,
					identifier: identifier.clone(),
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
				device_slug,
				path: base_path,
			} => Self::Physical {
				device_slug: device_slug.clone(),
				path: base_path.join(path),
			},
			Self::Cloud {
				service,
				identifier,
				path: base_path,
			} => {
				let path_str = path.as_ref().to_string_lossy();
				let separator = if base_path.ends_with('/') || path_str.starts_with('/') {
					""
				} else {
					"/"
				};
				Self::Cloud {
					service: *service,
					identifier: identifier.clone(),
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
				service,
				identifier,
				..
			} => {
				// Look up cloud volume by identity
				volume_manager.find_cloud_volume(*service, identifier).await
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
					service: s1,
					identifier: id1,
					..
				},
				Self::Cloud {
					service: s2,
					identifier: id2,
					..
				},
			) => {
				// Cloud paths are on the same volume if they have the same service and identifier
				s1 == s2 && id1 == id2
			}
			_ => false, // Content paths or mixed types can't be compared for volume
		}
	}

	/// Parse an SdPath from a URI string (unified addressing format)
	/// Examples:
	/// - "local://device-slug/path/to/file" -> Physical path
	/// - "s3://bucket/path/to/file" -> Cloud path
	/// - "content://content_id" -> Content path
	/// - "/local/path" -> Local physical path
	///
	/// Note: This is a synchronous version that doesn't require context.
	/// For resolving slugs/identifiers to actual volumes/devices, use from_uri_with_context()
	pub fn from_uri(uri: &str) -> Result<Self, SdPathParseError> {
		let parts: Vec<&str> = uri.splitn(2, "://").collect();

		if parts.len() != 2 {
			// No scheme - assume local path
			return Ok(Self::local(uri));
		}

		let scheme = parts[0];
		let rest = parts[1];

		match scheme {
			"content" => {
				let content_id =
					Uuid::parse_str(rest).map_err(|_| SdPathParseError::InvalidContentId)?;
				Ok(Self::Content { content_id })
			}

			"local" => {
				let parts: Vec<&str> = rest.splitn(2, '/').collect();
				let slug = parts[0].to_string();
				let path = if parts.len() > 1 { parts[1] } else { "" };

				Ok(Self::Physical {
					device_slug: slug,
					path: PathBuf::from("/").join(path),
				})
			}

			_ => {
				// Try to parse as cloud service scheme
				let service = crate::volume::backend::CloudServiceType::from_scheme(scheme)
					.ok_or(SdPathParseError::UnknownScheme)?;

				let parts: Vec<&str> = rest.splitn(2, '/').collect();
				let identifier = parts[0].to_string();
				let path = if parts.len() > 1 {
					parts[1].to_string()
				} else {
					String::new()
				};

				Ok(Self::Cloud {
					service,
					identifier,
					path,
				})
			}
		}
	}

	/// Parse URI into SdPath with context validation (kept for backwards compatibility)
	///
	/// # Examples
	/// - "local://jamies-macbook/Users/james/file.txt" -> Physical path
	/// - "s3://my-bucket/photos/vacation.jpg" -> Cloud path
	/// - "content://550e8400-..." -> Content path
	///
	/// Note: This now simply delegates to from_uri() since identities are stored directly.
	/// Context can still be used for validation if needed in the future.
	pub async fn from_uri_with_context(
		uri: &str,
		_context: &crate::context::CoreContext,
	) -> Result<Self, SdPathParseError> {
		Self::from_uri(uri)
	}

	/// Convert to a URI string
	pub fn to_uri(&self) -> String {
		self.display()
	}

	/// Get the device slug if this is a Physical path
	pub fn device_slug(&self) -> Option<&str> {
		match self {
			Self::Physical { device_slug, .. } => Some(device_slug),
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Legacy method - get device ID (deprecated, use device_slug instead)
	#[deprecated(note = "Use device_slug() instead")]
	pub fn device_id(&self) -> Option<Uuid> {
		// Return nil UUID - this method is deprecated
		None
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

	/// Get the cloud service and identifier if this is a Cloud path
	pub fn cloud_identity(&self) -> Option<(crate::volume::backend::CloudServiceType, &str)> {
		match self {
			Self::Cloud {
				service,
				identifier,
				..
			} => Some((*service, identifier.as_str())),
			Self::Physical { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Legacy method - get volume fingerprint (deprecated, use cloud_identity instead)
	#[deprecated(note = "Use cloud_identity() instead")]
	pub fn volume_fingerprint(&self) -> Option<&crate::volume::VolumeFingerprint> {
		// This method is deprecated - return None
		None
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

	/// Try to get as a Physical path, returning device_slug and path
	pub fn as_physical(&self) -> Option<(&str, &PathBuf)> {
		match self {
			Self::Physical { device_slug, path } => Some((device_slug.as_str(), path)),
			Self::Cloud { .. } => None,
			Self::Content { .. } => None,
		}
	}

	/// Try to get as a Cloud path, returning service, identifier, and path
	pub fn as_cloud(&self) -> Option<(crate::volume::backend::CloudServiceType, &str, &str)> {
		match self {
			Self::Cloud {
				service,
				identifier,
				path,
			} => Some((*service, identifier.as_str(), path.as_str())),
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
	UnknownScheme,  // NEW: Unknown URI scheme
	VolumeNotFound, // NEW: Cloud volume not found
	DeviceNotFound, // NEW: Device slug not found
}

impl fmt::Display for SdPathParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidFormat => write!(f, "Invalid SdPath URI format"),
			Self::InvalidDeviceId => write!(f, "Invalid device ID in SdPath URI"),
			Self::InvalidVolumeId => write!(f, "Invalid volume ID in SdPath URI"),
			Self::InvalidContentId => write!(f, "Invalid content ID in SdPath URI"),
			Self::UnknownScheme => write!(f, "Unknown URI scheme"),
			Self::VolumeNotFound => write!(f, "Cloud volume not found"),
			Self::DeviceNotFound => write!(f, "Device not found by slug"),
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

	/// Group by device slug
	pub fn by_device(&self) -> std::collections::HashMap<String, Vec<&SdPath>> {
		let mut map = std::collections::HashMap::new();
		for path in &self.paths {
			if let Some(device_slug) = path.device_slug() {
				map.entry(device_slug.to_string())
					.or_insert_with(Vec::new)
					.push(path);
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
		let device_slug = "test-device-abc123".to_string();
		let path = SdPath::new(device_slug.clone(), "/home/user/file.txt");

		match path {
			SdPath::Physical {
				device_slug: slug,
				path: p,
			} => {
				assert_eq!(slug, device_slug);
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
		let device_slug = "test-device-abc123".to_string();
		let path = SdPath::new(device_slug.clone(), "/home/user/file.txt");

		let display = path.display();
		assert!(display.contains(&device_slug));
		assert!(display.contains("/home/user/file.txt"));
		assert!(display.starts_with("local://"));
	}

	#[test]
	fn test_sdpath_uri_parsing() {
		// Test content URI
		let content_id = Uuid::new_v4();
		let uri = format!("content://{}", content_id);
		let path = SdPath::from_uri(&uri).unwrap();
		match path {
			SdPath::Content { content_id: cid } => assert_eq!(cid, content_id),
			_ => panic!("Expected Content variant"),
		}

		// Test physical URI
		let device_slug = "test-device-abc123";
		let uri = format!("local://{}/home/user/file.txt", device_slug);
		let path = SdPath::from_uri(&uri).unwrap();
		match path {
			SdPath::Physical {
				device_slug: slug,
				path: p,
			} => {
				assert_eq!(slug, device_slug);
				assert_eq!(p, PathBuf::from("/home/user/file.txt"));
			}
			_ => panic!("Expected Physical variant"),
		}

		// Test cloud URI
		let uri = "s3://my-bucket/photos/vacation.jpg";
		let path = SdPath::from_uri(uri).unwrap();
		match path {
			SdPath::Cloud {
				service,
				identifier,
				path,
			} => {
				assert_eq!(service.scheme(), "s3");
				assert_eq!(identifier, "my-bucket");
				assert_eq!(path, "photos/vacation.jpg");
			}
			_ => panic!("Expected Cloud variant"),
		}

		// Test local path without scheme
		let path = SdPath::from_uri("/local/path").unwrap();
		assert!(path.is_local());
	}
}
