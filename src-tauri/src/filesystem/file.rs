use crate::crypto;
use crate::db;
use crate::filesystem::checksum;
use crate::util::time;
use chrono::{NaiveDateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::entity::*;
use sea_orm::InsertResult;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path;

// -------------------------------------
// Entity: File
// Represents an item discovered on the filesystem
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "files")]
// -------------------------------------
pub struct Model {
  // identity
  #[sea_orm(primary_key)]
  pub id: u32,
  pub buffer_checksum: String,
  pub meta_checksum: String,
  pub uri: String,
  // metadata
  pub name: String,
  pub extension: String,
  pub size_in_bytes: String,
  // pub encryption: crypto::Encryption,
  #[sea_orm(nullable)]
  pub ipfs_id: Option<String>,
  // ownership
  #[sea_orm(nullable)]
  pub storage_device_id: Option<u32>,
  #[sea_orm(nullable)]
  pub capture_device_id: Option<u32>,
  #[sea_orm(nullable)]
  pub parent_file_id: Option<u32>,
  // date
  pub date_created: NaiveDateTime,
  pub date_modified: NaiveDateTime,
  pub date_indexed: NaiveDateTime,
}

pub type File = ActiveModel;

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn read_file(path: &str) -> io::Result<File> {
  let db = db::connection::get_connection().await.unwrap();

  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;

  if metadata.is_dir() {
    // return Err();
  }

  let size = metadata.len();
  let meta_checksum = checksum::create_meta_hash(path.to_owned(), size)?;

  let file = File {
    meta_checksum: Set(meta_checksum),
    name: Set(extract_name(path_buff.file_name())),
    extension: Set(extract_name(path_buff.extension())),
    uri: Set(path.to_owned()),
    size_in_bytes: Set(format!("{}", size)),
    date_created: Set(time::system_time_to_date_time(metadata.created()).unwrap()),
    date_modified: Set(time::system_time_to_date_time(metadata.modified()).unwrap()),
    date_indexed: Set(time::system_time_to_date_time(metadata.modified()).unwrap()),
    ..Default::default()
  };

  let res = file.insert(&db).await.unwrap();

  Ok(res)
}

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
