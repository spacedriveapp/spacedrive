use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::{ffi::OsString, fmt::Display, path::PathBuf};
use sysinfo::{DiskExt, System, SystemExt};
use thiserror::Error;
use tokio::sync::Mutex;

fn sys_guard() -> &'static Mutex<System> {
	static SYS: OnceLock<Mutex<System>> = OnceLock::new();
	SYS.get_or_init(|| Mutex::new(System::new_all()))
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
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
	pub name: OsString,
	pub mount_point: Vec<PathBuf>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_capacity: u64,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub available_capacity: u64,
	pub is_removable: bool,
	pub disk_type: DiskType,
	pub file_system: Option<String>,
	pub is_root_filesystem: bool,
}

#[derive(Error, Debug)]
pub enum VolumeError {
	#[error("Database error: {0}")]
	DatabaseErr(#[from] prisma_client_rust::QueryError),
	#[error("FromUtf8Error: {0}")]
	FromUtf8Error(#[from] std::string::FromUtf8Error),
}

impl From<VolumeError> for rspc::Error {
	fn from(e: VolumeError) -> Self {
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
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

pub async fn get_volumes() -> Vec<Volume> {
	let mut sys = sys_guard().lock().await;

	let mut disks: HashMap<OsString, Volume> = HashMap::new();

	sys.refresh_disks_list();

	// for loo sys.disks
	for disk in sys.disks() {
		let disk_name = disk.name().to_os_string();
		let mount_point = disk.mount_point().to_path_buf();

		#[cfg(unix)]
		{
			use std::os::unix::ffi::OsStrExt;

			// Ignore non-devices disks (overlay, fuse, tmpfs, etc.)
			#[cfg(target_os = "linux")]
			if !disk_name.as_bytes().starts_with(b"/dev") {
				continue;
			}

			#[cfg(target_os = "macos")]
			if !disk_name.as_bytes().starts_with(b"/Volumes") {
				continue;
			}
		}

		if let Some(disk) = disks.get_mut(&disk_name) {
			disk.mount_point.push(mount_point);
			continue;
		}

		let total_capacity = disk.total_space();
		let available_capacity = disk.available_space();
		let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		// Fix broken google drive partition size in Windows
		#[cfg(target_os = "windows")]
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
					.ok()
					.and_then(|wmic_process| {
						if wmic_process.status.success() {
							String::from_utf8(wmic_process.stdout).ok()
						} else {
							None
						}
					});

				if let Some(wmic_output) = wmic_output {
					if let Ok(n) = wmic_output.split("\r\r\n").collect::<Vec<&str>>()[1]
						.to_string()
						.trim()
						.parse::<u64>()
					{
						total_capacity = n;
					}
				}
			}
		}

		disks.insert(
			disk_name.clone(),
			Volume {
				name: disk_name,
				disk_type: match disk.type_() {
					sysinfo::DiskType::SSD => DiskType::SSD,
					sysinfo::DiskType::HDD => DiskType::HDD,
					_ => DiskType::Removable,
				},
				mount_point: vec![mount_point],
				file_system: String::from_utf8(disk.file_system().to_vec()).ok(),
				is_removable: disk.is_removable(),
				total_capacity,
				available_capacity,
				is_root_filesystem,
			},
		);
	}

	disks.values().cloned().collect()
}

// #[test]
// fn test_get_volumes() {
//   let volumes = get_volumes()?;
//   dbg!(&volumes);
//   assert!(volumes.len() > 0);
// }

// Adapted from: https://github.com/kimlimjustin/xplorer/blob/f4f3590d06783d64949766cc2975205a3b689a56/src-tauri/src/drives.rs
