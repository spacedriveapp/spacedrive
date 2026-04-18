//! # Volume backend implementations
//!
//! `core::volume::backend` abstracts heterogeneous storage I/O behind a single
//! [`VolumeBackend`] trait so indexing, file operations, and the UI can treat
//! local disks and cloud providers uniformly. Feature flags exposed through
//! [`BackendFeatures`] let callers branch on provider-specific capabilities
//! (server-side copy, stable file IDs, delta change notifications) without
//! re-interrogating the underlying OpenDAL operator every call.

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

	/// Delete file or directory
	async fn delete(&self, path: &Path) -> Result<(), VolumeError>;

	/// Create a directory at the specified path
	async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError>;

	/// Backend identification (used to optimize operations)
	fn is_local(&self) -> bool;

	/// Get backend type identifier
	fn backend_type(&self) -> BackendType;

	/// Capability descriptor used by higher-level jobs to pick optimal paths.
	///
	/// Returning a `BackendFeatures` by value (rather than a reference) keeps
	/// the trait object-safe without forcing every backend to store a static
	/// copy, and the struct is cheap enough (a handful of booleans and small
	/// enums) that the copy is free at call sites.
	fn features(&self) -> BackendFeatures;

	/// Downcast helper to a cloud backend when the implementation is
	/// [`CloudBackend`], returning `None` for local backends. This lets
	/// `FileCopyJob` reach the underlying OpenDAL operator for streaming
	/// transfers without exposing a full `Any`-based downcast on every
	/// backend.
	fn as_cloud(&self) -> Option<&cloud::CloudBackend> {
		None
	}
}

/// Describes how a backend signals changes to the indexer.
///
/// Knowing this up front lets the indexing layer decide between cheap
/// incremental delta polling and expensive full rescans, which is the
/// difference between a responsive cloud volume and one that hammers provider
/// quotas on every refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ChangeNotificationKind {
	/// No change detection possible; the indexer must do a full rescan.
	None,
	/// Token-based delta polling (OneDrive Graph `/delta`, Google Drive `changes.list`).
	DeltaToken,
	/// HTTP long-polling (Dropbox `/longpoll`).
	LongPoll,
	/// Bucket-level event stream delivered out-of-band (S3 EventBridge, Azure Blob change feed).
	ChangeFeed,
}

/// Content hash algorithm a backend exposes natively.
///
/// When a provider returns a content hash in metadata, the indexer can skip
/// its own hashing pass for unchanged files, which dominates cost on cloud
/// volumes where downloading bytes just to hash them is unaffordable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum HashAlgorithm {
	/// MD5 (S3 ETag for single-part uploads, GCS `md5Hash`, Google Drive `md5Checksum`).
	Md5,
	/// SHA-1 (Backblaze B2 `content_sha1`).
	Sha1,
	/// SHA-256.
	Sha256,
	/// Microsoft's proprietary block-rolling hash used by OneDrive.
	QuickXor,
	/// CRC32 family (GCS `crc32c`).
	Crc32,
}

/// Capability descriptor for a concrete [`VolumeBackend`] implementation.
///
/// Downstream jobs read this struct to branch between fast-path provider
/// primitives and generic fallbacks. For example, `FileCopyJob` switches
/// between `operator.copy()` and a streaming read/write loop based on
/// [`BackendFeatures::server_side_copy`], and the indexer decides whether to
/// rehash a file based on [`BackendFeatures::content_hash`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BackendFeatures {
	/// Whether the backend can copy server-side without streaming bytes through
	/// the client. Flips `FileCopyJob` between `op.copy()` and a manual
	/// reader/writer pipe.
	pub server_side_copy: bool,

	/// Whether the backend can rename/move server-side in a single call. When
	/// false, higher layers must emulate rename via copy plus delete.
	pub server_side_rename: bool,

	/// Whether the backend exposes a stable file identifier that survives
	/// rename and parent-move. Controls whether the indexer preserves tags and
	/// notes across such moves, or must reconcile by path only.
	pub stable_file_id: bool,

	/// How (or whether) the backend surfaces incremental changes. Drives the
	/// choice between delta polling, long-polling, and full rescans.
	pub change_notifications: ChangeNotificationKind,

	/// Native content hash returned in metadata, if any. Lets the indexer skip
	/// its own hashing pass when the provider already delivers a reliable
	/// digest.
	pub content_hash: Option<HashAlgorithm>,

	/// Whether the backend is case-insensitive for path lookups. Affects
	/// deduplication logic: on case-insensitive stores the indexer must not
	/// treat `FOO.TXT` and `foo.txt` as distinct entries.
	pub case_insensitive: bool,

	/// Whether the backend permits multiple entries with the same name in the
	/// same directory. OneDrive does, some providers do not.
	pub supports_duplicates: bool,

	/// Hard per-file size ceiling enforced by the provider, in bytes. Used to
	/// fail large uploads early with a clear error instead of streaming gigs
	/// before the provider rejects the request.
	pub max_file_size: Option<u64>,

	/// Threshold at which the backend should switch from a single PUT to
	/// multipart / resumable uploads, in bytes. Below this, a single-shot
	/// write is faster and cheaper.
	pub multipart_threshold: Option<u64>,
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
#[derive(Debug, Clone)]
pub struct RawDirEntry {
	pub name: String,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub inode: Option<u64>,
}

/// Raw metadata returned by volume backends.
///
/// Fields beyond the obvious `size`/`modified` are populated only when the
/// backend can surface them cheaply. Cloud providers expose rich metadata
/// (ETag, version, content hash, native file IDs) that the indexer uses to
/// short-circuit expensive rehashing and to preserve identity across renames.
#[derive(Debug, Clone)]
pub struct RawMetadata {
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub created: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
	pub inode: Option<u64>,
	/// Unix permission bits (mode), None for cloud backends or Windows.
	pub permissions: Option<u32>,

	/// HTTP-style entity tag. Enables conditional GET and cheap change
	/// detection without downloading the object body. S3, Azure, GCS,
	/// OneDrive expose one; Google Drive and Dropbox do not.
	pub etag: Option<String>,

	/// Provider-assigned version identifier for object-versioned buckets.
	/// Lets Spacedrive pin reads to a specific revision and detect in-place
	/// rewrites that keep the ETag unchanged (rare but possible).
	pub version: Option<String>,

	/// Content-MD5 as surfaced by the provider. When present, the indexer can
	/// skip its own hashing pass, saving bandwidth on large cloud files.
	pub content_md5: Option<String>,

	/// Native provider file identifier that survives renames and parent
	/// moves. OpenDAL does not expose this today, so Spacedrive populates it
	/// out of band (e.g. OneDrive Delta API) in later sets.
	pub provider_file_id: Option<String>,
}
