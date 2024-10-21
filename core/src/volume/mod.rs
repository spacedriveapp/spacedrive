// Adapted from: https://github.com/kimlimjustin/xplorer/blob/f4f3590d06783d64949766cc2975205a3b689a56/src-tauri/src/drives.rs

use crate::{library::Library, Node};

use sd_core_sync::SyncManager;
use sd_prisma::{
	prisma::{device, storage_statistics, PrismaClient},
	prisma_sync,
};
use sd_sync::{sync_db_not_null_entry, sync_entry, OperationFactory};
use sd_utils::uuid_to_bytes;

use std::{
	fmt::Display,
	hash::{Hash, Hasher},
	path::PathBuf,
	sync::{Arc, OnceLock},
};

use futures_concurrency::future::Join;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use sysinfo::{DiskExt, System, SystemExt};
use thiserror::Error;
use tokio::{spawn, sync::Mutex};
use tracing::error;
use uuid::Uuid;

pub mod watcher;

fn sys_guard() -> &'static Mutex<System> {
	static SYS: OnceLock<Mutex<System>> = OnceLock::new();
	SYS.get_or_init(|| Mutex::new(System::new_all()))
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum DiskType {
	SSD,
	HDD,
	Removable,
}

impl Display for DiskType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::SSD => "SSD",
			Self::HDD => "HDD",
			Self::Removable => "Removable",
		})
	}
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Volume {
	pub name: String,
	pub mount_points: Vec<PathBuf>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_capacity: u64,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub available_capacity: u64,
	pub disk_type: DiskType,
	pub file_system: Option<String>,
	pub is_root_filesystem: bool,
}

impl Hash for Volume {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name.hash(state);
		self.mount_points.iter().for_each(|mount_point| {
			// Hashing like this to ignore ordering between mount points
			mount_point.hash(state);
		});
		self.disk_type.hash(state);
		self.file_system.hash(state);
	}
}

impl PartialEq for Volume {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.disk_type == other.disk_type
			&& self.file_system == other.file_system
			// Leaving mount points for last because O(n * m)
			&& self
				.mount_points
				.iter()
				.all(|mount_point| other.mount_points.contains(mount_point))
	}
}

impl Eq for Volume {}

#[derive(Error, Debug)]
pub enum VolumeError {
	#[error("Database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("FromUtf8Error: {0}")]
	FromUtf8(#[from] std::string::FromUtf8Error),
	#[error(transparent)]
	Sync(#[from] sd_core_sync::Error),
}

impl From<VolumeError> for rspc::Error {
	fn from(e: VolumeError) -> Self {
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}

#[cfg(target_os = "linux")]
pub async fn get_volumes() -> Vec<Volume> {
	use std::{collections::HashMap, path::Path};

	let mut sys = sys_guard().lock().await;
	sys.refresh_disks_list();

	let mut volumes: Vec<Volume> = Vec::new();
	let mut path_to_volume_index = HashMap::new();
	for disk in sys.disks() {
		let disk_name = disk.name();
		let mount_point = disk.mount_point().to_path_buf();
		let file_system = String::from_utf8(disk.file_system().to_vec())
			.map(|s| s.to_uppercase())
			.ok();
		let total_capacity = disk.total_space();
		let available_capacity = disk.available_space();
		let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		let mut disk_path: PathBuf = PathBuf::from(disk_name);
		if file_system.as_ref().map(|fs| fs == "ZFS").unwrap_or(false) {
			// Use a custom path for ZFS disks to avoid conflicts with normal disks paths
			disk_path = Path::new("zfs://").join(disk_path);
		} else {
			// Ignore non-devices disks (overlay, fuse, tmpfs, etc.)
			if !disk_path.starts_with("/dev") {
				continue;
			}

			// Ensure disk has a valid device path
			let real_path = match tokio::fs::canonicalize(disk_name).await {
				Err(e) => {
					error!(?disk_name, ?e, "Failed to canonicalize disk path;",);
					continue;
				}
				Ok(real_path) => real_path,
			};

			// Check if disk is a symlink to another disk
			if real_path != disk_path {
				// Disk is a symlink to another disk, assign it to the same volume
				path_to_volume_index.insert(
					real_path.into_os_string(),
					path_to_volume_index
						.get(disk_name)
						.cloned()
						.unwrap_or(path_to_volume_index.len()),
				);
			}
		}

		if let Some(volume_index) = path_to_volume_index.get(disk_name) {
			// Disk already has a volume assigned, update it
			let volume: &mut Volume = volumes
				.get_mut(*volume_index)
				.expect("Volume index is present so the Volume must be present too");

			// Update mount point if not already present
			let mount_points = &mut volume.mount_points;
			if mount_point.iter().all(|p| *p != mount_point) {
				mount_points.push(mount_point);
				let mount_points_to_check = mount_points.clone();
				mount_points.retain(|candidate| {
					!mount_points_to_check
						.iter()
						.any(|path| candidate.starts_with(path) && candidate != path)
				});
				if !volume.is_root_filesystem {
					volume.is_root_filesystem = is_root_filesystem;
				}
			}

			// Update mount capacity, it can change between mounts due to quotas (ZFS, BTRFS?)
			if volume.total_capacity < total_capacity {
				volume.total_capacity = total_capacity;
			}

			// This shouldn't change between mounts, but just in case
			if volume.available_capacity > available_capacity {
				volume.available_capacity = available_capacity;
			}

			continue;
		}

		// Assign volume to disk path
		path_to_volume_index.insert(disk_path.into_os_string(), volumes.len());

		let mut name = disk_name.to_string_lossy().to_string();
		if name.replace(char::REPLACEMENT_CHARACTER, "") == "" {
			name = "Unknown".to_string()
		}

		volumes.push(Volume {
			name,
			disk_type: if disk.is_removable() {
				DiskType::Removable
			} else {
				match disk.kind() {
					sysinfo::DiskKind::SSD => DiskType::SSD,
					sysinfo::DiskKind::HDD => DiskType::HDD,
					_ => DiskType::Removable,
				}
			},
			file_system,
			mount_points: vec![mount_point],
			total_capacity,
			available_capacity,
			is_root_filesystem,
		});
	}

	volumes
}

#[cfg(target_os = "ios")]
pub async fn get_volumes() -> Vec<Volume> {
	use std::os::unix::fs::MetadataExt;

	use icrate::{
		objc2::runtime::{Class, Object},
		objc2::{msg_send, sel},
		Foundation::{self, ns_string, NSFileManager, NSFileSystemSize, NSNumber, NSString},
	};

	let mut volumes: Vec<Volume> = Vec::new();

	unsafe {
		let file_manager = NSFileManager::defaultManager();

		let root_dir = NSString::from_str("/");

		let root_dir_ref = root_dir.as_ref();

		let attributes = file_manager
			.attributesOfFileSystemForPath_error(root_dir_ref)
			.unwrap();

		let attributes_ref = attributes.as_ref();

		// Total space
		let key = NSString::from_str("NSFileSystemSize");
		let key_ref = key.as_ref();

		let t = attributes_ref.get(key_ref).unwrap();
		let total_space: u64 = msg_send![t, unsignedLongLongValue];

		// Used space
		let key = NSString::from_str("NSFileSystemFreeSize");
		let key_ref = key.as_ref();

		let t = attributes_ref.get(key_ref).unwrap();
		let free_space: u64 = msg_send![t, unsignedLongLongValue];

		volumes.push(Volume {
			name: "Root".to_string(),
			disk_type: DiskType::SSD,
			file_system: Some("APFS".to_string()),
			mount_points: vec![PathBuf::from("/")],
			total_capacity: total_space,
			available_capacity: free_space,
			is_root_filesystem: true,
		});
	}

	volumes
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ImageSystemEntity {
	mount_point: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ImageInfo {
	system_entities: Vec<ImageSystemEntity>,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct HDIUtilInfo {
	images: Vec<ImageInfo>,
}

// Android does not work via sysinfo and JNI is a pain to maintain. Therefore, we use React-Native-FS to get the volume data of the device.
// We leave the function though to be built for Android because otherwise, the build will fail.
#[cfg(not(any(target_os = "linux", target_os = "ios")))]
pub async fn get_volumes() -> Vec<Volume> {
	use futures::future;
	use tokio::process::Command;

	let mut sys = sys_guard().lock().await;
	sys.refresh_disks_list();

	// Ignore mounted DMGs
	#[cfg(target_os = "macos")]
	let dmgs = &Command::new("hdiutil")
		.args(["info", "-plist"])
		.output()
		.await
		.map_err(|e| error!(?e, "Failed to execute hdiutil;"))
		.ok()
		.and_then(|wmic_process| {
			use std::str::FromStr;

			if wmic_process.status.success() {
				let info: Result<HDIUtilInfo, _> = plist::from_bytes(&wmic_process.stdout);
				match info {
					Err(e) => {
						error!(?e, "Failed to parse hdiutil output;");
						None
					}
					Ok(info) => Some(
						info.images
							.into_iter()
							.flat_map(|image| image.system_entities)
							.flat_map(|entity: ImageSystemEntity| entity.mount_point)
							.flat_map(|mount_point| PathBuf::from_str(mount_point.as_str()))
							.collect::<std::collections::HashSet<_>>(),
					),
				}
			} else {
				error!("Command hdiutil return error");
				None
			}
		});

	future::join_all(sys.disks().iter().map(|disk| async {
		#[cfg(not(windows))]
		let disk_name = disk.name();
		let mount_point = disk.mount_point().to_path_buf();

		#[cfg(windows)]
		let Ok((disk_name, mount_point)) = ({
			use normpath::PathExt;
			mount_point
				.normalize_virtually()
				.map(|p| (p.localize_name().to_os_string(), p.into_path_buf()))
		}) else {
			return None;
		};

		#[cfg(target_os = "macos")]
		{
			// Ignore mounted DMGs
			if dmgs
				.as_ref()
				.map(|dmgs| dmgs.contains(&mount_point))
				.unwrap_or(false)
			{
				return None;
			}

			if !(mount_point.starts_with("/Volumes") || mount_point.starts_with("/System/Volumes"))
			{
				return None;
			}
		}

		#[cfg(windows)]
		#[allow(clippy::needless_late_init)]
		let mut total_capacity;
		#[cfg(not(windows))]
		#[allow(clippy::needless_late_init)]
		let total_capacity;
		total_capacity = disk.total_space();

		let available_capacity = disk.available_space();
		let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		// Fix broken google drive partition size in Windows
		#[cfg(windows)]
		if total_capacity < available_capacity && is_root_filesystem {
			// Use available capacity as total capacity in the case we can't get the correct value
			total_capacity = available_capacity;

			let caption = mount_point.to_str();
			if let Some(caption) = caption {
				let mut caption = caption.to_string();

				// Remove path separator from Disk letter
				caption.pop();

				let wmic_output = Command::new("cmd")
					.args([
						"/C",
						&format!("wmic logical disk where Caption='{caption}' get Size"),
					])
					.output()
					.await
					.map_err(|e| error!(?e, "Failed to execute hdiutil;"))
					.ok()
					.and_then(|wmic_process| {
						if wmic_process.status.success() {
							String::from_utf8(wmic_process.stdout).ok()
						} else {
							error!("Command wmic return error");
							None
						}
					});

				if let Some(wmic_output) = wmic_output {
					match wmic_output.split("\r\r\n").collect::<Vec<&str>>()[1]
						.to_string()
						.trim()
						.parse::<u64>()
					{
						Err(e) => error!(?e, "Failed to parse wmic output;"),
						Ok(n) => total_capacity = n,
					}
				}
			}
		}

		let mut name = disk_name.to_string_lossy().to_string();
		if name.replace(char::REPLACEMENT_CHARACTER, "") == "" {
			name = "Unknown".to_string()
		}

		Some(Volume {
			name,
			disk_type: if disk.is_removable() {
				DiskType::Removable
			} else {
				match disk.kind() {
					sysinfo::DiskKind::SSD => DiskType::SSD,
					sysinfo::DiskKind::HDD => DiskType::HDD,
					_ => DiskType::Removable,
				}
			},
			mount_points: vec![mount_point],
			file_system: String::from_utf8(disk.file_system().to_vec()).ok(),
			total_capacity,
			available_capacity,
			is_root_filesystem,
		})
	}))
	.await
	.into_iter()
	.flatten()
	.collect::<Vec<Volume>>()
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

fn compute_stats<'v>(volumes: impl IntoIterator<Item = &'v Volume>) -> (u64, u64) {
	volumes
		.into_iter()
		.fold((0, 0), |(mut total, mut available), volume| {
			total += volume.total_capacity;
			available += volume.available_capacity;

			(total, available)
		})
}

async fn update_storage_statistics(
	db: &PrismaClient,
	sync: &SyncManager,
	total_capacity: u64,
	available_capacity: u64,
) -> Result<(), VolumeError> {
	let device_pub_id = sync.device_pub_id.to_db();

	let storage_statistics_pub_id = db
		.storage_statistics()
		.find_first(vec![storage_statistics::device::is(vec![
			device::pub_id::equals(device_pub_id.clone()),
		])])
		.select(storage_statistics::select!({ pub_id }))
		.exec()
		.await?
		.map(|s| s.pub_id);

	if let Some(storage_statistics_pub_id) = storage_statistics_pub_id {
		let (sync_params, db_params) = [
			sync_db_not_null_entry!(total_capacity as i64, storage_statistics::total_capacity),
			sync_db_not_null_entry!(
				available_capacity as i64,
				storage_statistics::available_capacity
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_update(
				prisma_sync::storage_statistics::SyncId {
					pub_id: storage_statistics_pub_id.clone(),
				},
				sync_params,
			),
			db.storage_statistics()
				.update(
					storage_statistics::pub_id::equals(storage_statistics_pub_id),
					db_params,
				)
				// We don't need any data here, just the id avoids receiving the entire object
				// as we can't pass an empty select macro call
				.select(storage_statistics::select!({ id })),
		)
		.await?;
	} else {
		let new_storage_statistics_id = uuid_to_bytes(&Uuid::now_v7());

		let (sync_params, db_params) = [
			sync_db_not_null_entry!(total_capacity as i64, storage_statistics::total_capacity),
			sync_db_not_null_entry!(
				available_capacity as i64,
				storage_statistics::available_capacity
			),
			(
				sync_entry!(
					prisma_sync::device::SyncId {
						pub_id: device_pub_id.clone()
					},
					storage_statistics::device
				),
				storage_statistics::device::connect(device::pub_id::equals(device_pub_id)),
			),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_create(
				prisma_sync::storage_statistics::SyncId {
					pub_id: new_storage_statistics_id.clone(),
				},
				sync_params,
			),
			db.storage_statistics()
				.create(new_storage_statistics_id, db_params)
				// We don't need any data here, just the id avoids receiving the entire object
				// as we can't pass an empty select macro call
				.select(storage_statistics::select!({ id })),
		)
		.await?;
	}

	Ok(())
}

pub fn save_storage_statistics(node: &Node) {
	spawn({
		let libraries = Arc::clone(&node.libraries);
		async move {
			let (total_capacity, available_capacity) = compute_stats(&get_volumes().await);

			libraries
				.get_all()
				.await
				.into_iter()
				.map(move |library: Arc<Library>| async move {
					let Library { db, sync, .. } = &*library;

					update_storage_statistics(db, sync, total_capacity, available_capacity).await
				})
				.collect::<Vec<_>>()
				.join()
				.await
				.into_iter()
				.for_each(|res| {
					if let Err(e) = res {
						error!(?e, "Failed to save storage statistics;");
					}
				});
		}
	});
}
