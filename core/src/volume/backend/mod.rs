//! Volume backend implementations
//!
//! This module provides the backend abstraction layer for heterogeneous storage I/O.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use std::fmt::Debug;
use std::ops::Range;
use std::path::Path;
use std::time::SystemTime;

use crate::ops::indexing::state::EntryKind;
use crate::volume::error::VolumeError;

pub mod cloud;
pub mod local;

pub use cloud::CloudBackend;
pub use local::LocalBackend;

/// Minimal I/O backend trait for volume operations
///
/// This trait provides only low-level filesystem operations. All domain logic
/// (Entry creation, content identification, etc.) is handled by existing
/// Spacedrive infrastructure that consumes these raw operations.
#[async_trait]
pub trait VolumeBackend: Send + Sync + Debug {
	/// Read entire file content
	async fn read(&self, path: &Path) -> Result<Bytes, VolumeError>;

	/// Read specific byte range from file (critical for cloud efficiency)
	async fn read_range(&self, path: &Path, range: Range<u64>) -> Result<Bytes, VolumeError>;

	/// Write file content
	async fn write(&self, path: &Path, data: Bytes) -> Result<(), VolumeError>;

	/// List directory entries (returns minimal metadata)
	async fn read_dir(&self, path: &Path) -> Result<Vec<RawDirEntry>, VolumeError>;

	/// Get file/directory metadata
	async fn metadata(&self, path: &Path) -> Result<RawMetadata, VolumeError>;

	/// Check if path exists (optimized when possible)
	async fn exists(&self, path: &Path) -> Result<bool, VolumeError>;

	/// Delete file or directory
	async fn delete(&self, path: &Path) -> Result<(), VolumeError>;

	/// Create a directory at the specified path
	async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError>;

	/// Whether [`VolumeBackend::write_if`] is enforced by the service.
	///
	/// A backend that cannot refuse a write against a changed object answers
	/// false, and agents treat its sources as read-only rather than risk an
	/// overwrite nobody asked for.
	fn conditional_writes(&self) -> bool {
		false
	}

	/// Write only when the object at `path` is in the state `condition`
	/// names. Answers the new object's entity tag when the service reports
	/// one.
	///
	/// Returns [`VolumeError::PreconditionFailed`] when the service refused
	/// the write because the object changed, appeared or vanished.
	async fn write_if(
		&self,
		path: &Path,
		_data: Bytes,
		_condition: WriteCondition,
	) -> Result<Option<String>, VolumeError> {
		Err(VolumeError::Unsupported(format!(
			"conditional writes to {}",
			path.display()
		)))
	}

	/// The entity tag of the object at `path`, or `None` when nothing is
	/// there. The tag is what [`WriteCondition::Matches`] names.
	async fn entity_tag(&self, path: &Path) -> Result<Option<String>, VolumeError> {
		Err(VolumeError::Unsupported(format!(
			"entity tags for {}",
			path.display()
		)))
	}

	/// Read the whole object only while its entity tag is still `etag`, so
	/// the bytes returned are the version the tag names.
	async fn read_if_match(&self, path: &Path, _etag: &str) -> Result<Bytes, VolumeError> {
		Err(VolumeError::Unsupported(format!(
			"conditional reads of {}",
			path.display()
		)))
	}

	/// Stream the object only while its entity tag is still `etag`. A change
	/// that lands during the stream fails it with
	/// [`VolumeError::PreconditionFailed`], so the chunks are all from the
	/// version the tag names.
	async fn stream_if_match(
		&self,
		path: &Path,
		_etag: &str,
	) -> Result<BoxStream<'static, Result<Bytes, VolumeError>>, VolumeError> {
		Err(VolumeError::Unsupported(format!(
			"conditional reads of {}",
			path.display()
		)))
	}

	/// Backend identification (used to optimize operations)
	fn is_local(&self) -> bool;

	/// Get backend type identifier
	fn backend_type(&self) -> BackendType;
}

/// What a conditional write requires of the object it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteCondition {
	/// An object exists at the path and its entity tag is this one.
	Matches(String),
	/// Nothing exists at the path.
	Absent,
}

/// Backend type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
	Local,
	Cloud(CloudServiceType),
}

/// Cloud service type identifier
#[derive(
	Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum CloudServiceType {
	#[serde(rename = "s3")]
	S3,
	#[serde(rename = "gdrive")]
	GoogleDrive,
	#[serde(rename = "dropbox")]
	Dropbox,
	#[serde(rename = "onedrive")]
	OneDrive,
	#[serde(rename = "gcs")]
	GoogleCloudStorage,
	#[serde(rename = "azblob")]
	AzureBlob,
	#[serde(rename = "b2")]
	BackblazeB2,
	#[serde(rename = "wasabi")]
	Wasabi,
	#[serde(rename = "spaces")]
	DigitalOceanSpaces,
	#[serde(rename = "cloud")]
	Other,
}

impl CloudServiceType {
	/// Get the URI scheme for this cloud service
	/// Used for service-native addressing (e.g., "s3://bucket/path")
	pub fn scheme(&self) -> &'static str {
		match self {
			Self::S3 => "s3",
			Self::GoogleDrive => "gdrive",
			Self::OneDrive => "onedrive",
			Self::Dropbox => "dropbox",
			Self::AzureBlob => "azblob",
			Self::GoogleCloudStorage => "gcs",
			Self::BackblazeB2 => "b2",
			Self::Wasabi => "wasabi",
			Self::DigitalOceanSpaces => "spaces",
			Self::Other => "cloud",
		}
	}

	/// Parse cloud service type from URI scheme
	/// Returns None if the scheme doesn't match any known service
	pub fn from_scheme(scheme: &str) -> Option<Self> {
		match scheme {
			"s3" => Some(Self::S3),
			"gdrive" => Some(Self::GoogleDrive),
			"onedrive" => Some(Self::OneDrive),
			"dropbox" => Some(Self::Dropbox),
			"azblob" => Some(Self::AzureBlob),
			"gcs" => Some(Self::GoogleCloudStorage),
			"b2" => Some(Self::BackblazeB2),
			"wasabi" => Some(Self::Wasabi),
			"spaces" => Some(Self::DigitalOceanSpaces),
			_ => None,
		}
	}
}

/// Raw directory entry returned by volume backends
///
/// Carries the same timestamps as [`RawMetadata`] because a backend that lists
/// a directory has already stat'd every entry in it. Reading them here costs
/// nothing and is the only chance: birth time cannot be recovered once a file
/// has been copied.
#[derive(Debug, Clone)]
pub struct RawDirEntry {
	pub name: String,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub created: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
	pub inode: Option<u64>,
	/// Unix permission bits (mode), None for cloud backends or Windows
	pub permissions: Option<u32>,
	/// Unix owner, None for cloud backends or Windows
	pub uid: Option<u32>,
	pub gid: Option<u32>,
	/// Where a symlink points, verbatim from `readlink`
	pub link_target: Option<String>,
}

/// Raw metadata returned by volume backends
#[derive(Debug, Clone)]
pub struct RawMetadata {
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub created: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
	pub inode: Option<u64>,
	/// Unix permission bits (mode), None for cloud backends or Windows
	pub permissions: Option<u32>,
	/// Unix owner, None for cloud backends or Windows
	pub uid: Option<u32>,
	pub gid: Option<u32>,
	/// Where a symlink points, verbatim from `readlink`
	pub link_target: Option<String>,
}
