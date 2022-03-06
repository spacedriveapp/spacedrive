use std::process::Command;
use sysinfo::{DiskExt, System, SystemExt};

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct Volume {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub is_removable: bool,
    pub disk_type: String,
    pub file_system: String,
}

pub fn get() -> Result<Vec<Volume>, String> {
    let volumes = System::new_all()
        .disks()
        .iter()
        .map(|disk| {
            let mut total_space = disk.total_space();
            let available_space = disk.available_space();
            let mount_point = disk.mount_point().to_str().unwrap_or("/").to_string();
            let name = disk.name().to_str().unwrap_or("Volume").to_string();
            let is_removable = disk.is_removable();

            let mut caption = mount_point.clone();
            caption.pop();

            let file_system = String::from_utf8(disk.file_system().to_vec())
                .unwrap_or_else(|_| "Err".to_string());

            let disk_type = match disk.type_() {
                sysinfo::DiskType::SSD => "SSD".to_string(),
                sysinfo::DiskType::HDD => "HDD".to_string(),
                _ => "Removable Disk".to_string(),
            };

            if total_space < available_space && cfg!(target_os = "windows") {
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
                mount_point,
                total_space,
                available_space,
                is_removable,
                disk_type,
                file_system,
            }
        })
        .collect();

    Ok(volumes)
}

// Adapted from: https://github.com/kimlimjustin/xplorer/blob/f4f3590d06783d64949766cc2975205a3b689a56/src-tauri/src/drives.rs
