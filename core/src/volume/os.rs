use super::{MountType, Volume};
use serde::Deserialize;
use std::{path::PathBuf, sync::OnceLock};
use sysinfo::{DiskExt, System, SystemExt};
use tokio::sync::Mutex;
use tracing::error;

fn derive_mount_type(mount_point: &PathBuf, disk: &sysinfo::Disk) -> MountType {
	if disk.is_removable() {
		return MountType::External;
	}

	if let Some(mount_str) = mount_point.to_str() {
		// Network volume detection based on typical network paths
		if mount_str.starts_with("//")
			|| mount_str.starts_with("/mnt/network")
			|| mount_str.starts_with("/Volumes/network")
		{
			return MountType::Network;
		}

		// System volume detection
		if mount_str == "/"
			|| mount_str.starts_with("/System")
			|| mount_str.starts_with("/boot")
			|| mount_str.starts_with("/mnt/system")
		{
			return MountType::System;
		}

		#[cfg(windows)]
		{
			// Windows system drive (C:\\) detection
			if mount_str.starts_with("C:\\") {
				return MountType::System;
			}
		}
	}

	MountType::System // Default to System if no other condition matches
}
// Android does not work via sysinfo and JNI is a pain to maintain. Therefore, we use React-Native-FS to get the volume data of the device.
// We leave the function though to be built for Android because otherwise, the build will fail.
#[cfg(not(any(target_os = "linux", target_os = "ios")))]
pub async fn get_volumes() -> Vec<Volume> {
	use futures::future;
	use tokio::process::Command;

	use crate::volume::{DiskType, FileSystem, MountType};

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

	future::join_all(sys.disks().iter().map(|disk| async move {
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
		// let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		let mount_type = derive_mount_type(&mount_point, disk.clone());

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

		// Match the result and convert it to your FileSystem enum
		let file_system = match String::from_utf8(disk.file_system().to_vec()).ok() {
			Some(fs_str) => FileSystem::from_string(&fs_str),
			None => FileSystem::Other("Unknown".to_string()),
		};

		Some(Volume {
			id: None,
			pub_id: None,
			name,
			disk_type: match disk.kind() {
				sysinfo::DiskKind::SSD => DiskType::SSD,
				sysinfo::DiskKind::HDD => DiskType::HDD,
				_ => DiskType::Unknown,
			},
			file_system,
			mount_point,
			mount_type,
			total_bytes_capacity: total_capacity,
			total_bytes_available: available_capacity,
			write_speed_mbps: None,
			read_speed_mbps: None,
			is_mounted: true,
			error_status: None,
			read_only: false,
		})
	}))
	.await
	.into_iter()
	.flatten()
	.collect::<Vec<Volume>>()
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
			mount_points: PathBuf::from("/"),
			total_capacity: total_space,
			available_capacity: free_space,
			is_root_filesystem: true,
		});
	}

	volumes
}

fn sys_guard() -> &'static Mutex<System> {
	static SYS: OnceLock<Mutex<System>> = OnceLock::new();
	SYS.get_or_init(|| Mutex::new(System::new_all()))
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
