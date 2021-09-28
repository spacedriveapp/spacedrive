use crate::crypto;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path;

use crate::filesystem::checksum;
use crate::util::time;

#[derive(Debug, Serialize, Deserialize)]
pub struct File {
  // identity
  pub id: Option<u64>,
  pub checksum: String,
  pub uri: String,
  // metadata
  pub name: String,
  pub extension: String,
  pub size_in_bytes: u64,
  pub encryption: crypto::Encryption,
  pub ipfs_id: Option<String>,
  // ownership
  pub user_id: Option<u64>,
  pub storage_device_id: Option<u64>,
  pub capture_device_id: Option<u64>,
  pub parent_file_id: Option<u64>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}

pub struct Directory {
  // identity
  pub id: Option<u64>,
  pub name: String,
  // calculations
  pub calculated_size_in_bytes: u64,
  pub calculated_file_count: u32,
  // ownership
  pub user_id: Option<u64>,
  pub storage_device_id: Option<u64>,
  pub parent_directory_id: Option<u32>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}

#[tauri::command]
pub fn read_file_command(path: &str) -> Result<File, String> {
  let file = read_file(path).unwrap();
  Ok(file)
}

pub fn read_file(path: &str) -> Result<File, String> {
  let path_buff = path::PathBuf::from(path);
  // extract metadata
  let metadata = fs::metadata(&path).unwrap();
  if metadata.is_dir() {
    panic!("Not a file, this is a directory");
  }

  let file = File {
    id: None,
    name: path_buff.file_name().unwrap().to_str().unwrap().to_owned(),
    extension: path_buff.extension().unwrap().to_str().unwrap().to_owned(),
    uri: path.to_owned(),
    checksum: checksum::create_hash(path).unwrap(),
    size_in_bytes: metadata.len(),
    date_created: time::system_time_to_date_time(metadata.created().unwrap()).unwrap(),
    date_modified: time::system_time_to_date_time(metadata.created().unwrap()).unwrap(),
    date_indexed: chrono::offset::Utc::now(),
    encryption: crypto::Encryption::NONE,
    ipfs_id: None,
    user_id: None,
    storage_device_id: None,
    capture_device_id: None,
    parent_file_id: None,
  };

  println!("file: {:?}", file);

  Ok(file)
}
