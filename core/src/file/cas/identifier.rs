use super::checksum::generate_cas_id;
use crate::{
	file::FileError,
	job::JobReportUpdate,
	job::{Job, JobResult, WorkerContext},
	library::LibraryContext,
	prisma::{file, file_path},
	sys::get_location,
};
use chrono::{DateTime, FixedOffset};
use futures::future::join_all;
use log::{error, info};
use prisma_client_rust::{prisma_models::PrismaValue, raw, raw::Raw, Direction};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::{fs, io};

// FileIdentifierJob takes file_paths without a file_id and uniquely identifies them
// first: generating the cas_id and extracting metadata
// finally: creating unique file records, and linking them to their file_paths
#[derive(Debug)]
pub struct FileIdentifierJob {
	pub location_id: i32,
	pub path: PathBuf,
}

// we break this job into chunks of 100 to improve performance
static CHUNK_SIZE: usize = 100;

#[async_trait::async_trait]
impl Job for FileIdentifierJob {
	fn name(&self) -> &'static str {
		"file_identifier"
	}

	async fn run(&self, ctx: WorkerContext) -> JobResult {
		info!("Identifying orphan file paths...");

		let location = get_location(&ctx.library_ctx(), self.location_id).await?;
		let location_path = location.path.unwrap_or_else(|| "".to_string());

		let total_count = count_orphan_file_paths(&ctx.library_ctx(), location.id.into()).await?;
		info!("Found {} orphan file paths", total_count);

		let task_count = (total_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!("Will process {} tasks", task_count);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let mut completed: usize = 0;
		let mut cursor: i32 = 1;
		// loop until task count is complete
		while completed < task_count {
			// link file_path ids to a CreateFile struct containing unique file data
			let mut chunk: HashMap<i32, CreateFile> = HashMap::new();
			let mut cas_lookup: HashMap<String, i32> = HashMap::new();

			// get chunk of orphans to process
			let file_paths = match get_orphan_file_paths(&ctx.library_ctx(), cursor).await {
				Ok(file_paths) => file_paths,
				Err(e) => {
					info!("Error getting orphan file paths: {:#?}", e);
					continue;
				}
			};
			info!(
				"Processing {:?} orphan files. ({} completed of {})",
				file_paths.len(),
				completed,
				task_count
			);

			// analyze each file_path
			for file_path in &file_paths {
				// get the cas_id and extract metadata
				match prepare_file(&location_path, file_path).await {
					Ok(file) => {
						let cas_id = file.cas_id.clone();
						// create entry into chunks for created file data
						chunk.insert(file_path.id, file);
						cas_lookup.insert(cas_id, file_path.id);
					}
					Err(e) => {
						info!("Error processing file: {:#?}", e);
						continue;
					}
				};
			}

			// find all existing files by cas id
			let generated_cas_ids = chunk.values().map(|c| c.cas_id.clone()).collect();
			let existing_files = ctx
				.library_ctx()
				.db
				.file()
				.find_many(vec![file::cas_id::in_vec(generated_cas_ids)])
				.exec()
				.await?;

			info!("Found {} existing files", existing_files.len());

			// link those existing files to their file paths
			// Had to put the file_path in a variable outside of the closure, to satisfy the borrow checker
			let library_ctx = ctx.library_ctx();
			let prisma_file_path = library_ctx.db.file_path();
			for result in join_all(existing_files.iter().map(|file| {
				prisma_file_path
					.find_unique(file_path::id::equals(
						*cas_lookup.get(&file.cas_id).unwrap(),
					))
					.update(vec![file_path::file_id::set(Some(file.id))])
					.exec()
			}))
			.await
			{
				if let Err(e) = result {
					error!("Error linking file: {:#?}", e);
				}
			}

			let existing_files_cas_ids = existing_files
				.iter()
				.map(|file| file.cas_id.clone())
				.collect::<HashSet<_>>();

			// extract files that don't already exist in the database
			let new_files = chunk
				.iter()
				.map(|(_id, create_file)| create_file)
				.filter(|create_file| !existing_files_cas_ids.contains(&create_file.cas_id))
				.collect::<Vec<_>>();

			// assemble prisma values for new unique files
			let mut values = Vec::with_capacity(new_files.len() * 3);
			for file in &new_files {
				values.extend([
					PrismaValue::String(file.cas_id.clone()),
					PrismaValue::Int(file.size_in_bytes),
					PrismaValue::DateTime(file.date_created),
				]);
			}

			// create new file records with assembled values
			let created_files: Vec<FileCreated> = ctx
				.library_ctx()
				.db
				._query_raw(Raw::new(
					&format!(
						"INSERT INTO files (cas_id, size_in_bytes, date_created) VALUES {}
						ON CONFLICT (cas_id) DO NOTHING RETURNING id, cas_id",
						vec!["({}, {}, {})"; new_files.len()].join(",")
					),
					values,
				))
				.await
				.unwrap_or_else(|e| {
					error!("Error inserting files: {:#?}", e);
					Vec::new()
				});

			// This code is duplicates, is this right?
			for result in join_all(created_files.iter().map(|file| {
				// associate newly created files with their respective file_paths
				// TODO: this is potentially bottle necking the chunk system, individually linking file_path to file, 100 queries per chunk
				// - insert many could work, but I couldn't find a good way to do this in a single SQL query
				prisma_file_path
					.find_unique(file_path::id::equals(
						*cas_lookup.get(&file.cas_id).unwrap(),
					))
					.update(vec![file_path::file_id::set(Some(file.id))])
					.exec()
			}))
			.await
			{
				if let Err(e) = result {
					error!("Error linking file: {:#?}", e);
				}
			}

			// handle loop end
			let last_row = match file_paths.last() {
				Some(l) => l,
				None => {
					break;
				}
			};
			cursor = last_row.id;
			completed += 1;

			ctx.progress(vec![
				JobReportUpdate::CompletedTaskCount(completed),
				JobReportUpdate::Message(format!(
					"Processed {} of {} orphan files",
					completed * CHUNK_SIZE,
					total_count
				)),
			]);
		}

		// let _remaining = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;
		Ok(())
	}
}

#[derive(Deserialize, Serialize, Debug)]
struct CountRes {
	count: Option<usize>,
}

pub async fn count_orphan_file_paths(
	ctx: &LibraryContext,
	location_id: i64,
) -> Result<usize, FileError> {
	let files_count = ctx.db
		._query_raw::<CountRes>(raw!(
			"SELECT COUNT(*) AS count FROM file_paths WHERE file_id IS NULL AND is_dir IS FALSE AND location_id = {}",
			PrismaValue::Int(location_id)
		))
		.await?;
	Ok(files_count[0].count.unwrap_or(0))
}

pub async fn get_orphan_file_paths(
	ctx: &LibraryContext,
	cursor: i32,
) -> Result<Vec<file_path::Data>, FileError> {
	info!(
		"discovering {} orphan file paths at cursor: {:?}",
		CHUNK_SIZE, cursor
	);
	ctx.db
		.file_path()
		.find_many(vec![
			file_path::file_id::equals(None),
			file_path::is_dir::equals(false),
		])
		.order_by(file_path::id::order(Direction::Asc))
		.cursor(file_path::id::cursor(cursor))
		.take(CHUNK_SIZE as i64)
		.exec()
		.await
		.map_err(|e| e.into())
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateFile {
	pub cas_id: String,
	pub size_in_bytes: i64,
	pub date_created: DateTime<FixedOffset>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FileCreated {
	pub id: i32,
	pub cas_id: String,
}

pub async fn prepare_file(
	location_path: impl AsRef<Path>,
	file_path: &file_path::Data,
) -> Result<CreateFile, io::Error> {
	let path = location_path
		.as_ref()
		.join(file_path.materialized_path.as_str());

	let metadata = fs::metadata(&path).await?;

	// let date_created: DateTime<Utc> = metadata.created().unwrap().into();

	let size = metadata.len();

	let cas_id = {
		if !file_path.is_dir {
			let mut ret = generate_cas_id(path, size).await?;
			ret.truncate(16);
			ret
		} else {
			"".to_string()
		}
	};

	Ok(CreateFile {
		cas_id,
		size_in_bytes: size as i64,
		date_created: file_path.date_created,
	})
}
