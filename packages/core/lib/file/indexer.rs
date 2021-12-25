use crate::db::connection::DB_INSTANCE;
use crate::db::entity::file;
use crate::file::{checksum, init};
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
fn is_node_modules(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.contains("node_modules"))
        .unwrap_or(false)
}

fn is_app_bundle(entry: &DirEntry) -> bool {
    let is_dir = entry.metadata().unwrap().is_dir();
    let contains_dot = entry
        .file_name()
        .to_str()
        .map(|s| s.contains(".app") | s.contains(".bundle"))
        .unwrap_or(false);

    let is_app_bundle = is_dir && contains_dot;
    // if is_app_bundle {
    //   let path_buff = entry.path();
    //   let path = path_buff.to_str().unwrap();

    //   self::path(&path, );
    // }

    is_app_bundle
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
        for entry in WalkDir::new(path).into_iter().filter_entry(|dir| {
            let approved = !is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir);
            approved
        }) {
            let entry = entry?;
            let child_path = entry.path().to_str().unwrap();
            let parent_dir = get_parent_dir_id(&dirs, &entry);
            // analyse the child file
            let child_file = self::path(&child_path, parent_dir).await.unwrap();

            println!(
                "Reading file from dir {:?} {:?} assigned id {:?}",
                parent_dir, child_path, child_file.id
            );
            // if this file is a directory, save in hashmap with database id
            if child_file.is_dir {
                dirs.insert(child_path.to_owned(), child_file.id);
            }

            // println!("{}", entry.path().display());
        }
    }

    println!("Scanning complete: {}", &path);
    Ok(())
}

pub async fn test_scan(path: &str) -> Result<()> {
    let mut count: u32 = 0;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let child_path = entry.path().to_str().unwrap();
        count = count + 1;
        println!("Reading file from dir {:?}", child_path);
    }
    println!("files found {}", count);
    Ok(())
}

fn get_parent_dir_id(dirs: &HashMap<String, u32>, entry: &DirEntry) -> Option<u32> {
    let path = entry.path();
    let parent_path = path
        .parent()
        .unwrap_or_else(|| path)
        .to_str()
        .unwrap_or_default();
    let parent_dir_id = dirs.get(&parent_path.to_owned());
    match parent_dir_id {
        Some(x) => Some(x.clone()),
        None => None,
    }
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn path(path: &str, parent_id: Option<u32>) -> Result<file::Model> {
    let db = DB_INSTANCE.get().unwrap();

    let path_buff = path::PathBuf::from(path);
    let metadata = fs::metadata(&path)?;
    let size = metadata.len();
    let meta_checksum =
        checksum::create_meta_checksum(path_buff.to_str().unwrap_or_default(), size)?;

    let existing_files = file::Entity::find()
        .filter(file::Column::MetaChecksum.contains(&meta_checksum))
        .all(db)
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

        let _file = file.save(db).await.unwrap();

        // REPLACE WHEN SEA QL PULLS THROUGH
        let existing_files = file::Entity::find()
            .filter(file::Column::MetaChecksum.contains(&meta_checksum))
            .all(db)
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
