use crate::sys::{create_location, LocationResource};
use crate::CoreContext;
use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::prisma_models::PrismaValue;
use prisma_client_rust::raw;
use prisma_client_rust::raw::Raw;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::{collections::HashMap, fs, path::Path, path::PathBuf, time::Instant};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	Message(String),
}

static BATCH_SIZE: usize = 100;

// creates a vector of valid path buffers from a directory
pub async fn scan_path(
	ctx: &CoreContext,
	path: &str,
	on_progress: impl Fn(Vec<ScanProgress>) + Send + Sync + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
	let db = &ctx.database;
	let path = path.to_string();

	let location = create_location(&ctx, &path).await?;

	// query db to highers id, so we can increment it for the new files indexed
	#[derive(Deserialize, Serialize, Debug)]
	struct QueryRes {
		id: Option<i32>,
	}
	// grab the next id so we can increment in memory for batch inserting
	let first_file_id = match db
		._query_raw::<QueryRes>(raw!("SELECT MAX(id) id FROM file_paths"))
		.await
	{
		Ok(rows) => rows[0].id.unwrap_or(0),
		Err(e) => panic!("Error querying for next file id: {}", e),
	};

	//check is path is a directory
	if !PathBuf::from(&path).is_dir() {
		// return Err(anyhow::anyhow!("{} is not a directory", &path));
		panic!("{} is not a directory", &path);
	}
	let dir_path = path.clone();

	// spawn a dedicated thread to scan the directory for performance
	let (paths, scan_start, on_progress) = tokio::task::spawn_blocking(move || {
		// store every valid path discovered
		let mut paths: Vec<(PathBuf, i32, Option<i32>, bool)> = Vec::new();
		// store a hashmap of directories to their file ids for fast lookup
		let mut dirs: HashMap<String, i32> = HashMap::new();
		// begin timer for logging purposes
		let scan_start = Instant::now();

		let mut next_file_id = first_file_id;
		let mut get_id = || {
			next_file_id += 1;
			next_file_id
		};
		// walk through directory recursively
		for entry in WalkDir::new(&dir_path).into_iter().filter_entry(|dir| {
			let approved =
				!is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir) && !is_library(dir);
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

			println!("found: {:?}", path);

			let parent_path = path
				.parent()
				.unwrap_or(Path::new(""))
				.to_str()
				.unwrap_or("");
			let parent_dir_id = dirs.get(&*parent_path);

			let path_str = match path.as_os_str().to_str() {
				Some(path_str) => path_str,
				None => {
					println!("Error reading file {}", &path.display());
					continue;
				}
			};

			on_progress(vec![
				ScanProgress::Message(format!("{}", path_str)),
				ScanProgress::ChunkCount(paths.len() / BATCH_SIZE),
			]);

			let file_id = get_id();
			let file_type = entry.file_type();
			let is_dir = file_type.is_dir();

			if is_dir || file_type.is_file() {
				paths.push((path.to_owned(), file_id, parent_dir_id.cloned(), is_dir));
			}

			if is_dir {
				let _path = match path.to_str() {
					Some(path) => path.to_owned(),
					None => continue,
				};
				dirs.insert(_path, file_id);
			}
		}
		(paths, scan_start, on_progress)
	})
	.await
	.unwrap();

	let db_write_start = Instant::now();
	let scan_read_time = scan_start.elapsed();

	for (i, chunk) in paths.chunks(BATCH_SIZE).enumerate() {
		on_progress(vec![
			ScanProgress::SavedChunks(i as usize),
			ScanProgress::Message(format!(
				"Writing {} of {} to db",
				i * chunk.len(),
				paths.len(),
			)),
		]);

		// vector to store active models
		let mut files: Vec<PrismaValue> = Vec::new();

		for (file_path, file_id, parent_dir_id, is_dir) in chunk {
			files.extend(
				match prepare_values(&file_path, *file_id, &location, parent_dir_id, *is_dir) {
					Ok(values) => values.to_vec(),
					Err(e) => {
						println!("Error creating file model from path {:?}: {}", file_path, e);
						continue;
					}
				},
			);
		}

		println!("Creating {} file paths. {:?}", files.len(), files);

		let raw = Raw::new(
			&format!("
		      		INSERT INTO file_paths (id, is_dir, location_id, materialized_path, name, extension, parent_id, date_created) 
		      		VALUES {}
		        ", 
		        vec!["({}, {}, {}, {}, {}, {}, {}, {})"; chunk.len()].join(", ")
			),
			files
		);

		let count = db._execute_raw(raw).await;

		println!("Inserted {:?} records", count);
	}
	println!(
		"scan of {:?} completed in {:?}. {:?} files found. db write completed in {:?}",
		&path,
		scan_read_time,
		paths.len(),
		db_write_start.elapsed()
	);
	Ok(())
}

// reads a file at a path and creates an ActiveModel with metadata
fn prepare_values(
	file_path: &PathBuf,
	id: i32,
	location: &LocationResource,
	parent_id: &Option<i32>,
	is_dir: bool,
) -> Result<[PrismaValue; 8], std::io::Error> {
	let metadata = fs::metadata(&file_path)?;
	let location_path = Path::new(location.path.as_ref().unwrap().as_str());
	// let size = metadata.len();
	let name;
	let extension;
	let date_created: DateTime<Utc> = metadata.created().unwrap().into();

	// if the 'file_path' is not a directory, then get the extension and name.

	// if 'file_path' is a directory, set extension to an empty string to avoid periods in folder names
	// - being interpreted as file extensions
	if is_dir {
		extension = "".to_string();
		name = extract_name(file_path.file_name());
	} else {
		extension = extract_name(file_path.extension());
		name = extract_name(file_path.file_stem());
	}

	let materialized_path = file_path.strip_prefix(location_path).unwrap();
	let materialized_path_as_string = materialized_path.to_str().unwrap_or("").to_owned();

	let values = [
		PrismaValue::Int(id as i64),
		PrismaValue::Boolean(metadata.is_dir()),
		PrismaValue::Int(location.id as i64),
		PrismaValue::String(materialized_path_as_string),
		PrismaValue::String(name),
		PrismaValue::String(extension.to_lowercase()),
		parent_id
			.clone()
			.map(|id| PrismaValue::Int(id as i64))
			.unwrap_or(PrismaValue::Null),
		PrismaValue::DateTime(date_created.into()),
	];

	Ok(values)
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

fn is_library(entry: &DirEntry) -> bool {
	entry
		.path()
		.to_str()
		// make better this is shit
		.map(|s| s.contains("/Library/"))
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
