use crate::crypto;
use crate::filesystem::checksum;
use crate::util::time;
use chrono::prelude::*;
use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path;

// -------------------------------------
// Entity: File
// Represents an item discovered on the filesystem
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "files")]
// -------------------------------------
pub struct Model {
  // identity
  #[sea_orm(primary_key)]
  pub id: u64,
  pub buffer_checksum: String,
  pub meta_checksum: String,
  pub uri: String,
  // metadata
  pub name: String,
  pub extension: String,
  pub size_in_bytes: u64,
  // pub encryption: crypto::Encryption,
  #[sea_orm(nullable)]
  pub ipfs_id: Option<String>,
  // ownership
  #[sea_orm(nullable)]
  pub user_id: Option<u64>,
  #[sea_orm(nullable)]
  pub storage_device_id: Option<u64>,
  #[sea_orm(nullable)]
  pub capture_device_id: Option<u64>,
  #[sea_orm(nullable)]
  pub parent_file_id: Option<u64>,
  // date
  pub date_created: NaiveDateTime,
  pub date_modified: NaiveDateTime,
  pub date_indexed: NaiveDateTime,
}
pub type File = Model;

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
// pub async fn read_file(path: &str) -> io::Result<File> {
//   let path_buff = path::PathBuf::from(path);
//   let metadata = fs::metadata(&path)?;

//   if metadata.is_dir() {
//     // return Err();
//   }

//   let size = metadata.len();
//   let meta_checksum = checksum::create_meta_hash(path.to_owned(), size)?;

//   // assemble File struct with initial values
//   let file = File {
//     name: extract_name(path_buff.file_name()),
//     extension: extract_name(path_buff.extension()),
//     uri: path.to_owned(),
//     size_in_bytes: size,
//     date_created: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
//     date_modified: time::system_time_to_date_time(metadata.created()).unwrap_or(Utc::now()),
//     date_indexed: Utc::now(),
//     encryption: crypto::Encryption::NONE,
//     // this will be populated later, either by the database or other functions
//     id: None,
//     meta_checksum,
//     buffer_checksum: None,
//     ipfs_id: None,
//     user_id: None,
//     storage_device_id: None,
//     capture_device_id: None,
//     parent_file_id: None,
//   };
//   Ok(file)
// }

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
  os_string
    .unwrap_or_default()
    .to_str()
    .unwrap_or_default()
    .to_owned()
}

// pub async fn commit_file(file: &File) -> Result<(), InvokeError> {
//   let connection = db::connection::get_connection()?;

// });
