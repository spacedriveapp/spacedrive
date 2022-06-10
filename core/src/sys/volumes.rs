// use crate::native;
use crate::{library::LibraryContext, prisma::volume::*};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
// #[cfg(not(target_os = "macos"))]
use std::process::Command;
// #[cfg(not(target_os = "macos"))]
use sysinfo::{DiskExt, System, SystemExt};

use super::SysError;

#[derive(Serialize, Deserialize, Debug, Default, Clone, TS)]
#[repr(C)]
#[ts(export)]
pub struct Volume {
	pub name: String,
	pub mount_point: String,
	pub total_capacity: u64,
	pub available_capacity: u64,
	pub is_removable: bool,
	pub disk_type: Option<String>,
	pub file_system: Option<String>,
	pub is_root_filesystem: bool,
}

impl Volume {
	pub async fn save(ctx: &LibraryContext) -> Result<(), SysError> {
		let volumes = Self::get_volumes()?;

		// enter all volumes associate with this client add to db
		for volume in volumes {
			ctx.db
				.volume()
				.upsert(
					node_id_mount_point_name(
						ctx.node_local_id.clone(),
						volume.mount_point.to_string(),
						volume.name.to_string(),
					),
					(
						node_id::set(ctx.node_local_id),
						name::set(volume.name),
						mount_point::set(volume.mount_point),
						vec![
							disk_type::set(volume.disk_type.clone()),
							filesystem::set(volume.file_system.clone()),
							total_bytes_capacity::set(volume.total_capacity.to_string()),
							total_bytes_available::set(volume.available_capacity.to_string()),
						],
					),
					vec![
						disk_type::set(volume.disk_type),
						filesystem::set(volume.file_system),
						total_bytes_capacity::set(volume.total_capacity.to_string()),
						total_bytes_available::set(volume.available_capacity.to_string()),
					],
				)
				.exec()
				.await?;
		}
		// cleanup: remove all unmodified volumes associate with this client

		Ok(())
	}
	pub fn get_volumes() -> Result<Vec<Volume>, SysError> {
		let all_volumes: Vec<Volume> = System::new_all()
			.disks()
			.iter()
			.map(|disk| {
				let mut total_space = disk.total_space();
				let mut mount_point = disk.mount_point().to_str().unwrap_or("/").to_string();
				let available_space = disk.available_space();
				let mut name = disk.name().to_str().unwrap_or("Volume").to_string();
				let is_removable = disk.is_removable();

				let file_system = String::from_utf8(disk.file_system().to_vec())
					.unwrap_or_else(|_| "Err".to_string());

				let disk_type = match disk.type_() {
					sysinfo::DiskType::SSD => "SSD".to_string(),
					sysinfo::DiskType::HDD => "HDD".to_string(),
					_ => "Removable Disk".to_string(),
				};

				if cfg!(target_os = "macos") && mount_point == "/"
					|| mount_point == "/System/Volumes/Data"
				{
					name = "Macintosh HD".to_string();
					mount_point = "/".to_string();
				}

				if total_space < available_space && cfg!(target_os = "windows") {
					let mut caption = mount_point.clone();
					caption.pop();
					let wmic_process = Command::new("cmd")
						.args([
							"/C",
							&format!("wmic logical disk where Caption='{caption}' get Size"),
						])
						.output()
						.expect("failed to execute process");
					let wmic_process_output = String::from_utf8(wmic_process.stdout).unwrap();
					let parsed_size =
						wmic_process_output.split("\r\r\n").collect::<Vec<&str>>()[1].to_string();

					if let Ok(n) = parsed_size.trim().parse::<u64>() {
						total_space = n;
					}
				}

				Volume {
					name,
					mount_point: mount_point.clone(),
					total_capacity: total_space,
					available_capacity: available_space,
					is_removable,
					disk_type: Some(disk_type),
					file_system: Some(file_system),
					is_root_filesystem: mount_point == "/",
				}
			})
			.collect();

		let volumes = all_volumes
			.clone()
			.into_iter()
			.filter(|volume| !volume.mount_point.starts_with("/System"))
			.collect();

		Ok(volumes)
	}
}

// #[test]
// fn test_get_volumes() {
//   let volumes = get_volumes().unwrap();
//   dbg!(&volumes);
//   assert!(volumes.len() > 0);
// }

// Adapted from: https://github.com/kimlimjustin/xplorer/blob/f4f3590d06783d64949766cc2975205a3b689a56/src-tauri/src/drives.rs
