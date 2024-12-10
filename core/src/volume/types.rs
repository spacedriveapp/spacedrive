use super::{
	// cloud::CloudCredentials,
	error::{CloudVolumeError, VolumeError},
};
use crate::library::Library;
use sd_core_sync::DevicePubId;
use sd_prisma::{
	prisma::{
		device,
		volume::{self},
		PrismaClient,
	},
	prisma_sync,
};
use sd_sync::*;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use std::fmt;
use std::path::PathBuf;
use std::{path::Path, sync::Arc};
use strum_macros::Display;
use uuid::Uuid;

/// A fingerprint of a volume, used to identify it when it is not persisted in the database
#[derive(Debug, Clone, Hash, Eq, PartialEq, Type)]
pub struct VolumeFingerprint(pub Vec<u8>);

impl VolumeFingerprint {
	pub fn new(device_id: &DevicePubId, volume: &Volume) -> Self {
		// Hash the device ID, mount point, name, total bytes capacity, and file system
		let mut hasher = blake3::Hasher::new();
		hasher.update(&device_id.to_db());
		hasher.update(volume.mount_point.to_string_lossy().as_bytes());
		hasher.update(volume.name.as_bytes());
		hasher.update(&volume.total_bytes_capacity.to_be_bytes());
		hasher.update(volume.file_system.to_string().as_bytes());
		// These are all properties that are unique to a volume and unlikely to change
		// If a `.sdvolume` file is found in the volume, and is fingerprint does not match,
		// but the `pub_id` is the same, we can update the values and regenerate the fingerprint
		// preserving the tracked instance of the volume
		Self(hasher.finalize().as_bytes().to_vec())
	}
}

impl fmt::Display for VolumeFingerprint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VolumePubId(pub Vec<u8>);

impl From<Vec<u8>> for VolumePubId {
	fn from(v: Vec<u8>) -> Self {
		Self(v)
	}
}

impl Into<Vec<u8>> for VolumePubId {
	fn into(self) -> Vec<u8> {
		self.0
	}
}

pub type LibraryId = Uuid;

/// Events emitted by the Volume Manager when volume state changes
#[derive(Debug, Clone, Type, Deserialize, Serialize)]
pub enum VolumeEvent {
	/// Emitted when a new volume is discovered and added
	VolumeAdded(Volume),
	/// Emitted when a volume is removed from the system
	VolumeRemoved(Volume),
	/// Emitted when a volume's properties are updated
	VolumeUpdated { old: Volume, new: Volume },
	/// Emitted when a volume's speed test completes
	VolumeSpeedTested {
		fingerprint: VolumeFingerprint,
		read_speed: u64,
		write_speed: u64,
	},
	/// Emitted when a volume's mount status changes
	VolumeMountChanged {
		fingerprint: VolumeFingerprint,
		is_mounted: bool,
	},
	/// Emitted when a volume encounters an error
	VolumeError {
		fingerprint: VolumeFingerprint,
		error: String,
	},
}

/// Represents a physical or virtual storage volume in the system
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Volume {
	/// Fingerprint of the volume as a hash of its properties, not persisted to the database
	/// Used as the unique identifier for a volume in this module
	pub fingerprint: Option<VolumeFingerprint>,
	/// Database ID (None if not yet committed to database)
	pub id: Option<i32>,
	/// Unique public identifier
	pub pub_id: Option<Vec<u8>>,
	/// Database ID of the device this volume is attached to, if any
	pub device_id: Option<i32>,

	/// Human-readable volume name
	pub name: String,
	/// Type of mount (system, external, etc)
	pub mount_type: MountType,
	/// Path where the volume is mounted
	#[specta(type = Vec<String>)]
	pub mount_point: PathBuf,
	/// for APFS volumes like Macintosh HD, additional mount points are returned
	#[specta(type = Vec<String>)]
	pub mount_points: Vec<PathBuf>,
	/// Whether the volume is currently mounted
	pub is_mounted: bool,
	/// Type of storage device (SSD, HDD, etc)
	pub disk_type: DiskType,
	/// Filesystem type (NTFS, EXT4, etc)
	pub file_system: FileSystem,
	/// Whether the volume is mounted read-only
	pub read_only: bool,
	/// Current error status if any
	pub error_status: Option<String>,

	// Performance metrics
	/// Read speed in megabytes per second
	pub read_speed_mbps: Option<u64>,
	/// Write speed in megabytes per second
	pub write_speed_mbps: Option<u64>,
	/// Total storage capacity in bytes
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_bytes_capacity: u64,
	/// Available storage space in bytes
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_bytes_available: u64,
}

// We can use this to see if a volume has changed
impl PartialEq for Volume {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
            && self.disk_type == other.disk_type
            && self.file_system == other.file_system
			&& self.mount_type == other.mount_type
			&& self.mount_point == other.mount_point
			// Check if any mount points overlap
			&& (self.mount_points.iter().any(|mp| other.mount_points.contains(mp))
			|| other.mount_points.iter().any(|mp| self.mount_points.contains(mp)))
			&& self.is_mounted == other.is_mounted
			&& self.read_only == other.read_only
			&& self.error_status == other.error_status
			&& self.total_bytes_capacity == other.total_bytes_capacity
			&& self.total_bytes_available == other.total_bytes_available
	}
}

impl Eq for Volume {}

impl From<volume::Data> for Volume {
	fn from(vol: volume::Data) -> Self {
		Volume {
			id: Some(vol.id),
			pub_id: Some(vol.pub_id),
			device_id: vol.device_id,
			name: vol.name.unwrap_or_else(|| "Unknown".to_string()),
			mount_type: vol
				.mount_type
				.as_deref()
				.map(MountType::from_string)
				.unwrap_or(MountType::System),
			mount_point: PathBuf::from(vol.mount_point.unwrap_or_else(|| "/".to_string())),
			mount_points: Vec::new(),
			is_mounted: vol.is_mounted.unwrap_or(false),
			disk_type: vol
				.disk_type
				.as_deref()
				.map(DiskType::from_string)
				.unwrap_or(DiskType::Unknown),
			file_system: vol
				.file_system
				.as_deref()
				.map(FileSystem::from_string)
				.unwrap_or_else(|| FileSystem::Other("Unknown".to_string())),
			read_only: vol.read_only.unwrap_or(false),
			error_status: vol.error_status,
			total_bytes_capacity: vol
				.total_bytes_capacity
				.and_then(|t| t.parse().ok())
				.unwrap_or(0),
			total_bytes_available: vol
				.total_bytes_available
				.and_then(|a| a.parse().ok())
				.unwrap_or(0),
			read_speed_mbps: vol.read_speed_mbps.map(|s| s as u64),
			write_speed_mbps: vol.write_speed_mbps.map(|s| s as u64),
			fingerprint: None,
		}
	}
}

impl Volume {
	/// Creates a new Volume instance from detected system volume information
	pub fn new(
		name: String,
		mount_type: MountType,
		mount_point: PathBuf,
		mount_points: Vec<PathBuf>,
		disk_type: DiskType,
		file_system: FileSystem,
		total_bytes_capacity: u64,
		total_bytes_available: u64,
		read_only: bool,
	) -> Self {
		Self {
			id: None,
			pub_id: None,
			device_id: None,
			name,
			mount_type,
			mount_point,
			mount_points,
			is_mounted: true,
			disk_type,
			file_system,
			read_only,
			error_status: None,
			read_speed_mbps: None,
			write_speed_mbps: None,
			total_bytes_capacity,
			total_bytes_available,
			fingerprint: None,
		}
	}

	/// Check if a path is under any of this volume's mount points
	pub fn contains_path(&self, path: &Path) -> bool {
		self.mount_points.iter().any(|mp| path.starts_with(mp))
	}

	/// Merge system detected volume with database volume, preferring system values for hardware info
	pub fn merge_with_db(system_volume: &Volume, db_volume: &Volume) -> Volume {
		Volume {
			// Keep system-detected hardware properties
			mount_point: system_volume.mount_point.clone(),
			mount_points: system_volume.mount_points.clone(),
			total_bytes_capacity: system_volume.total_bytes_capacity,
			total_bytes_available: system_volume.total_bytes_available,
			disk_type: system_volume.disk_type.clone(),
			file_system: system_volume.file_system.clone(),
			mount_type: system_volume.mount_type.clone(),
			is_mounted: system_volume.is_mounted,
			fingerprint: system_volume.fingerprint.clone(),
			name: system_volume.name.clone(),
			read_only: system_volume.read_only,
			error_status: system_volume.error_status.clone(),
			read_speed_mbps: system_volume.read_speed_mbps,
			write_speed_mbps: system_volume.write_speed_mbps,

			// Keep database-tracked properties and metadata
			id: db_volume.id,
			device_id: db_volume.device_id,
			pub_id: db_volume.pub_id.clone(),
		}
	}

	pub fn is_volume_tracked(&self) -> bool {
		self.pub_id.is_some()
	}

	/// Creates a new volume record in the database
	pub async fn create(
		&self,
		db: &Arc<PrismaClient>,
		device_pub_id: Vec<u8>,
	) -> Result<Volume, VolumeError> {
		let pub_id = Uuid::now_v7().as_bytes().to_vec();

		let device_id = db
			.device()
			.find_unique(device::pub_id::equals(device_pub_id.clone()))
			.select(device::select!({ id }))
			.exec()
			.await?
			.ok_or(VolumeError::DeviceNotFound(device_pub_id))?
			.id;

		let volume = db
			.volume()
			.create(
				pub_id,
				vec![
					volume::name::set(Some(self.name.clone())),
					volume::mount_type::set(Some(self.mount_type.to_string())),
					volume::mount_point::set(Some(self.mount_point.to_str().unwrap().to_string())),
					volume::is_mounted::set(Some(self.is_mounted)),
					volume::disk_type::set(Some(self.disk_type.to_string())),
					volume::file_system::set(Some(self.file_system.to_string())),
					volume::read_only::set(Some(self.read_only)),
					volume::error_status::set(self.error_status.clone()),
					volume::total_bytes_capacity::set(Some(self.total_bytes_capacity.to_string())),
					volume::total_bytes_available::set(Some(
						self.total_bytes_available.to_string(),
					)),
					volume::read_speed_mbps::set(
						self.read_speed_mbps.filter(|&v| v != 0).map(|v| v as i64),
					),
					volume::write_speed_mbps::set(
						self.write_speed_mbps.filter(|&v| v != 0).map(|v| v as i64),
					),
					volume::device_id::set(Some(device_id)),
				],
			)
			.exec()
			.await?;
		Ok(volume.into())
	}

	/// Updates an existing volume record in the database
	pub async fn update(&self, db: &PrismaClient) -> Result<(), VolumeError> {
		let id = self.id.ok_or(VolumeError::NotInDatabase)?;

		db.volume()
			.update(
				volume::id::equals(id),
				vec![
					volume::name::set(Some(self.name.clone())),
					volume::mount_type::set(Some(self.mount_type.to_string())),
					volume::mount_point::set(Some(self.mount_point.to_str().unwrap().to_string())),
					volume::is_mounted::set(Some(self.is_mounted)),
					volume::disk_type::set(Some(self.disk_type.to_string())),
					volume::file_system::set(Some(self.file_system.to_string())),
					volume::read_only::set(Some(self.read_only)),
					volume::error_status::set(self.error_status.clone()),
					volume::total_bytes_capacity::set(Some(self.total_bytes_capacity.to_string())),
					volume::total_bytes_available::set(Some(
						self.total_bytes_available.to_string(),
					)),
					volume::read_speed_mbps::set(
						self.read_speed_mbps.filter(|&v| v != 0).map(|v| v as i64),
					),
					volume::write_speed_mbps::set(
						self.write_speed_mbps.filter(|&v| v != 0).map(|v| v as i64),
					),
				],
			)
			.exec()
			.await?;
		Ok(())
	}

	/// Writes the .sdvolume file to the volume's root
	pub async fn write_volume_file(&self) -> Result<(), VolumeError> {
		if !self.is_mounted || self.read_only {
			return Ok(()); // Skip if volume isn't mounted or is read-only
		}

		let fingerprint = self
			.fingerprint
			.as_ref()
			.ok_or(VolumeError::MissingFingerprint)?;
		let pub_id = self.pub_id.as_ref().ok_or(VolumeError::NotTracked)?;

		let volume_file = SdVolumeFile {
			pub_id: pub_id.clone(),
			fingerprint: fingerprint.to_string(),
			last_seen: chrono::Utc::now(),
		};

		let path = self.mount_point.join(".sdvolume");
		let file = tokio::fs::File::create(&path)
			.await
			.map_err(|e| VolumeError::IoError(e))?;

		serde_json::to_writer(file.into_std().await, &volume_file)
			.map_err(|e| VolumeError::SerializationError(e))?;

		Ok(())
	}

	/// Reads the .sdvolume file from the volume's root if it exists
	pub async fn read_volume_file(&self) -> Result<Option<SdVolumeFile>, VolumeError> {
		if !self.is_mounted {
			return Ok(None);
		}

		let path = self.mount_point.join(".sdvolume");
		if !path.exists() {
			return Ok(None);
		}

		let file = tokio::fs::File::open(&path)
			.await
			.map_err(|e| VolumeError::IoError(e))?;

		let volume_file = serde_json::from_reader(file.into_std().await)
			.map_err(|e| VolumeError::SerializationError(e))?;

		Ok(Some(volume_file))
	}

	pub async fn sync_db_create(
		&self,
		library: &Library,
		device_pub_id: Vec<u8>,
	) -> Result<Volume, VolumeError> {
		let Library { db, sync, .. } = library;
		let pub_id = Uuid::now_v7().as_bytes().to_vec();

		let device_id = db
			.device()
			.find_unique(device::pub_id::equals(device_pub_id.clone()))
			.select(device::select!({ id }))
			.exec()
			.await?
			.ok_or(VolumeError::DeviceNotFound(device_pub_id))?
			.id;

		let (sync_params, db_params) = [
			sync_db_entry!(self.name.clone(), volume::name),
			sync_db_entry!(
				self.mount_point.to_str().unwrap_or_default().to_string(),
				volume::mount_point
			),
			sync_db_entry!(self.mount_type.to_string(), volume::mount_type),
			sync_db_entry!(
				self.total_bytes_capacity.to_string(),
				volume::total_bytes_capacity
			),
			sync_db_entry!(
				self.total_bytes_available.to_string(),
				volume::total_bytes_available
			),
			sync_db_entry!(self.disk_type.to_string(), volume::disk_type),
			sync_db_entry!(self.file_system.to_string(), volume::file_system),
			sync_db_entry!(self.is_mounted, volume::is_mounted),
			sync_db_entry!(
				self.read_speed_mbps.unwrap_or(0) as i64,
				volume::read_speed_mbps
			),
			sync_db_entry!(
				self.write_speed_mbps.unwrap_or(0) as i64,
				volume::write_speed_mbps
			),
			sync_db_entry!(self.read_only, volume::read_only),
			sync_db_entry!(
				self.error_status.clone().unwrap_or_default(),
				volume::error_status
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		// Add device connection to db_params
		let mut db_params = db_params;
		db_params.push(volume::device::connect(device::id::equals(device_id)));

		let volume = sync
			.write_op(
				db,
				sync.shared_create(
					prisma_sync::volume::SyncId {
						pub_id: pub_id.clone(),
					},
					sync_params,
				),
				db.volume().create(pub_id, db_params),
			)
			.await?;

		Ok(volume.into())
	}

	pub async fn sync_db_update(&self, library: &Library) -> Result<(), VolumeError> {
		let Library { db, sync, .. } = library;
		let pub_id = self.pub_id.as_ref().ok_or(VolumeError::NotTracked)?;

		let (sync_params, db_params) = [
			sync_db_entry!(self.name.clone(), volume::name),
			sync_db_entry!(
				self.mount_point.to_str().unwrap_or_default().to_string(),
				volume::mount_point
			),
			sync_db_entry!(self.mount_type.to_string(), volume::mount_type),
			sync_db_entry!(
				self.total_bytes_capacity.to_string(),
				volume::total_bytes_capacity
			),
			sync_db_entry!(
				self.total_bytes_available.to_string(),
				volume::total_bytes_available
			),
			sync_db_entry!(self.disk_type.to_string(), volume::disk_type),
			sync_db_entry!(self.file_system.to_string(), volume::file_system),
			sync_db_entry!(self.is_mounted, volume::is_mounted),
			sync_db_entry!(
				self.read_speed_mbps.unwrap_or(0) as i64,
				volume::read_speed_mbps
			),
			sync_db_entry!(
				self.write_speed_mbps.unwrap_or(0) as i64,
				volume::write_speed_mbps
			),
			sync_db_entry!(self.read_only, volume::read_only),
			sync_db_entry!(
				self.error_status.clone().unwrap_or_default(),
				volume::error_status
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_update(
				prisma_sync::volume::SyncId {
					pub_id: pub_id.clone(),
				},
				sync_params,
			),
			db.volume()
				.update(volume::pub_id::equals(pub_id.clone()), db_params),
		)
		.await?;

		Ok(())
	}

	pub async fn sync_db_delete(&self, library: &Library) -> Result<(), VolumeError> {
		let Library { db, sync, .. } = library;
		let pub_id = self.pub_id.as_ref().ok_or(VolumeError::NotTracked)?;

		sync.write_op(
			db,
			sync.shared_delete(prisma_sync::volume::SyncId {
				pub_id: pub_id.clone(),
			}),
			db.volume().delete(volume::pub_id::equals(pub_id.clone())),
		)
		.await?;

		Ok(())
	}

	// pub async fn new_cloud_volume(
	// 	provider: CloudProvider,
	// 	credentials: CloudCredentials,
	// ) -> Result<Self, VolumeError> {
	// 	let provider_impl = match provider {
	// 		// CloudProvider::GoogleDrive => Box::new(GoogleDriveProvider::new(credentials)),
	// 		// CloudProvider::Dropbox => Box::new(DropboxProvider::new(credentials)),
	// 		_ => return Err(CloudVolumeError::UnsupportedCloudProvider(provider)),
	// 	};

	// 	let storage_info = provider_impl.get_storage_info().await?;

	// 	Ok(Self {
	// 		id: None,
	// 		pub_id: None,
	// 		device_id: None,
	// 		name: format!("{} Cloud Storage", provider),
	// 		mount_type: MountType::Cloud(provider),
	// 		mount_point: PathBuf::from("/"), // Virtual root path
	// 		mount_points: vec![],
	// 		is_mounted: true,
	// 		disk_type: DiskType::Virtual,
	// 		file_system: FileSystem::Cloud,
	// 		read_only: false,
	// 		error_status: None,
	// 		read_speed_mbps: None,
	// 		write_speed_mbps: None,
	// 		total_bytes_capacity: storage_info.total_bytes_capacity,
	// 		total_bytes_available: storage_info.total_bytes_available,
	// 		fingerprint: None,
	// 	})
	// }

	// pub async fn refresh_cloud_storage_info(&mut self) -> Result<(), VolumeError> {
	// 	if let MountType::Cloud(provider) = &self.mount_type {
	// 		let provider_impl = get_cloud_provider(provider)?;
	// 		let storage_info = provider_impl.get_storage_info().await?;

	// 		self.total_bytes_capacity = storage_info.total_bytes_capacity;
	// 		self.total_bytes_available = storage_info.total_bytes_available;
	// 	}
	// 	Ok(())
	// }
}

/// Represents the type of physical storage device
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
#[allow(clippy::upper_case_acronyms)]
pub enum DiskType {
	/// Solid State Drive
	SSD,
	/// Hard Disk Drive
	HDD,
	/// Virtual disk type
	Virtual,
	/// Unknown or virtual disk type
	Unknown,
}

impl DiskType {
	pub fn from_string(disk_type: &str) -> Self {
		match disk_type.to_uppercase().as_str() {
			"SSD" => Self::SSD,
			"HDD" => Self::HDD,
			_ => Self::Unknown,
		}
	}
}

/// Represents the filesystem type of the volume
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
pub enum FileSystem {
	/// Windows NTFS filesystem
	NTFS,
	/// FAT32 filesystem
	FAT32,
	/// Linux EXT4 filesystem
	EXT4,
	/// Apple APFS filesystem
	APFS,
	/// ExFAT filesystem
	ExFAT,
	/// Other/unknown filesystem type
	Other(String),
}

impl FileSystem {
	pub fn from_string(fs: &str) -> Self {
		match fs.to_uppercase().as_str() {
			"NTFS" => FileSystem::NTFS,
			"FAT32" => FileSystem::FAT32,
			"EXT4" => FileSystem::EXT4,
			"APFS" => FileSystem::APFS,
			"EXFAT" => FileSystem::ExFAT,
			other => FileSystem::Other(other.to_string()),
		}
	}
}

/// Represents how the volume is mounted in the system
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
pub enum MountType {
	/// System/boot volume
	System,
	/// External/removable volume
	External,
	/// Network-attached volume
	Network,
	/// Virtual/container volume
	Virtual,
	// Cloud mounted as a virtual volume
	Cloud(CloudProvider),
}

impl MountType {
	pub fn from_string(mount_type: &str) -> Self {
		match mount_type.to_uppercase().as_str() {
			"SYSTEM" => Self::System,
			"EXTERNAL" => Self::External,
			"NETWORK" => Self::Network,
			"VIRTUAL" => Self::Virtual,
			_ => Self::System,
		}
	}
}

/// Represents the cloud storage provider
#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
pub enum CloudProvider {
	SpacedriveCloud,
	GoogleDrive,
	Dropbox,
	OneDrive,
	ICloud,
	AmazonS3,
	Mega,
	Box,
	pCloud,
	Proton,
	Sync,
	Backblaze,
	Wasabi,
	DigitalOcean,
	Azure,
	OwnCloud,
	NextCloud,
	WebDAV,
}

/// Configuration options for volume operations
#[derive(Debug, Clone)]
pub struct VolumeOptions {
	/// Whether to include system volumes
	pub include_system: bool,
	/// Whether to include virtual volumes
	pub include_virtual: bool,
	/// Whether to run speed tests on discovery
	pub run_speed_test: bool,
	/// Maximum concurrent speed tests
	pub max_concurrent_speed_tests: usize,
}

impl Default for VolumeOptions {
	fn default() -> Self {
		Self {
			include_system: true,
			include_virtual: false,
			run_speed_test: true,
			max_concurrent_speed_tests: 2,
		}
	}
}

impl Serialize for VolumeFingerprint {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		// Convert to hex string when serializing
		serializer.serialize_str(&hex::encode(&self.0))
	}
}

impl<'de> Deserialize<'de> for VolumeFingerprint {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		hex::decode(s)
			.map(VolumeFingerprint)
			.map_err(serde::de::Error::custom)
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SdVolumeFile {
	pub pub_id: Vec<u8>,
	pub fingerprint: String, // Store as hex string
	pub last_seen: chrono::DateTime<chrono::Utc>,
}
