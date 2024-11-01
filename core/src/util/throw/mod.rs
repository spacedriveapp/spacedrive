use sd_prisma::prisma::exif_data::device_id;
use sd_prisma::prisma::volume;
use sd_prisma::prisma::PrismaClient;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use std::{
	hash::{Hash, Hasher},
	path::PathBuf,
};
use strum_macros::Display;

use tracing::error;
use uuid::Uuid;

pub mod manager;
pub mod os;
pub mod speed;
pub mod statistics;
pub mod watcher;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VolumeError {
	#[error("I/O error: {0}")]
	Io(#[from] tokio::io::Error),

	#[error("Timeout error: {0}")]
	Timeout(#[from] tokio::time::error::Elapsed),

	#[error("No mount point found for volume")]
	NoMountPoint,

	#[error("Directory error: {0}")]
	DirectoryError(String),

	#[error("Database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("No device found")]
	NoDeviceFound,
}

// Conversion to rspc::Error
impl From<VolumeError> for rspc::Error {
	fn from(e: VolumeError) -> Self {
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Volume {
	// ids will be None if the volume is not committed to the database
	pub id: Option<i32>,
	pub pub_id: Option<Vec<u8>>,

	pub name: String, // Volume name
	pub mount_type: MountType,
	pub mount_point: PathBuf, // List of mount points
	pub is_mounted: bool,
	pub disk_type: DiskType,
	pub file_system: FileSystem,
	pub read_only: bool,              // True if read-only
	pub error_status: Option<String>, // SMART error status or similar

	// Statistics
	// I/O speed in Mbps
	pub read_speed_mbps: Option<u64>,
	pub write_speed_mbps: Option<u64>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_bytes_capacity: u64, // Total bytes capacity
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_bytes_available: u64, // Total bytes available
}

// impl Hash for Volume {
// 	fn hash<H: Hasher>(&self, state: &mut H) {
// 		self.name.hash(state);
// 		self.mount_point.hash(state);
// 		self.disk_type.hash(state);
// 		self.file_system.hash(state);
// 	}
// }

impl PartialEq for Volume {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.disk_type == other.disk_type
			&& self.file_system == other.file_system
			// Leaving mount points for last because O(n * m)
			&& self.mount_point == other.mount_point
	}
}

impl Eq for Volume {}

impl From<volume::Data> for Volume {
	fn from(vol: volume::Data) -> Self {
		Volume {
			id: Some(vol.id),
			pub_id: Some(vol.pub_id),
			name: vol.name.unwrap_or_else(|| "Unknown".to_string()),
			mount_type: vol
				.mount_type
				.and_then(|mt| Some(MountType::from_string(&mt)))
				.unwrap_or(MountType::System),
			mount_point: PathBuf::from(vol.mount_point.unwrap_or_else(|| "/".to_string())),
			is_mounted: vol.is_mounted.unwrap_or(false),
			disk_type: vol
				.disk_type
				.and_then(|dt| Some(DiskType::from_string(&dt)))
				.unwrap_or(DiskType::Unknown),
			file_system: vol
				.file_system
				.and_then(|fs| Some(FileSystem::from_string(&fs)))
				.unwrap_or(FileSystem::Other("Unknown".to_string())),
			read_only: vol.read_only.unwrap_or(false),
			error_status: vol.error_status,

			// Statistics
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
		}
	}
}

impl Volume {
	pub async fn create(&self, db: &PrismaClient, device_id: i32) -> Result<(), VolumeError> {
		let pub_id = Uuid::now_v7().as_bytes().to_vec();
		db.volume()
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
					volume::read_speed_mbps::set(self.read_speed_mbps.and_then(|v| {
						if v == 0 {
							None
						} else {
							Some(v as i64)
						}
					})),
					volume::write_speed_mbps::set(self.write_speed_mbps.and_then(|v| {
						if v == 0 {
							None
						} else {
							Some(v as i64)
						}
					})),
					volume::device_id::set(Some(device_id)),
				],
			)
			.exec()
			.await?;
		Ok(())
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
#[allow(clippy::upper_case_acronyms)]
pub enum DiskType {
	SSD,
	HDD,
	Unknown,
}

impl DiskType {
	pub fn from_string(disk_type: &str) -> Self {
		match disk_type {
			"SSD" => Self::SSD,
			"HDD" => Self::HDD,
			_ => Self::Unknown,
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
pub enum FileSystem {
	NTFS,
	FAT32,
	EXT4,
	APFS,
	ExFAT,
	Other(String),
}

impl FileSystem {
	// Create a function to convert a String into a FileSystem enum
	pub fn from_string(fs: &str) -> Self {
		match fs.to_uppercase().as_str() {
			"NTFS" => FileSystem::NTFS,
			"FAT32" => FileSystem::FAT32,
			"EXT4" => FileSystem::EXT4,
			"APFS" => FileSystem::APFS,
			"EXFAT" => FileSystem::ExFAT,
			// If the string does not match known variants, store it in the Other variant
			_ => FileSystem::Other(fs.to_string()),
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq, Display)]
pub enum MountType {
	System,
	External,
	Network,
	Virtual,
}

impl MountType {
	pub fn from_string(mount_type: &str) -> Self {
		match mount_type {
			"System" => Self::System,
			"External" => Self::External,
			"Network" => Self::Network,
			"Virtual" => Self::Virtual,
			_ => Self::System,
		}
	}
}

// pub async fn save_volume(library: &Library) -> Result<(), VolumeError> {
// 	// enter all volumes associate with this client add to db
// 	for volume in get_volumes() {
// 		let params = vec![
// 			disk_type::set(volume.disk_type.map(|t| t.to_string())),
// 			filesystem::set(volume.file_system.clone()),
// 			total_bytes_capacity::set(volume.total_capacity.to_string()),
// 			total_bytes_available::set(volume.available_capacity.to_string()),
// 		];

// 		library
// 			.db
// 			.volume()
// 			.upsert(
// 				node_id_mount_point_name(
// 					library.node_local_id,
// 					volume.mount_point,
// 					volume.name,
// 				),
// 				volume::create(
// 					library.node_local_id,
// 					volume.name,
// 					volume.mount_point,
// 					params.clone(),
// 				),
// 				params,
// 			)
// 			.exec()
// 			.await?;
// 	}
// 	// cleanup: remove all unmodified volumes associate with this client

// 	Ok(())
// }

// #[test]
// fn test_get_volumes() {
//   let volumes = get_volumes()?;
//   dbg!(&volumes);
//   assert!(volumes.len() > 0);
// }
