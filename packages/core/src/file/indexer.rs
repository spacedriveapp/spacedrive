use std::{collections::HashMap, ffi::OsStr, fs, path::Path, path::PathBuf, time::Instant};

use anyhow::{anyhow, Result};

use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use super::watcher::watch_dir;
use crate::db;
use crate::sys::locations::{create_location, get_location, LocationResource};
use crate::util::time;

pub async fn scan_loc(location_id: i64) -> Result<()> {
	// get location by location_id from db and include location_paths
	let location = get_location(location_id).await?;

	if let Some(path) = &location.path {
		scan_path(path).await?;
		watch_dir(path);
	}

	Ok(())
}

// creates a vector of valid path buffers from a directory
pub async fn scan_path(path: &str) -> Result<()> {
	println!("Scanning directory: {}", &path);
	// let current_library = library::loader::get().await?;

	let db = db::get().await.unwrap();

	let location = create_location(&path).await?;

	// query db to highers id, so we can increment it for the new files indexed
	#[derive(Deserialize, Serialize, Debug)]
	struct QueryRes {
		id: i64,
	}
	let mut next_file_id = match db._query_raw::<QueryRes>(r#"SELECT MAX(id) id FROM files"#).await {
		Ok(rows) => rows[0].id,
		Err(e) => Err(anyhow!("Error querying for next file id: {}", e))?,
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
	let mut paths: Vec<(PathBuf, i64, Option<i64>)> = Vec::new();
	// store a hashmap of directories to their file ids for fast lookup
	let mut dirs: HashMap<String, i64> = HashMap::new();
	// begin timer for logging purposes
	let scan_start = Instant::now();
	// walk through directory recursively
	for entry in WalkDir::new(path).into_iter().filter_entry(|dir| {
		let approved = !is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir) && !is_library(dir);
		approved
	}) {
		// extract directory entry or log and continue if failed
		let entry = match entry {
			Ok(entry) => entry,
			Err(e) => {
				println!("Error reading file {}", e);
				continue;
			},
		};
		let path = entry.path();

		let parent_path = path.parent().unwrap_or(Path::new("")).to_str().unwrap_or("");
		let parent_dir_id = dirs.get(&*parent_path);
		println!("Discovered: {:?}, {:?}", &path, &parent_dir_id);

		let file_id = get_id();
		paths.push((path.to_owned(), file_id, parent_dir_id.cloned()));

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
		let mut files: Vec<String> = Vec::new();
		for (file_path, file_id, parent_dir_id) in chunk {
			files.push(match prepare_model(&file_path, *file_id, &location, parent_dir_id) {
				Ok(file) => file,
				Err(e) => {
					println!("Error creating file model from path {:?}: {}", file_path, e);
					continue;
				},
			});
		}
		let raw_sql = format!(
			r#"
                INSERT INTO files (id, is_dir, location_id, parent_id, stem, name, extension, size_in_bytes, date_created, date_modified) 
                VALUES {}
            "#,
			files.join(", ")
		);
		let count = db._execute_raw(&raw_sql).await;
		println!("Inserted {:?} records", count);
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
fn prepare_model(file_path: &PathBuf, id: i64, location: &LocationResource, parent_id: &Option<i64>) -> Result<String> {
	let metadata = fs::metadata(&file_path)?;
	let location_path = location.path.as_ref().unwrap().as_str();
	let size = metadata.len();
	let name = extract_name(file_path.file_stem());
	let extension = extract_name(file_path.extension());

	let stem = match file_path.to_str() {
		Some(p) => p
			.clone()
			.strip_prefix(&location_path)
			.and_then(|p| p.strip_suffix(format!("{}{}", name, extension).as_str()))
			.unwrap_or_default(),
		None => return Err(anyhow!("{}", file_path.to_str().unwrap_or_default())),
	};

	Ok(construct_file_sql(
		id,
		metadata.is_dir(),
		location.id,
		parent_id.clone(),
		stem,
		&name,
		&extension,
		&size.to_string(),
		&time::system_time_to_date_time(metadata.created()).unwrap().to_string(),
		&time::system_time_to_date_time(metadata.modified()).unwrap().to_string(),
	))
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
	os_string.unwrap_or_default().to_str().unwrap_or_default().to_owned()
}

fn is_hidden(entry: &DirEntry) -> bool {
	entry.file_name().to_str().map(|s| s.starts_with(".")).unwrap_or(false)
}

fn is_library(entry: &DirEntry) -> bool {
	entry
        .path()
        .to_str()
        // make better this is shit
        .map(|s| s.contains("/Library/"))
        .unwrap_or(false)
}

fn is_node_modules(entry: &DirEntry) -> bool {
	entry.file_name().to_str().map(|s| s.contains("node_modules")).unwrap_or(false)
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

fn construct_file_sql(
	id: i64,
	is_dir: bool,
	location_id: i64,
	parent_id: Option<i64>,
	stem: &str,
	name: &str,
	extension: &str,
	size_in_bytes: &str,
	date_created: &str,
	date_modified: &str,
) -> String {
	format!(
		"({}, {}, {}, {}, \"{}\", \"{}\",\"{}\", \"{}\", \"{}\", \"{}\")",
		id,
		is_dir as u8,
		location_id,
		parent_id.map(|id| id.to_string()).unwrap_or("NULL".to_string()),
		stem,
		name,
		extension,
		size_in_bytes,
		date_created,
		date_modified
	)
}
