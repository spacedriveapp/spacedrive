use crate::commands::DB_INSTANCE;
use crate::db::entity::file;
use crate::filesystem::{checksum, init};
use crate::util::time;
use anyhow::Result;
use sea_orm::entity::*;
use sea_orm::ActiveModelTrait;
use sea_orm::QueryFilter;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::{fs, path};
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
  entry
    .file_name()
    .to_str()
    .map(|s| s.starts_with("."))
    .unwrap_or(false)
}

fn is_app_bundle(entry: &DirEntry) -> bool {
  let is_dir = entry.metadata().unwrap().is_dir();
  let contains_dot = entry
    .file_name()
    .to_str()
    .map(|s| s.contains("."))
    .unwrap_or(false);

  is_dir && contains_dot
}

pub async fn scan(path: &str) -> Result<()> {
  println!("Scanning directory: {}", &path);
  // read the scan directory
  let file = self::path(path, None).await?;

  // hashmap to store refrences to directories
  let mut dirs: HashMap<String, u32> = HashMap::new();

  if file.is_dir {
    // insert root directory
    dirs.insert(path.to_owned(), file.id);
    // iterate over files and subdirectories
    for entry in WalkDir::new(path)
      .into_iter()
      .filter_entry(|e| !is_hidden(e) && !is_app_bundle(e))
    {
      let entry = entry?;
      let path_buff = entry.path();
      let path = path_buff.to_str().unwrap();
      // get the parent directory from the path
      let parent = path_buff.parent().unwrap().to_str().unwrap();
      // get parent dir database id from hashmap
      let parent_dir = dirs.get(&parent.to_owned());
      // analyse the child file
      let child_file = self::path(&path, parent_dir).await.unwrap();
      // if this file is a directory, save in hashmap with database id
      if child_file.is_dir {
        dirs.insert(path.to_owned(), child_file.id);
      }

      // println!("{}", entry.path().display());
    }
  }
  Ok(())
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn path(path: &str, parent_id: Option<&u32>) -> Result<file::Model> {
  let db = DB_INSTANCE.get().unwrap();

  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path)?;
  let size = metadata.len();
  let meta_checksum = checksum::create_meta_checksum(path_buff.to_str().unwrap_or_default(), size)?;

  let existing_files = file::Entity::find()
    .filter(file::Column::MetaChecksum.contains(&meta_checksum))
    .all(&db)
    .await?;

  if existing_files.len() == 0 {
    let primary_library = init::get_primary_library(&db).await?;

    let file = file::ActiveModel {
      is_dir: Set(metadata.is_dir()),
      parent_id: Set(parent_id.map(|x| x.clone())),
      meta_checksum: Set(meta_checksum.to_owned()),
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

    let file = file.save(&db).await.unwrap();

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

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
  os_string
    .unwrap_or_default()
    .to_str()
    .unwrap_or_default()
    .to_owned()
}
