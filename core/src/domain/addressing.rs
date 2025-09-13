//! Core addressing data structures for the Virtual Distributed File System
//!
//! This module contains the fundamental "nouns" of the addressing system -
//! the data structures that represent paths in Spacedrive's distributed
//! file system.

use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SdPath {
	/// A direct pointer to a file at a specific path on a specific device
	Physical {
		/// The device where this file exists
		device_id: Uuid,
		/// The local path on that device
		path: PathBuf,
	},
	/// An abstract, location-independent handle that refers to file content
	Content {
		/// The unique content identifier
		content_id: Uuid,
	},
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
			Self::Content { content_id } => {
				format!("sd://content/{}", content_id)
			}
		}
	}

	/// Get just the file name
	pub fn file_name(&self) -> Option<&str> {
		match self {
			Self::Physical { path, .. } => path.file_name()?.to_str(),
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
			_ => false, // Content paths or mixed types can't be compared for volume
		}
	}

	/// Parse an SdPath from a URI string
	/// Examples:
	/// - "sd://device_id/path/to/file" -> Physical path
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
			Self::Content { .. } => None,
		}
	}

	/// Get the path if this is a Physical path
	pub fn path(&self) -> Option<&PathBuf> {
		match self {
			Self::Physical { path, .. } => Some(path),
			Self::Content { .. } => None,
		}
	}

	/// Get the content ID if this is a Content path
	pub fn content_id(&self) -> Option<Uuid> {
		match self {
			Self::Content { content_id } => Some(*content_id),
			Self::Physical { .. } => None,
		}
	}

	/// Check if this is a Physical path
	pub fn is_physical(&self) -> bool {
		matches!(self, Self::Physical { .. })
	}

	/// Check if this is a Content path
	pub fn is_content(&self) -> bool {
		matches!(self, Self::Content { .. })
	}

	/// Try to get as a Physical path, returning device_id and path
	pub fn as_physical(&self) -> Option<(Uuid, &PathBuf)> {
		match self {
			Self::Physical { device_id, path } => Some((*device_id, path)),
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
		// For now, if it's already physical, just return it
		// TODO: Implement proper resolution using job context's library and networking
		match self {
			Self::Physical { .. } => Ok(self.clone()),
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
	InvalidContentId,
}

impl fmt::Display for SdPathParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidFormat => write!(f, "Invalid SdPath URI format"),
			Self::InvalidDeviceId => write!(f, "Invalid device ID in SdPath URI"),
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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
