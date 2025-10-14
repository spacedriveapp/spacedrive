//! Volume backend implementations
//!
//! This module provides the backend abstraction layer for heterogeneous storage I/O.

use async_trait::async_trait;
use bytes::Bytes;
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

	/// Backend identification (used to optimize operations)
	fn is_local(&self) -> bool;

	/// Get backend type identifier
	fn backend_type(&self) -> BackendType;
}

/// Backend type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
	Local,
	Cloud(CloudServiceType),
}

/// Cloud service type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum CloudServiceType {
	S3,
	GoogleDrive,
	Dropbox,
	OneDrive,
	GoogleCloudStorage,
	AzureBlob,
	BackblazeB2,
	Wasabi,
	DigitalOceanSpaces,
	Other,
}

/// Raw directory entry returned by volume backends
#[derive(Debug, Clone)]
pub struct RawDirEntry {
	pub name: String,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub inode: Option<u64>,
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
}
