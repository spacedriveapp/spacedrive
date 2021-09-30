use crate::crypto;
use chrono::prelude::*;
use rusqlite::named_params;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path;

use tauri::InvokeError;

use crate::db;
// use crate::db::mapper::QueryMapper;
use crate::filesystem::checksum;
use crate::util::time;

#[derive(Debug, Serialize, Deserialize)]
pub struct File {
  // identity
  pub id: Option<u64>,
  pub buffer_checksum: Option<String>,
  pub meta_checksum: String,
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
// Generates meta checksum and extracts metadata
pub async fn read_file(path: &str) -> io::Result<File> {
  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;

  if metadata.is_dir() {
    // return Err();
  }

  let size = metadata.len();
  let meta_checksum = checksum::create_meta_hash(path.to_owned(), size)?;

  // assemble File struct with initial values
  let file = File {
    name: extract_name(path_buff.file_name()),
    extension: extract_name(path_buff.extension()),
    uri: path.to_owned(),
    size_in_bytes: size,
    date_created: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
    date_modified: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
    date_indexed: Utc::now(),
    encryption: crypto::Encryption::NONE,
    // this will be populated later, either by the database or other functions
    id: None,
    meta_checksum,
    buffer_checksum: None,
    ipfs_id: None,
    user_id: None,
    storage_device_id: None,
    capture_device_id: None,
    parent_file_id: None,
  };
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

pub async fn commit_file(file: &File) -> Result<(), InvokeError> {
  let connection = db::connection::get_connection()?;

  connection.execute("
    INSERT INTO files (uri, meta_checksum, buffer_checksum, name, extension, size_in_bytes, encryption, ipfs_id, user_id, storage_device_id, capture_device_id, parent_file_id, date_created, date_modified, date_indexed) VALUES (:uri, :meta_checksum, :buffer_checksum, :name, :extension, :size_in_bytes, :encryption, :ipfs_id, :user_id, :storage_device_id, :capture_device_id, :parent_file_id, :date_created, :date_modified, :date_indexed)
  ", named_params! {
    ":uri": &file.uri,
    ":meta_checksum": &file.meta_checksum,
    ":buffer_checksum": &file.buffer_checksum,
    ":name": &file.name,
    ":extension": &file.extension,
    ":size_in_bytes": &file.size_in_bytes,
    ":encryption": crypto::Encryption::NONE,
    ":ipfs_id": &file.ipfs_id,
    ":user_id": &file.user_id,
    ":storage_device_id": &file.storage_device_id,
    ":capture_device_id": &file.capture_device_id,
    ":parent_file_id": &file.parent_file_id,
    ":date_created": &file.date_created,
    ":date_modified": &file.date_modified,
    ":date_indexed": &file.date_indexed
});

  Ok(())
}

// const FILE_MAPPER: QueryMapper<File> = |row| {
//   Ok(File {
//     id: row.get(0)?,
//     buffer_checksum: row.get(1)?,
//     meta_checksum: row.get(2)?,
//     uri: row.get(3)?,
//     name: row.get(4)?,
//     extension: row.get(5)?,
//     size_in_bytes: row.get(6)?,
//     encryption: crypto::Encryption::from(row.get(7)?),
//     ipfs_id: row.get(8)?,
//     user_id: row.get(9)?,
//     storage_device_id: row.get(10)?,
//     capture_device_id: row.get(11)?,
//     parent_file_id: row.get(12)?,
//     date_created: chrono::DateTime::parse_from_str(row.get(13).unwrap(), "utc")?,
//     date_modified: row.get(14)?,
//     date_indexed: row.get(15)?,
//   })
// };
