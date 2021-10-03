use crate::commands::DB_INSTANCE;
use crate::db::entity::dir;
use crate::db::entity::file;
use crate::filesystem::{checksum, init};
use crate::util::time;
use anyhow::{anyhow, Result};
use sea_orm::entity::*;
use sea_orm::QueryFilter;
use std::ffi::OsStr;
use std::{fs, path};

pub struct FileOrDir {
  pub dir: Option<dir::Model>,
  pub file: Option<file::Model>,
}

// reads the metadata associated with the file or directory
// found at the supplied path and processes accordingly
pub async fn path(path: &str, dir_id: Option<u32>) -> Result<FileOrDir> {
  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;
  if metadata.is_dir() {
    Ok(FileOrDir {
      dir: Some(read_dir(path_buff, metadata, &dir_id).await?),
      file: None,
    })
  } else {
    Ok(FileOrDir {
      dir: None,
      file: Some(read_file(path_buff, metadata, &dir_id).await?),
    })
  }
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn read_file(
  path_buff: path::PathBuf,
  metadata: fs::Metadata,
  dir_id: &Option<u32>,
) -> Result<file::Model> {
  let db = DB_INSTANCE.get().unwrap();
  let size = metadata.len();
  let meta_checksum = checksum::create_meta_checksum(path_buff.to_str().unwrap_or_default(), size)?;

  let existing_files = file::Entity::find()
    .filter(file::Column::MetaChecksum.contains(&meta_checksum))
    .all(&db)
    .await?;

  if existing_files.len() == 0 {
    let primary_library = init::get_primary_library(&db).await?;

    let file = file::ActiveModel {
      meta_checksum: Set(meta_checksum.to_owned()),
      directory_id: Set(*dir_id),
      name: Set(extract_name(path_buff.file_name())),
      extension: Set(extract_name(path_buff.extension())),
      uri: Set(path_buff.to_str().unwrap().to_owned()),
      library_id: Set(primary_library.id),
      size_in_bytes: Set(size.to_string()),
      date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
      date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      date_indexed: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      ..Default::default()
    };

    let file = file.save(&db).await?;

    println!("FILE: {:?}", file);

    // REPLACE WHEN SEA QL PULLS THROUGH
    let existing_files = file::Entity::find()
      .filter(file::Column::MetaChecksum.contains(&meta_checksum))
      .all(&db)
      .await?;

    let existing_file = existing_files.first().unwrap().clone();
    Ok(existing_file)
  } else {
    let existing_file = existing_files.first().unwrap().clone();
    Ok(existing_file)
  }
}

pub async fn read_dir(
  path_buff: path::PathBuf,
  metadata: fs::Metadata,
  dir_id: &Option<u32>,
) -> Result<dir::Model> {
  let db = DB_INSTANCE.get().unwrap();

  let path_str = path_buff.to_str().unwrap();

  let file_name = path_buff.file_name().unwrap().to_str().unwrap().to_owned();

  if file_name.contains(".") {
    return Err(anyhow!("Directory is bundle, do not index"));
  }

  let existing_dirs = dir::Entity::find()
    .filter(dir::Column::Uri.contains(&path_str))
    .all(&db)
    .await?;

  if existing_dirs.is_empty() {
    let primary_library = init::get_primary_library(&db).await?;
    let directory = dir::ActiveModel {
      name: Set(path_buff.file_name().unwrap().to_str().unwrap().to_owned()),
      uri: Set(path_str.to_owned()),
      watch: Set(false),
      parent_directory_id: Set(*dir_id),
      library_id: Set(primary_library.id),
      date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
      date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      date_indexed: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
      ..Default::default()
    };

    let directory = directory.save(&db).await?;

    println!("DIR: {:?}", &directory);

    let existing_dirs = dir::Entity::find()
      .filter(dir::Column::Uri.contains(&path_str))
      .all(&db)
      .await?;

    let existing_dir = existing_dirs.first().unwrap().clone();
    Ok(existing_dir)
  } else {
    let existing_dir = existing_dirs.first().unwrap().clone();
    Ok(existing_dir)
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
