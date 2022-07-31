use crate::{
	job::{JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::location,
	sys::create_location,
};
use chrono::{DateTime, Utc};
use prisma_client_rust::{raw, raw::Raw, PrismaValue};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	ffi::OsStr,
	path::{Path, PathBuf},
	time::Duration,
};
use tokio::{fs, time::Instant};
use tracing::{error, info};
use walkdir::{DirEntry, WalkDir};

static BATCH_SIZE: usize = 100;
pub const INDEXER_JOB_NAME: &str = "indexer";

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	Message(String),
}

pub struct IndexerJob {}

#[derive(Serialize, Deserialize, Clone)]
pub struct IndexerJobInit {
	pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct IndexerJobData {
	location: location::Data,
	db_write_start: DateTime<Utc>,
	scan_read_time: Duration,
	total_paths: usize,
}

pub(crate) type IndexerJobStep = Vec<(PathBuf, i32, Option<i32>, bool)>;

impl IndexerJobData {
	fn on_scan_progress(ctx: WorkerContext, progress: Vec<ScanProgress>) {
		ctx.progress(
			progress
				.iter()
				.map(|p| match p.clone() {
					ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
					ScanProgress::SavedChunks(p) => JobReportUpdate::CompletedTaskCount(p),
					ScanProgress::Message(m) => JobReportUpdate::Message(m),
				})
				.collect(),
		)
	}
}

#[async_trait::async_trait]
impl StatefulJob for IndexerJob {
	type Init = IndexerJobInit;
	type Data = IndexerJobData;
	type Step = IndexerJobStep;

	fn name(&self) -> &'static str {
		INDEXER_JOB_NAME
	}

	// creates a vector of valid path buffers from a directory
	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let location = create_location(&ctx.library_ctx(), &state.init.path).await?;

		// query db to highers id, so we can increment it for the new files indexed
		#[derive(Deserialize, Serialize, Debug)]
		struct QueryRes {
			id: Option<i32>,
		}
		// grab the next id so we can increment in memory for batch inserting
		let first_file_id = match ctx
			.library_ctx()
			.db
			._query_raw::<QueryRes>(raw!("SELECT MAX(id) id FROM file_paths"))
			.await
		{
			Ok(rows) => rows[0].id.unwrap_or(0),
			Err(e) => panic!("Error querying for next file id: {:#?}", e),
		};

		//check is path is a directory
		if !state.init.path.is_dir() {
			// return Err(anyhow::anyhow!("{} is not a directory", &path));
			panic!("{:#?} is not a directory", state.init.path);
		}

		// spawn a dedicated thread to scan the directory for performance
		let path = state.init.path.clone();
		let inner_ctx = ctx.clone();
		let (paths, scan_start) = tokio::task::spawn_blocking(move || {
			// store every valid path discovered
			let mut paths: Vec<(PathBuf, i32, Option<i32>, bool)> = Vec::new();
			// store a hashmap of directories to their file ids for fast lookup
			let mut dirs = HashMap::new();
			// begin timer for logging purposes
			let scan_start = Instant::now();

			let mut next_file_id = first_file_id;
			let mut get_id = || {
				next_file_id += 1;
				next_file_id
			};
			// walk through directory recursively
			for entry in WalkDir::new(&path).into_iter().filter_entry(|dir| {
				// check if entry is approved
				!is_hidden(dir) && !is_app_bundle(dir) && !is_node_modules(dir) && !is_library(dir)
			}) {
				// extract directory entry or log and continue if failed
				let entry = match entry {
					Ok(entry) => entry,
					Err(e) => {
						error!("Error reading file {}", e);
						continue;
					}
				};
				let path = entry.path();

				info!("Found filesystem path: {:?}", path);

				let parent_path = path
					.parent()
					.unwrap_or_else(|| Path::new(""))
					.to_str()
					.unwrap_or("");
				let parent_dir_id = dirs.get(&*parent_path);

				let path_str = match path.as_os_str().to_str() {
					Some(path_str) => path_str,
					None => {
						error!("Error reading file {}", &path.display());
						continue;
					}
				};

				IndexerJobData::on_scan_progress(
					inner_ctx.clone(),
					vec![
						ScanProgress::Message(format!("Scanning {}", path_str)),
						ScanProgress::ChunkCount(paths.len() / BATCH_SIZE),
					],
				);

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
			(paths, scan_start)
		})
		.await?;

		state.data = Some(IndexerJobData {
			location,
			db_write_start: Utc::now(),
			scan_read_time: scan_start.elapsed(),
			total_paths: paths.len(),
		});

		state.steps = paths
			.chunks(BATCH_SIZE)
			.enumerate()
			.map(|(i, chunk)| {
				IndexerJobData::on_scan_progress(
					ctx.clone(),
					vec![
						ScanProgress::SavedChunks(i as usize),
						ScanProgress::Message(format!(
							"Writing {} of {} to db",
							i * chunk.len(),
							paths.len(),
						)),
					],
				);
				chunk.to_vec()
			})
			.collect();

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		// vector to store active models
		let mut files = Vec::new();
		let step = &state.steps[0];

		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		for (file_path, file_id, parent_dir_id, is_dir) in step {
			files.extend(
				match prepare_values(file_path, *file_id, &data.location, parent_dir_id, *is_dir)
					.await
				{
					Ok(values) => values.to_vec(),
					Err(e) => {
						error!("Error creating file model from path {:?}: {}", file_path, e);
						continue;
					}
				},
			);
		}

		let raw = Raw::new(
				&format!("
		      		INSERT INTO file_paths (id, is_dir, location_id, materialized_path, name, extension, parent_id, date_created) 
		      		VALUES {}
		        ",
						 vec!["({}, {}, {}, {}, {}, {}, {}, {})"; step.len()].join(", ")
				),
				files
			);

		let count = ctx.library_ctx().db._execute_raw(raw).await;

		info!("Inserted {:?} records", count);

		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"scan of {:?} completed in {:?}. {:?} files found. db write completed in {:?}",
			state.init.path,
			data.scan_read_time,
			data.total_paths,
			Utc::now() - data.db_write_start,
		);

		Ok(())
	}
}

// // PathContext provides the indexer with instruction to handle particular directory structures and identify rich context.
// pub struct PathContext {
// 	// an app specific key "com.github.repo"
// 	pub key: String,
// 	pub name: String,
// 	pub is_dir: bool,
// 	// possible file extensions for this path
// 	pub extensions: Vec<String>,
// 	// sub-paths that must be found
// 	pub must_contain_sub_paths: Vec<String>,
// 	// sub-paths that are ignored
// 	pub always_ignored_sub_paths: Option<String>,
// }

// reads a file at a path and creates an ActiveModel with metadata
async fn prepare_values(
	file_path: impl AsRef<Path>,
	id: i32,
	location: &location::Data,
	parent_id: &Option<i32>,
	is_dir: bool,
) -> Result<[PrismaValue; 8], std::io::Error> {
	let file_path = file_path.as_ref();

	let metadata = fs::metadata(file_path).await?;
	let location_path = location.local_path.as_ref().map(PathBuf::from).unwrap();
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
		.map(|s| s.starts_with('.'))
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

	// let is_app_bundle = is_dir && contains_dot;
	// if is_app_bundle {
	//   let path_buff = entry.path();
	//   let path = path_buff.to_str().unwrap();

	//   self::path(&path, );
	// }

	is_dir && contains_dot
}
