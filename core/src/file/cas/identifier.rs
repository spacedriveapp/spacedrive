use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use crate::job::JobReportUpdate;
use crate::sys::get_location;
use crate::{
	file::FileError,
	job::{Job, WorkerContext},
	prisma::{file, file_path},
	CoreContext,
};
use chrono::{DateTime, FixedOffset, Utc};
use futures::executor::block_on;
use prisma_client_rust::prisma_models::PrismaValue;
use prisma_client_rust::raw::Raw;
use prisma_client_rust::{raw, Direction};
use serde::{Deserialize, Serialize};

use super::checksum::generate_cas_id;
#[derive(Deserialize, Serialize, Debug)]
pub struct FileCreated {
	pub id: i32,
	pub cas_id: String,
}

#[derive(Debug)]
pub struct FileIdentifierJob {
	pub location_id: i32,
	pub path: String,
}

// we break this job into chunks of 100 to improve performance
static CHUNK_SIZE: usize = 100;

#[async_trait::async_trait]
impl Job for FileIdentifierJob {
	fn name(&self) -> &'static str {
		"file_identifier"
	}
	async fn run(&self, ctx: WorkerContext) -> Result<(), Box<dyn std::error::Error>> {
		println!("Identifying files");
		let location = get_location(&ctx.core_ctx, self.location_id).await?;
		let location_path = location.path.unwrap_or("".to_string());

		let total_count = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;
		println!("Found {} orphan file paths", total_count);

		let task_count = (total_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		println!("Will process {} tasks", task_count);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let db = ctx.core_ctx.database.clone();

		let _ctx = tokio::task::spawn_blocking(move || {
			let mut completed: usize = 0;
			let mut cursor: i32 = 1;

			while completed < task_count {
				// link file_path ids to a CreateFile struct containing unique file data
				let mut chunk: HashMap<i32, CreateFile> = HashMap::new();
				let mut cas_lookup: HashMap<String, i32> = HashMap::new();

				// get chunk of orphans to process
				let file_paths = match block_on(get_orphan_file_paths(&ctx.core_ctx, cursor)) {
					Ok(file_paths) => file_paths,
					Err(e) => {
						println!("Error getting orphan file paths: {}", e);
						continue;
					}
				};
				println!(
					"Processing {:?} orphan files. ({} completed of {})",
					file_paths.len(),
					completed,
					task_count
				);

				// analyze each file_path
				for file_path in file_paths.iter() {
					// get the cas_id and extract metadata
					match prepare_file(&location_path, file_path) {
						Ok(file) => {
							let cas_id = file.cas_id.clone();
							// create entry into chunks for created file data
							chunk.insert(file_path.id, file);
							cas_lookup.insert(cas_id, file_path.id);
						}
						Err(e) => {
							println!("Error processing file: {}", e);
							continue;
						}
					};
				}

				// find all existing files by cas id
				let generated_cas_ids = chunk.values().map(|c| c.cas_id.clone()).collect();
				let existing_files: Vec<file::Data> = block_on(
					db.file()
						.find_many(vec![file::cas_id::in_vec(generated_cas_ids)])
						.exec(),
				)
				.unwrap();
				println!("Found {} existing files", existing_files.len());

				// TODO: link existing files to file_paths
				for file in existing_files.iter() {
					let file_path_id = cas_lookup.get(&file.cas_id).unwrap();
					block_on(
						db.file_path()
							.find_unique(file_path::id::equals(file_path_id.clone()))
							.update(vec![file_path::file_id::set(Some(file.id.clone()))])
							.exec(),
					)
					.unwrap();
				}

				// extract files that don't already exist in the database
				let new_files: Vec<&CreateFile> = chunk
					.iter()
					.map(|(_, c)| c)
					.filter(|c| !existing_files.iter().any(|d| d.cas_id == c.cas_id))
					.collect();

				// assemble prisma values
				let mut values: Vec<PrismaValue> = Vec::new();
				for file in new_files.iter() {
					values.extend([
						PrismaValue::String(file.cas_id.clone()),
						PrismaValue::Int(file.size_in_bytes.clone()),
						PrismaValue::DateTime(file.date_created.clone()),
					]);
				}

				// create new files
				let created_files: Vec<FileCreated> = block_on(db._query_raw(Raw::new(
					&format!(
						"INSERT INTO files (cas_id, size_in_bytes, date_created) VALUES {}
						ON CONFLICT (cas_id) DO NOTHING RETURNING id, cas_id",
						vec!["({}, {}, {})"; new_files.len()].join(",")
					),
					values,
				)))
				.unwrap_or_else(|e| {
					println!("Error inserting files: {}", e);
					Vec::new()
				});

				// associate newly created files with their respective file_paths
				for file in created_files.iter() {
					// TODO: This is bottle necking the chunk system, individually linking file_path to file, 100 queries per chunk.
					// Maybe an insert many could work? not sure.
					let file_path_id = cas_lookup.get(&file.cas_id).unwrap();
					block_on(
						db.file_path()
							.find_unique(file_path::id::equals(file_path_id.clone()))
							.update(vec![file_path::file_id::set(Some(file.id.clone()))])
							.exec(),
					)
					.unwrap();
				}

				// handle loop end
				let last_row = file_paths.last().unwrap();
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
			ctx
		})
		.await?;

		// let _remaining = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;
		Ok(())
	}
}

#[derive(Deserialize, Serialize, Debug)]
struct CountRes {
	count: Option<usize>,
}

pub async fn count_orphan_file_paths(
	ctx: &CoreContext,
	location_id: i64,
) -> Result<usize, FileError> {
	let db = &ctx.database;
	let files_count = db
		._query_raw::<CountRes>(raw!(
			"SELECT COUNT(*) AS count FROM file_paths WHERE file_id IS NULL AND is_dir IS FALSE AND location_id = {}",
			PrismaValue::Int(location_id)
		))
		.await?;
	Ok(files_count[0].count.unwrap_or(0))
}

pub async fn get_orphan_file_paths(
	ctx: &CoreContext,
	cursor: i32,
) -> Result<Vec<file_path::Data>, FileError> {
	let db = &ctx.database;
	println!("cursor: {:?}", cursor);
	let files = db
		.file_path()
		.find_many(vec![
			file_path::file_id::equals(None),
			file_path::is_dir::equals(false),
		])
		.order_by(file_path::id::order(Direction::Asc))
		.cursor(file_path::id::cursor(cursor))
		.take(CHUNK_SIZE as i64)
		.exec()
		.await?;
	Ok(files)
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateFile {
	pub cas_id: String,
	pub size_in_bytes: i64,
	pub date_created: DateTime<FixedOffset>,
}

pub fn prepare_file(
	location_path: &str,
	file_path: &file_path::Data,
) -> Result<CreateFile, io::Error> {
	let path = Path::new(&location_path).join(Path::new(file_path.materialized_path.as_str()));

	let metadata = fs::metadata(&path)?;

	// let date_created: DateTime<Utc> = metadata.created().unwrap().into();

	let size = metadata.len();

	let cas_id = {
		if !file_path.is_dir {
			let mut ret = generate_cas_id(path.clone(), size.clone()).unwrap();
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
