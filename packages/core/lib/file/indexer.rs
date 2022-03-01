use std::{collections::HashMap, ffi::OsStr, fs, path::Path, path::PathBuf, time::Instant};

use anyhow::Result;
use chrono::Utc;
use sea_orm::{entity::*, QueryOrder};
use walkdir::{DirEntry, WalkDir};

use crate::db::{connection::db, entity::file};
use crate::file::checksum::create_meta_integrity_hash;
use crate::library;
use crate::util::time;

use super::locations::get_location;
use super::watcher::watch_dir;

pub async fn scan_paths(location_id: u32) -> Result<()> {
    // get location by location_id from db and include location_paths
    let location = get_location(location_id).await?;

    scan(&location.path).await?;
    watch_dir(&location.path);

    Ok(())
}

// creates a vector of valid path buffers from a directory
pub async fn scan(path: &str) -> Result<()> {
    println!("Scanning directory: {}", &path);
    let current_library = library::loader::get().await?;

    let db = db().await.unwrap();

    // query db to highers id, so we can increment it for the new files indexed
    let mut next_file_id = match file::Entity::find()
        .order_by_desc(file::Column::Id)
        .one(db)
        .await
    {
        Ok(file) => file.map_or(0, |file| file.id),
        Err(_) => 0,
    };
    let mut get_id = || {
        next_file_id += 1; // increment id
        next_file_id
    };

    //check is path is a directory
    if !PathBuf::from(path).is_dir() {
        return Err(anyhow::anyhow!("{} is not a directory", path));
    }

    // store every valid path discovered
    let mut paths: Vec<(PathBuf, u32, Option<u32>, u32)> = Vec::new();
    // store a hashmap of directories to their file ids for fast lookup
    let mut dirs: HashMap<String, u32> = HashMap::new();
    // begin timer for logging purposes
    let scan_start = Instant::now();
    // walk through directory recursively
    for entry in WalkDir::new(path).into_iter().filter_entry(|dir| {
        let approved = !is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir);
        approved
    }) {
        // extract directory entry or log and continue if failed
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                println!("Error reading file {}", e);
                continue;
            }
        };
        let path = entry.path();

        let parent_path = path
            .parent()
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap_or("");
        let parent_dir_id = dirs.get(&*parent_path);
        println!("Discovered: {:?}, {:?}", &path, &parent_dir_id);

        let file_id = get_id();
        paths.push((
            path.to_owned(),
            file_id,
            parent_dir_id.cloned(),
            current_library.id,
        ));

        if entry.file_type().is_dir() {
            let _path = match path.to_str() {
                Some(path) => path.to_owned(),
                None => continue,
            };
            dirs.insert(_path, file_id);
        }
    }
    let db_write_start = Instant::now();
    let scan_read_time = scan_start.elapsed();

    for (i, chunk) in paths.chunks(100).enumerate() {
        println!("Writing {} files to db at chunk {}", chunk.len(), i);
        // vector to store active models
        let mut files: Vec<file::ActiveModel> = Vec::new();
        for (file_path, file_id, parent_dir_id, library_id) in chunk {
            // TODO: add location
            files.push(
                match create_active_file_model(
                    &file_path,
                    &file_id,
                    parent_dir_id.as_ref(),
                    path, // TODO: we'll need the location path directly from location object just in case we're re-scanning a portion
                    library_id.clone(),
                ) {
                    Ok(file) => file,
                    Err(e) => {
                        println!("Error creating file model from path {:?}: {}", file_path, e);
                        continue;
                    }
                },
            );
        }
        // insert chunk of files into db
        file::Entity::insert_many(files)
            .exec(db)
            .await
            .map_err(|e| {
                println!("{:?}", e.to_string());
                e
            })?;
    }
    println!(
        "scan of {:?} completed in {:?}. {:?} files found. db write completed in {:?}",
        path,
        scan_read_time,
        paths.len(),
        db_write_start.elapsed()
    );
    Ok(())
}

// reads a file at a path and creates an ActiveModel with metadata
fn create_active_file_model(
    uri: &PathBuf,
    id: &u32,
    parent_id: Option<&u32>,
    location_path: &str,
    library_id: u32,
) -> Result<file::ActiveModel> {
    let metadata = fs::metadata(&uri)?;
    let size = metadata.len();
    let mut meta_integrity_hash =
        create_meta_integrity_hash(uri.to_str().unwrap_or_default(), size)?;
    meta_integrity_hash.truncate(20);

    let mut location_relative_uri = uri
        .to_str()
        .unwrap()
        .split(location_path)
        .last()
        .unwrap()
        .to_owned();

    // if location_relative_uri is empty String return "/"
    location_relative_uri = match location_relative_uri.is_empty() {
        true => "/".to_owned(),
        false => location_relative_uri,
    };

    Ok(file::ActiveModel {
        id: Set(*id),
        is_dir: Set(metadata.is_dir()),
        parent_id: Set(parent_id.map(|x| x.clone())),
        meta_integrity_hash: Set(meta_integrity_hash),
        name: Set(extract_name(uri.file_stem())),
        extension: Set(extract_name(uri.extension())),
        encryption: Set(file::Encryption::None),
        uri: Set(location_relative_uri),
        library_id: Set(library_id),
        size_in_bytes: Set(size.to_string()),
        date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
        date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
        date_indexed: Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    })
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

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
    os_string
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_owned()
}

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
