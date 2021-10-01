use crate::db;
use crate::filesystem::checksum;
use crate::util::time;
use sea_orm::entity::*;
use sea_orm::QueryFilter;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path;

use crate::db::entity::file;

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn read_file(path: &str) -> io::Result<file::ActiveModel> {
  let db = db::connection::get_connection().await.unwrap();

  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;

  let size = metadata.len();
  let meta_checksum = checksum::create_meta_hash(path.to_owned(), size)?;

  let existing_file = file::Entity::find()
    .filter(file::Column::MetaChecksum.contains(&meta_checksum))
    .all(&db)
    .await
    .unwrap();

  println!("Existing file found {:?}", existing_file);

  let file = file::ActiveModel {
    meta_checksum: Set(meta_checksum),
    name: Set(extract_name(path_buff.file_name())),
    extension: Set(extract_name(path_buff.extension())),
    uri: Set(path.to_owned()),
    size_in_bytes: Set(format!("{}", size)),
    date_created: Set(Some(
      time::system_time_to_date_time(metadata.created()).unwrap(),
    )),
    date_modified: Set(Some(
      time::system_time_to_date_time(metadata.modified()).unwrap(),
    )),
    date_indexed: Set(Some(
      time::system_time_to_date_time(metadata.modified()).unwrap(),
    )),
    ..Default::default()
  };

  let file = file
    .save(&db)
    .await
    .map_err(|error| println!("Failed to read file: {}", error))
    .unwrap();

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

// pub async fn commit_file(file: &File) -> Result<(), InvokeError> {
//   let connection = db::connection::get_connection()?;

// });
