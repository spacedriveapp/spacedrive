//! # Cloud storage backend implementation using OpenDAL
//!
//! `core::volume::backend::cloud` provides unified cloud storage access through
//! Apache OpenDAL. Each [`CloudBackend`] wraps an [`opendal::Operator`] with a
//! resilience layer stack (tracing, retry, timeout, concurrency limit) so that
//! transient network failures, provider rate limits, and stuck connections
//! never propagate as hard failures into Spacedrive's indexing and file
//! operations pipeline.

use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use opendal::layers::{ConcurrentLimitLayer, RetryLayer, TimeoutLayer, TracingLayer};
use opendal::{Lister, Operator};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::debug;

use super::{
	BackendFeatures, BackendType, ChangeNotificationKind, CloudServiceType, HashAlgorithm,
	RawDirEntry, RawMetadata, VolumeBackend,
};
use crate::ops::indexing::state::EntryKind;
use crate::volume::error::VolumeError;

/// Convert OpenDAL's `last_modified` value into a `SystemTime`.
///
/// OpenDAL 0.55 uses `opendal::raw::Timestamp` (a `jiff::Timestamp` newtype).
/// Isolating the conversion here keeps jiff out of every call site, and
/// coercing pre-1970 values to the epoch matches Spacedrive's existing
/// "unknown modified time" convention.
fn jiff_to_system_time(ts: opendal::raw::Timestamp) -> SystemTime {
	let millis = ts.into_inner().as_millisecond();
	if millis >= 0 {
		SystemTime::UNIX_EPOCH + Duration::from_millis(millis as u64)
	} else {
		SystemTime::UNIX_EPOCH
	}
}

/// Wrap an [`Operator`] with Spacedrive's default resilience layer stack.
///
/// The order matters: `TracingLayer` sits innermost so it observes the raw
/// provider calls without being confused by retries; `RetryLayer` sits
/// outermost so that every caller benefits from transparent retries regardless
/// of whether a timeout or a concurrency gate fired first. Timeouts live
/// between the retry and the concurrency limit so a stuck request is aborted
/// before it monopolises a concurrency slot, and the next retry attempt
/// competes for a fresh slot.
fn apply_default_layers(op: Operator) -> Operator {
	op.layer(TracingLayer)
		.layer(ConcurrentLimitLayer::new(16))
		.layer(
			TimeoutLayer::new()
				.with_timeout(Duration::from_secs(30))
				.with_io_timeout(Duration::from_secs(30)),
		)
		.layer(RetryLayer::default().with_max_times(3).with_jitter())
}

/// Cloud storage backend powered by OpenDAL.
///
/// Unifies access to S3, Google Drive, Dropbox, OneDrive, and the other
/// services OpenDAL supports so the rest of Spacedrive can treat every
/// provider through the same [`VolumeBackend`] trait.
#[derive(Debug, Clone)]
pub struct CloudBackend {
	operator: opendal::Operator,
	service_type: CloudServiceType,
	/// Root prefix within the provider (e.g. bucket prefix, OneDrive folder).
	root: PathBuf,
}

impl CloudBackend {
	/// Create an S3-compatible backend (AWS, Wasabi, DigitalOcean Spaces, custom endpoint).
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

		let operator = Operator::new(builder)
			.map_err(|e| VolumeError::Platform(format!("Failed to create S3 operator: {}", e)))?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::S3,
			root: PathBuf::from("/"),
		})
	}

	/// Create a new cloud backend for Google Drive.
	///
	/// Like OneDrive, OpenDAL's Gdrive builder requires either `access_token`
	/// alone or `refresh_token + client_id + client_secret`. The two paths
	/// are mutually exclusive. We take the long-lived path so refreshes
	/// happen transparently; the short-lived `access_token` argument stays
	/// in the signature for caller stability.
	pub async fn new_google_drive(
		_access_token: impl AsRef<str>,
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Gdrive::default()
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Google Drive operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::GoogleDrive,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a new cloud backend for OneDrive.
	///
	/// OpenDAL's Onedrive builder accepts either `access_token` alone for
	/// short-lived access, or `refresh_token + client_id [+ client_secret]`
	/// for long-lived access that auto-refreshes. The two paths are mutually
	/// exclusive and passing both fails the builder. We take the long-lived
	/// path because the OAuth flow always requests the `offline_access` scope,
	/// so every registered volume has a refresh token. The `access_token`
	/// parameter is preserved in the signature so the caller in `VolumeManager`
	/// stays unchanged; OpenDAL obtains its own short-lived token from the
	/// refresh token on first use.
	pub async fn new_onedrive(
		_access_token: impl AsRef<str>,
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Onedrive::default()
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create OneDrive operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::OneDrive,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create a Dropbox backend. OpenDAL auto-refreshes the access token from the refresh token.
	pub async fn new_dropbox(
		refresh_token: impl AsRef<str>,
		client_id: impl AsRef<str>,
		client_secret: impl AsRef<str>,
		root: Option<String>,
	) -> Result<Self, VolumeError> {
		let mut builder = opendal::services::Dropbox::default()
			.refresh_token(refresh_token.as_ref())
			.client_id(client_id.as_ref())
			.client_secret(client_secret.as_ref());

		if let Some(r) = &root {
			builder = builder.root(r);
		}

		let operator = Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Dropbox operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::Dropbox,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Create an Azure Blob Storage backend.
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

		let operator = Operator::new(builder)
			.map_err(|e| {
				VolumeError::Platform(format!("Failed to create Azure Blob operator: {}", e))
			})?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::AzureBlob,
			root: PathBuf::from("/"),
		})
	}

	/// Create a Google Cloud Storage backend.
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

		let operator = Operator::new(builder)
			.map_err(|e| VolumeError::Platform(format!("Failed to create GCS operator: {}", e)))?
			.finish();

		Ok(Self {
			operator: apply_default_layers(operator),
			service_type: CloudServiceType::GoogleCloudStorage,
			root: PathBuf::from(root.unwrap_or_else(|| "/".to_string())),
		})
	}

	/// Server-side copy within the same cloud backend.
	///
	/// Uses the provider's native copy primitive (S3 `CopyObject`, OneDrive
	/// `/copy`, Dropbox `/files/copy_v2`) instead of streaming bytes through
	/// the client. Callers must verify `features().server_side_copy` first;
	/// this method short-circuits with [`VolumeError::UnsupportedOperation`]
	/// when the capability is absent, so a misconfigured fallback surfaces as
	/// a typed error rather than panicking or dragging the failure into the
	/// retry/timeout layers where diagnostics are noisier.
	pub async fn copy(&self, from: &Path, to: &Path) -> Result<(), VolumeError> {
		if !self.features().server_side_copy {
			return Err(VolumeError::UnsupportedOperation {
				operation: "server-side copy",
				backend: self.service_type.scheme(),
			});
		}

		let from_path = self.to_cloud_path(from);
		let to_path = self.to_cloud_path(to);
		debug!("CloudBackend::copy: {} -> {}", from_path, to_path);

		self.operator
			.copy(&from_path, &to_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
	}

	/// Provider scheme string for log fields and error messages.
	pub fn service_scheme(&self) -> &'static str {
		self.service_type.scheme()
	}

	/// Wrap a pre-built operator with Spacedrive's default resilience layers.
	///
	/// Every construction path flows through here so retry, timeout, and
	/// tracing behaviour stay consistent across providers.
	pub fn from_operator(operator: Operator, service_type: CloudServiceType) -> Self {
		Self {
			operator: apply_default_layers(operator),
			service_type,
			root: PathBuf::from("/"),
		}
	}

	/// Borrow the underlying OpenDAL operator.
	///
	/// Exposed to sibling modules (file copy strategies, change detection)
	/// that need streaming reader/writer handles beyond what the
	/// [`VolumeBackend`] trait surfaces. Kept `pub(crate)` so extension code
	/// cannot bypass the capability checks baked into [`features`].
	pub(crate) fn operator(&self) -> &Operator {
		&self.operator
	}

	/// Translate an [`SdPath::Cloud`] string into the provider-native key.
	///
	/// Shared by the file copy strategies that already have a cloud path
	/// string in hand and would otherwise duplicate the `trim_start_matches`
	/// dance inline.
	pub(crate) fn cloud_key(&self, path: &str) -> String {
		path.trim_start_matches('/').to_string()
	}
}

impl CloudBackend {
	/// Strip the leading `/` so the result is a provider-native key.
	fn to_cloud_path(&self, path: &Path) -> String {
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
		let mut lister = self
			.operator
			.lister(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

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
				modified: metadata.last_modified().map(jiff_to_system_time),
				inode: None,
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

		let modified = metadata.last_modified().map(jiff_to_system_time);

		Ok(RawMetadata {
			kind,
			size: metadata.content_length(),
			modified,
			created: None,
			accessed: None,
			inode: None,
			permissions: None,
			etag: metadata.etag().map(str::to_owned),
			version: metadata.version().map(str::to_owned),
			content_md5: metadata.content_md5().map(str::to_owned),
			// TODO(cloud-mvp): populate provider_file_id from the OneDrive
			// Delta API — OpenDAL does not expose native IDs. See
			// .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-6.
			provider_file_id: None,
		})
	}

	async fn exists(&self, path: &Path) -> Result<bool, VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		// Only a genuine `NotFound` means "doesn't exist"; swallowing other
		// kinds (auth failures, rate limits, transport errors) as `false` is a
		// data-integrity hazard because callers then happily overwrite or
		// skip files they have no business touching.
		match self.operator.stat(&cloud_path).await {
			Ok(_) => Ok(true),
			Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(false),
			Err(e) => Err(VolumeError::Io(std::io::Error::new(
				std::io::ErrorKind::Other,
				e,
			))),
		}
	}

	async fn delete(&self, path: &Path) -> Result<(), VolumeError> {
		let cloud_path = self.to_cloud_path(path);
		debug!("CloudBackend::delete: {}", cloud_path);

		let metadata = self
			.operator
			.stat(&cloud_path)
			.await
			.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		if metadata.is_dir() {
			self.operator
				.remove_all(&cloud_path)
				.await
				.map_err(|e| VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		} else {
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

		// OpenDAL treats a trailing `/` as a directory marker; without it, some
		// backends (S3, GCS) would create a zero-byte file at `cloud_path` instead.
		if !cloud_path.ends_with('/') {
			cloud_path.push('/');
		}

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

	fn features(&self) -> BackendFeatures {
		let mut features = features_for_service(self.service_type);

		// Trust OpenDAL's runtime capability over our hardcoded table: if the
		// service builder was configured in a way that disables multipart or
		// server-side copy, our optimistic defaults would cause real runtime
		// errors. The capability probe is cheap (it reads from an Arc-shared
		// info struct already populated at construction).
		let cap = self.operator.info().full_capability();
		if !cap.copy {
			features.server_side_copy = false;
		}
		if !cap.rename {
			features.server_side_rename = false;
		}
		features
	}

	fn as_cloud(&self) -> Option<&CloudBackend> {
		Some(self)
	}
}

/// Static capability table indexed by [`CloudServiceType`].
///
/// Values come from `.investigations/cloud-drives/research/02-opendal-deep-dive.md`
/// and the individual provider documentation. They describe what the service
/// is _capable of_ in principle; the runtime cross-check in
/// [`CloudBackend::features`] narrows it to what the configured operator can
/// actually do.
fn features_for_service(service: CloudServiceType) -> BackendFeatures {
	match service {
		CloudServiceType::OneDrive => BackendFeatures {
			server_side_copy: true,
			server_side_rename: true,
			stable_file_id: true,
			change_notifications: ChangeNotificationKind::DeltaToken,
			content_hash: Some(HashAlgorithm::QuickXor),
			case_insensitive: true,
			supports_duplicates: false,
			max_file_size: Some(250 * 1024 * 1024 * 1024),
			multipart_threshold: Some(4 * 1024 * 1024),
		},
		CloudServiceType::S3 | CloudServiceType::Wasabi | CloudServiceType::DigitalOceanSpaces => {
			BackendFeatures {
				server_side_copy: true,
				server_side_rename: false,
				stable_file_id: false,
				change_notifications: ChangeNotificationKind::None,
				content_hash: Some(HashAlgorithm::Md5),
				case_insensitive: false,
				supports_duplicates: true,
				max_file_size: Some(5 * 1024 * 1024 * 1024 * 1024),
				multipart_threshold: Some(5 * 1024 * 1024),
			}
		}
		CloudServiceType::GoogleDrive => BackendFeatures {
			server_side_copy: true,
			server_side_rename: true,
			stable_file_id: true,
			change_notifications: ChangeNotificationKind::DeltaToken,
			content_hash: Some(HashAlgorithm::Md5),
			case_insensitive: false,
			supports_duplicates: true,
			max_file_size: Some(5 * 1024 * 1024 * 1024 * 1024),
			multipart_threshold: Some(5 * 1024 * 1024),
		},
		CloudServiceType::Dropbox => BackendFeatures {
			server_side_copy: true,
			server_side_rename: true,
			stable_file_id: false,
			change_notifications: ChangeNotificationKind::LongPoll,
			// Dropbox exposes a proprietary block hash; representing it as
			// Sha256 is inaccurate, so leave as None until the extension
			// layer understands the Dropbox content_hash algorithm.
			content_hash: None,
			case_insensitive: true,
			supports_duplicates: false,
			max_file_size: Some(350 * 1024 * 1024 * 1024),
			multipart_threshold: Some(150 * 1024 * 1024),
		},
		CloudServiceType::AzureBlob => BackendFeatures {
			server_side_copy: true,
			server_side_rename: false,
			stable_file_id: false,
			change_notifications: ChangeNotificationKind::None,
			content_hash: Some(HashAlgorithm::Md5),
			case_insensitive: false,
			supports_duplicates: true,
			// Single block blob limit: 50,000 blocks x 100 MiB = ~4.75 TiB.
			max_file_size: Some(4_770_000_000_000),
			multipart_threshold: Some(4 * 1024 * 1024),
		},
		CloudServiceType::GoogleCloudStorage => BackendFeatures {
			server_side_copy: true,
			server_side_rename: false,
			stable_file_id: false,
			change_notifications: ChangeNotificationKind::None,
			content_hash: Some(HashAlgorithm::Crc32),
			case_insensitive: false,
			supports_duplicates: true,
			max_file_size: Some(5 * 1024 * 1024 * 1024 * 1024),
			multipart_threshold: Some(5 * 1024 * 1024),
		},
		CloudServiceType::BackblazeB2 => BackendFeatures {
			server_side_copy: true,
			server_side_rename: false,
			stable_file_id: true,
			change_notifications: ChangeNotificationKind::None,
			content_hash: Some(HashAlgorithm::Sha1),
			case_insensitive: false,
			supports_duplicates: false,
			max_file_size: Some(10 * 1024 * 1024 * 1024 * 1024),
			multipart_threshold: Some(100 * 1024 * 1024),
		},
		CloudServiceType::Other => {
			// TODO(cloud-mvp): verify features for custom/other providers —
			// see .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-1.
			BackendFeatures {
				server_side_copy: false,
				server_side_rename: false,
				stable_file_id: false,
				change_notifications: ChangeNotificationKind::None,
				content_hash: None,
				case_insensitive: false,
				supports_duplicates: true,
				max_file_size: None,
				multipart_threshold: None,
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Build an in-memory OpenDAL operator wrapped in Spacedrive's default
	/// layer stack. Used by every happy-path test that only needs to round
	/// trip bytes without hitting a real provider.
	fn memory_backend() -> CloudBackend {
		let op = Operator::new(opendal::services::Memory::default())
			.expect("memory builder")
			.finish();
		CloudBackend::from_operator(op, CloudServiceType::Other)
	}

	#[test]
	fn test_backend_features_cloud_services() {
		let onedrive = features_for_service(CloudServiceType::OneDrive);
		assert!(onedrive.server_side_copy);
		assert!(onedrive.server_side_rename);
		assert!(onedrive.stable_file_id);
		assert_eq!(
			onedrive.change_notifications,
			ChangeNotificationKind::DeltaToken
		);
		assert_eq!(onedrive.content_hash, Some(HashAlgorithm::QuickXor));
		assert!(onedrive.case_insensitive);
		assert!(!onedrive.supports_duplicates);
		assert_eq!(onedrive.max_file_size, Some(250 * 1024 * 1024 * 1024));
		assert_eq!(onedrive.multipart_threshold, Some(4 * 1024 * 1024));

		let s3 = features_for_service(CloudServiceType::S3);
		assert!(s3.server_side_copy);
		assert!(!s3.server_side_rename);
		assert!(!s3.stable_file_id);
		assert_eq!(s3.change_notifications, ChangeNotificationKind::None);
		assert_eq!(s3.content_hash, Some(HashAlgorithm::Md5));
		assert!(!s3.case_insensitive);
		assert!(s3.supports_duplicates);

		let gdrive = features_for_service(CloudServiceType::GoogleDrive);
		assert!(gdrive.server_side_copy);
		assert!(gdrive.server_side_rename);
		assert!(gdrive.stable_file_id);
		assert_eq!(
			gdrive.change_notifications,
			ChangeNotificationKind::DeltaToken
		);
		assert_eq!(gdrive.content_hash, Some(HashAlgorithm::Md5));
	}

	#[test]
	fn test_timestamp_roundtrip_jiff_chrono() {
		// 2026-04-18T12:34:56.789Z — a timestamp with millisecond precision.
		let original_millis: i64 = 1_776_516_896_789;
		let ts: opendal::raw::Timestamp =
			opendal::raw::Timestamp::from_millisecond(original_millis).expect("valid timestamp");

		let system_time = jiff_to_system_time(ts);

		let converted_millis = system_time
			.duration_since(SystemTime::UNIX_EPOCH)
			.expect("post-epoch")
			.as_millis() as i64;

		assert_eq!(converted_millis, original_millis);
	}

	#[test]
	fn test_timestamp_roundtrip_pre_epoch_coerces_to_epoch() {
		let ts: opendal::raw::Timestamp =
			opendal::raw::Timestamp::from_millisecond(-1000).expect("valid timestamp");
		let system_time = jiff_to_system_time(ts);
		assert_eq!(system_time, SystemTime::UNIX_EPOCH);
	}

	#[tokio::test]
	async fn test_exists_distinguishes_not_found_from_error() {
		let backend = memory_backend();

		// Happy path: missing key returns Ok(false), not an error.
		let result = backend.exists(Path::new("nope.txt")).await;
		assert!(matches!(result, Ok(false)));

		// Existence after write returns Ok(true).
		backend
			.write(Path::new("exists.txt"), Bytes::from("hi"))
			.await
			.expect("write");
		assert!(matches!(
			backend.exists(Path::new("exists.txt")).await,
			Ok(true)
		));

		// TODO(cloud-mvp): extend with a failing non-NotFound case once a
		// fault-injection fixture is available (services::Memory cannot
		// synthesise auth or transport errors) — see
		// .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-1.
	}

	#[tokio::test]
	async fn test_apply_default_layers_composes_retry_and_timeout() {
		let raw = Operator::new(opendal::services::Memory::default())
			.expect("memory builder")
			.finish();
		let layered = apply_default_layers(raw);

		// The layered operator must still perform a basic round-trip;
		// composing layers that fight each other would surface here.
		layered
			.write("layer-probe.bin", Bytes::from("ok"))
			.await
			.expect("write through layered operator");
		let read = layered.read("layer-probe.bin").await.expect("read back");
		assert_eq!(read.to_bytes(), Bytes::from("ok"));
	}

	#[tokio::test]
	async fn test_copy_errors_when_features_disabled() {
		// `services::Memory` advertises `cap.copy == false`, which the
		// runtime cross-check in `features()` maps to `server_side_copy ==
		// false`. The guard in `CloudBackend::copy` must then surface
		// `UnsupportedOperation` instead of letting the OpenDAL operator
		// produce a generic `Unsupported` error deep inside the retry stack.
		let backend = memory_backend();
		assert!(!backend.features().server_side_copy);

		backend
			.write(Path::new("src.bin"), Bytes::from("payload"))
			.await
			.expect("write source");

		let result = backend
			.copy(Path::new("src.bin"), Path::new("dst.bin"))
			.await;

		match result {
			Err(VolumeError::UnsupportedOperation { operation, backend }) => {
				assert_eq!(operation, "server-side copy");
				assert_eq!(backend, "cloud");
			}
			other => panic!("expected UnsupportedOperation, got {:?}", other),
		}
	}

	#[tokio::test]
	async fn test_create_folder_cloud_backend_idempotent() {
		// OpenDAL's `create_dir` on a trailing-slash path is a no-op when the
		// directory already exists for every service in our matrix. Guarding
		// at the backend level ensures a re-run of `CreateFolderAction`
		// (e.g. after a flaky network retry) never surfaces as an error to
		// the caller.
		let backend = memory_backend();
		let folder = Path::new("photos/2024");

		backend
			.create_directory(folder, false)
			.await
			.expect("first create_directory should succeed");

		backend
			.create_directory(folder, false)
			.await
			.expect("second create_directory should be idempotent");
	}

	#[tokio::test]
	async fn test_cloud_backend_cross_checks_opendal_capability() {
		// `services::Memory` advertises neither copy nor rename; the
		// runtime cross-check in `features()` must override our hardcoded
		// table (which for `Other` already returns false, so this mostly
		// asserts that the override path never accidentally promotes a
		// capability that the operator cannot honour).
		let backend = memory_backend();
		let cap = backend.operator.info().full_capability();
		let features = backend.features();

		if !cap.copy {
			assert!(!features.server_side_copy);
		}
		if !cap.rename {
			assert!(!features.server_side_rename);
		}
	}

	/// Live S3 smoke test. `#[ignore]` by default; run with
	/// `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_BUCKET`, and
	/// `AWS_REGION` set to exercise the real provider path.
	#[tokio::test]
	#[ignore]
	async fn test_cloud_backend_s3() {
		let bucket = std::env::var("AWS_BUCKET").unwrap();
		let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
		let access_key = std::env::var("AWS_ACCESS_KEY_ID").unwrap();
		let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap();

		let backend = CloudBackend::new_s3(&bucket, &region, &access_key, &secret_key, None)
			.await
			.unwrap();

		let test_data = Bytes::from("Hello, cloud!");
		backend
			.write(Path::new("test.txt"), test_data.clone())
			.await
			.unwrap();

		let read_data = backend.read(Path::new("test.txt")).await.unwrap();
		assert_eq!(test_data, read_data);

		let metadata = backend.metadata(Path::new("test.txt")).await.unwrap();
		assert_eq!(metadata.size, test_data.len() as u64);
		assert_eq!(metadata.kind, EntryKind::File);

		assert!(backend.exists(Path::new("test.txt")).await.unwrap());
	}
}
