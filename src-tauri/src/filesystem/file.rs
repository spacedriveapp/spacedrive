use crate::crypto;
use chrono::prelude::*;
use crossbeam::thread;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path;
use std::time::Instant;

use crate::filesystem::checksum;
use crate::util::time;

#[derive(Debug, Serialize, Deserialize)]
pub struct File {
  // identity
  pub id: Option<u64>,
  pub checksum: Option<String>,
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

// Read a file from path returning the File struct
// Generates checksum and extracts metadata
pub async fn read_file(path: &str) -> io::Result<File> {
  // let start = Instant::now();
  let path_buff = path::PathBuf::from(path);
  // extract metadata
  let metadata = match fs::metadata(&path) {
    Ok(metadata) => metadata,
    Err(e) => return Err(e),
  };

  // if metadata.is_dir() {
  //   return Err();
  // }

  // let checksum = thread::scope(|s| {
  //   let res = s.spawn(move |_| checksum::create_hash(path).unwrap());
  //   res.join()
  // })
  // .unwrap()
  // .unwrap();

  // let checksum = match checksum {
  //   Ok(metadata) => metadata, // Err(e) => return Err(e.into()),
  // };

  // generate checksum
  // let checksum = match checksum::create_hash(path) {
  //   Ok(checksum) => checksum,
  //   Err(e) => return Err(e),
  // };
  // assemble File struct with initial values
  let file = File {
    name: extract_name(path_buff.file_name()),
    extension: extract_name(path_buff.extension()),
    uri: path.to_owned(),
    size_in_bytes: metadata.len(),
    date_created: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
    date_modified: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
    date_indexed: Utc::now(),
    encryption: crypto::Encryption::NONE,
    // this will be populated later, either by the database or other functions
    id: None,
    checksum: None,
    ipfs_id: None,
    user_id: None,
    storage_device_id: None,
    capture_device_id: None,
    parent_file_id: None,
  };

  checksum::create_hash(path).await;

  Ok(file)
}

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
  os_string
    .unwrap_or_default()
    .to_str()
    .unwrap_or_default()
    .to_owned()
}
