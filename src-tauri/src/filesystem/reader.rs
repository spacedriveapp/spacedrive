use crate::commands::DB_INSTANCE;
use crate::db;
use crate::db::entity::file;
use crate::filesystem::{checksum, init};
use crate::util::time;
use anyhow::{Context, Result};
use sea_orm::entity::*;
use sea_orm::QueryFilter;
use std::ffi::OsStr;
use std::{fs, path};
pub enum ReaderError {
  FailedToGetPrimaryLibrary,
}

pub async fn path(path: &str) -> Result<()> {
  let db = DB_INSTANCE.get().unwrap();

  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;

  if metadata.is_dir() {
    // read_dir().await?;
  } else {
    read_file(&path, path_buff, metadata).await?;
  }
  Ok(())
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn read_file(path: &str, path_buff: path::PathBuf, metadata: fs::Metadata) -> Result<()> {
  let db = DB_INSTANCE.get().unwrap();
  let size = metadata.len();
  let meta_checksum = checksum::create_meta_checksum(path.to_owned(), size)?;

  let primary_library = init::get_primary_library(&db).await?;

  let existing_files = file::Entity::find()
    .filter(file::Column::MetaChecksum.contains(&meta_checksum))
    .all(&db)
    .await?;

  if existing_files.len() == 0 {
    let file = file::ActiveModel {
      meta_checksum: Set(meta_checksum),
      name: Set(extract_name(path_buff.file_name())),
      extension: Set(extract_name(path_buff.extension())),
      uri: Set(path.to_owned()),
      library_id: Set(primary_library.id),
      size_in_bytes: Set(size.to_string()),
      date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
      date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      date_indexed: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      ..Default::default()
    };

    let file = file.save(&db).await?;

    println!("FILE: {:?}", file);

    Ok(())
  } else {
    let file = &existing_files[0];
    Ok(())
  }
}

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
  os_string
    .unwrap_or_default()
    .to_str()
    .unwrap_or_default()
    .to_owned()
}
