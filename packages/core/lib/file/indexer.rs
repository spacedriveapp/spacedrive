use crate::db::connection::DB_INSTANCE;
use crate::db::entity::file;
use crate::file::{checksum::create_meta_checksum, init};
use crate::util::time;
use anyhow::Result;
use sea_orm::QueryTrait;
use sea_orm::{entity::*, DatabaseBackend, QueryFilter};
// use sea_orm::ActiveModelTrait;
// use sea_orm::QueryFilter;
use futures::{
    stream::{self, StreamExt},
    Stream,
};
use sea_orm::ConnectionTrait;
use sea_orm::QueryOrder;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::time::Instant;
use std::{fs, path::PathBuf};
use walkdir::{DirEntry, WalkDir};
// creates a vector of valid path buffers from a directory
pub async fn scan(path: &str) -> Result<()> {
    println!("Scanning directory: {}", &path);
    let db = DB_INSTANCE.get().unwrap();
    let primary_library = init::get_primary_library(&db).await?;

    let mut next_file_id = match file::Entity::find()
        .order_by_desc(file::Column::Id)
        .one(db)
        .await
    {
        Ok(file) => file.map_or(0, |file| file.id),
        Err(_) => 0,
    };

    let mut get_id = || {
        next_file_id += 1;
        next_file_id
    };

    // store every valid path discovered
    let mut paths: Vec<(PathBuf, u32, Option<u32>, u32)> = Vec::new();
    // store a hashmap of directories to their file ids for fast lookup
    let mut dirs: HashMap<String, u32> = HashMap::new();
    let scan_start = Instant::now();
    // walk through directory recursively
    for entry in WalkDir::new(path).into_iter().filter_entry(|dir| {
        let approved = !is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir);
        approved
    }) {
        let entry = entry?;
        let path = entry.path();
        println!("{:?}", &path);

        let parent_path = path
            .parent()
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap_or("");
        let parent_dir_id = dirs.get(&*parent_path);
        println!("{:?}, {:?}", &path, &parent_dir_id);

        let file_id = get_id();
        paths.push((
            path.to_owned(),
            file_id,
            parent_dir_id.cloned(),
            primary_library.id,
        ));

        if entry.file_type().is_dir() {
            dirs.insert(path.to_str().unwrap().to_owned(), file_id);
        }
    }
    let db_write_start = Instant::now();
    let scan_read_time = scan_start.elapsed();

    for chunk in paths.chunks(100) {
        // vector to store active models
        let mut files: Vec<file::ActiveModel> = Vec::new();
        for (path, file_id, parent_dir_id, library_id) in chunk {
            files.push(
                create_active_file_model(
                    &path,
                    &file_id,
                    parent_dir_id.as_ref(),
                    library_id.clone(),
                )
                .unwrap(),
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
    // in chunks of 100, insert files into the database via transaction
    // for chunk in paths.chunks(25).into_iter() {
    //     println!("preparing chunk of {} files", chunk.len());

    //     let mut files: Vec<file::ActiveModel> = Vec::new();

    //     for (path, file_id, parent_dir_id, library_id) in chunk {
    //         files.push(
    //             create_active_file_model(path, file_id, parent_dir_id.as_ref(), library_id.clone())
    //                 .unwrap(),
    //         );
    //     }

    //     file::Entity::insert_many(files).exec(db).await?;

    // let txn = db.begin().await?;
    // for (path, file_id, parent_dir_id, library_id) in chunk {
    //     let active_model =
    //         create_active_file_model(path, file_id, parent_dir_id.as_ref(), library_id.clone())
    //             .unwrap();
    //     println!("{:?}", active_model.get_primary_key_value());
    //     active_model.save(&txn).await?;
    // }
    // txn.commit().await?;

    // stream::iter(files.iter()).filter_map(|active_file_model| async {
    //     active_file_model.save(&txn).await.unwrap();
    //     Some(())
    // });

    // chunk.iter().for_each(|(path, file_id)| {
    //     let parent_path = path
    //         .parent()
    //         .unwrap_or(Path::new(""))
    //         .to_str()
    //         .unwrap_or("");
    //     let parent_dir_id = dirs.get(&*parent_path);
    //     println!("{:?}, {:?}", &path, &parent_dir_id);

    //     let active_model =
    //         create_active_file_model(file_id, path, parent_dir_id, primary_library.id).unwrap();

    //     active_model.save(&txn);
    // });

    // for file in chunk {
    //     let parent_path = file
    //         .0
    //         .parent()
    //         .unwrap_or(Path::new(""))
    //         .to_str()
    //         .unwrap_or("");
    //     let parent_dir_id = dirs.get(&*parent_path);
    //     println!("{:?}, {:?}", &file.0, &parent_dir_id);

    //     let active_model =
    //         create_active_file_model(file.1, file.0.clone(), parent_dir_id, primary_library.id)
    //             .await?;

    //     active_model.save(&txn).await?; // problem line
    // }

    //     println!("inserted {} files", chunk.len());
    // }
    Ok(())
}

// reads a file at a path and creates an ActiveModel with metadata
fn create_active_file_model(
    path: &PathBuf,
    id: &u32,
    parent_id: Option<&u32>,
    library_id: u32,
) -> Result<file::ActiveModel> {
    let metadata = fs::metadata(&path)?;
    let size = metadata.len();
    let meta_checksum = create_meta_checksum(path.to_str().unwrap_or_default(), size)?;

    Ok(file::ActiveModel {
        id: Set(*id),
        is_dir: Set(metadata.is_dir()),
        parent_id: Set(parent_id.map(|x| x.clone())),
        meta_checksum: Set(meta_checksum.to_owned()),
        name: Set(extract_name(path.file_name())),
        extension: Set(extract_name(path.extension())),
        encryption: Set(file::Encryption::None),
        uri: Set(path.to_str().unwrap().to_owned()),
        library_id: Set(library_id),
        size_in_bytes: Set(size.to_string()),
        date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
        date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
        date_indexed: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
        ..Default::default()
    })
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
// pub async fn path(path: &str, parent_id: Option<u32>) -> Result<file::Model> {
//     let db = DB_INSTANCE.get().unwrap();

//     let path_buff = path::PathBuf::from(path);
//     let metadata = fs::metadata(&path)?;
//     let size = metadata.len();
//     let meta_checksum = create_meta_checksum(path_buff.to_str().unwrap_or_default(), size)?;

//     let existing_files = file::Entity::find()
//         .filter(file::Column::MetaChecksum.contains(&meta_checksum))
//         .all(db)
//         .await?;

//     if existing_files.len() == 0 {
//         let primary_library = init::get_primary_library(&db).await?;

//         let file = file::ActiveModel {
//             is_dir: Set(metadata.is_dir()),
//             parent_id: Set(parent_id.map(|x| x.clone())),
//             meta_checksum: Set(meta_checksum.to_owned()),
//             name: Set(extract_name(path_buff.file_name())),
//             extension: Set(extract_name(path_buff.extension())),
//             encryption: Set(file::Encryption::None),
//             uri: Set(path_buff.to_str().unwrap().to_owned()),
//             library_id: Set(primary_library.id),
//             size_in_bytes: Set(size.to_string()),
//             date_created: Set(Some(time::system_time_to_date_time(metadata.created())?)),
//             date_modified: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
//             date_indexed: Set(Some(time::system_time_to_date_time(metadata.modified())?)),
//             ..Default::default()
//         };

//         let _file = file.save(db).await.unwrap();

//         // REPLACE WHEN SEA QL PULLS THROUGH
//         let existing_files = file::Entity::find()
//             .filter(file::Column::MetaChecksum.contains(&meta_checksum))
//             .all(db)
//             .await?;

//         let existing_file = existing_files.first().unwrap().clone();
//         Ok(existing_file)
//     } else {
//         let existing_file = existing_files.first().unwrap().clone();
//         Ok(existing_file)
//     }
// }

// pub async fn scan(path: &str) -> Result<()> {
//     println!("Scanning directory: {}", &path);
//     // read the scan directory
//     let file = self::path(path, None).await?;

//     // hashmap to store refrences to directories
//     let mut dirs: HashMap<String, u32> = HashMap::new();

//     if file.is_dir {
//         // insert root directory
//         dirs.insert(path.to_owned(), file.id);
//         // iterate over files and subdirectories
//         for entry in WalkDir::new(path).into_iter().filter_entry(|dir| {
//             let approved = !is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir);
//             approved
//         }) {
//             let entry = entry?;
//             let child_path = entry.path().to_str().unwrap();
//             let parent_dir = get_parent_dir_id(&dirs, &entry);
//             // analyse the child file
//             let child_file = self::path(&child_path, parent_dir).await.unwrap();

//             println!(
//                 "Reading file from dir {:?} {:?} assigned id {:?}",
//                 parent_dir, child_path, child_file.id
//             );
//             // if this file is a directory, save in hashmap with database id
//             if child_file.is_dir {
//                 dirs.insert(child_path.to_owned(), child_file.id);
//             }

//             // println!("{}", entry.path().display());
//         }
//     }

//     println!("Scanning complete: {}", &path);
//     Ok(())
// }

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

// fn get_parent_dir_id(dirs: &HashMap<String, u32>, entry: &DirEntry) -> Option<u32> {
//     let path = entry.path();
//     let parent_path = path
//         .parent()
//         .unwrap_or_else(|| path)
//         .to_str()
//         .unwrap_or_default();
//     let parent_dir_id = dirs.get(&parent_path.to_owned());
//     match parent_dir_id {
//         Some(x) => Some(x.clone()),
//         None => None,
//     }
// }

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
