use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::LibraryContext,
	prisma::{file, file_path, location},
};

use chrono::{DateTime, FixedOffset};
use prisma_client_rust::{prisma_models::PrismaValue, raw, raw::Raw, Direction};
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
};
use tokio::{fs, io};
use tracing::{error, info};

use super::checksum::generate_cas_id;

// we break this job into chunks of 100 to improve performance
static CHUNK_SIZE: usize = 100;
pub const IDENTIFIER_JOB_NAME: &str = "file_identifier";

pub struct FileIdentifierJob {}

// FileIdentifierJobInit takes file_paths without a file_id and uniquely identifies them
// first: generating the cas_id and extracting metadata
// finally: creating unique file records, and linking them to their file_paths
#[derive(Serialize, Deserialize, Clone)]
pub struct FileIdentifierJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>, // subpath to start from
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FilePathIdAndLocationIdCursor {
	file_path_id: i32,
	location_id: i32,
}

impl From<&FilePathIdAndLocationIdCursor> for file_path::UniqueWhereParam {
	fn from(cursor: &FilePathIdAndLocationIdCursor) -> Self {
		file_path::location_id_id(cursor.location_id, cursor.file_path_id)
	}
}

#[derive(Serialize, Deserialize)]
pub struct FileIdentifierJobState {
	total_count: usize,
	task_count: usize,
	location: location::Data,
	location_path: PathBuf,
	cursor: FilePathIdAndLocationIdCursor,
}

#[async_trait::async_trait]
impl StatefulJob for FileIdentifierJob {
	type Init = FileIdentifierJobInit;
	type Data = FileIdentifierJobState;
	type Step = ();

	fn name(&self) -> &'static str {
		IDENTIFIER_JOB_NAME
	}

	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		info!("Identifying orphan file paths...");

		let library = ctx.library_ctx();

		let location_id = state.init.location.id;

		let location = library
			.db
			.location()
			.find_unique(location::id::equals(location_id))
			.exec()
			.await?
			.unwrap();

		let location_path = location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.unwrap_or_default();

		let total_count = library
			.db
			.file_path()
			.count(orphan_path_filters(location_id))
			.exec()
			.await? as usize;

		info!("Found {} orphan file paths", total_count);

		let task_count = (total_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!("Will process {} tasks", task_count);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let first_path_id = library
			.db
			.file_path()
			.find_first(orphan_path_filters(location_id))
			.exec()
			.await?
			.map(|d| d.id)
			.unwrap_or(1);

		state.data = Some(FileIdentifierJobState {
			total_count,
			task_count,
			location,
			location_path,
			cursor: FilePathIdAndLocationIdCursor {
				file_path_id: first_path_id,
				location_id: state.init.location.id,
			},
		});

		state.steps = (0..task_count).map(|_| ()).collect();

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		// link file_path ids to a CreateFile struct containing unique file data
		let mut chunk: HashMap<i32, CreateFile> = HashMap::new();
		let mut cas_lookup: HashMap<String, i32> = HashMap::new();

		let data = state
			.data
			.as_mut()
			.expect("critical error: missing data on job state");

		// get chunk of orphans to process
		let file_paths =
			match get_orphan_file_paths(&ctx.library_ctx(), &data.cursor, data.location.id).await {
				Ok(file_paths) => file_paths,
				Err(e) => {
					info!("Error getting orphan file paths: {:#?}", e);
					return Ok(());
				}
			};
		info!(
			"Processing {:?} orphan files. ({} completed of {})",
			file_paths.len(),
			state.step_number,
			data.task_count
		);

		// analyze each file_path
		for file_path in &file_paths {
			// get the cas_id and extract metadata
			match assemble_object_metadata(&data.location_path, file_path).await {
				Ok(file) => {
					let cas_id = file.cas_id.clone();
					// create entry into chunks for created file data
					chunk.insert(file_path.id, file);
					cas_lookup.insert(cas_id, file_path.id);
				}
				Err(e) => {
					info!("Error assembling Object metadata: {:#?}", e);
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

		info!("Found {} existing Objects", existing_files.len());

		// connect Paths that match existing Objects in the database
		for existing_file in &existing_files {
			if let Err(e) = ctx
				.library_ctx()
				.db
				.file_path()
				.update(
					file_path::location_id_id(
						state.init.location.id,
						*cas_lookup.get(&existing_file.cas_id).unwrap(),
					),
					vec![file_path::file_id::set(Some(existing_file.id))],
				)
				.exec()
				.await
			{
				info!("Error updating file_id: {:#?}", e);
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

		if new_files.is_empty() {
			error!("This shouldn't happen?");
			return Ok(());
		}

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
			.exec()
			.await
			.unwrap_or_else(|e| {
				error!("Error inserting files: {:#?}", e);
				Vec::new()
			});

		for created_file in created_files {
			// associate newly created files with their respective file_paths
			// TODO: this is potentially bottle necking the chunk system, individually linking file_path to file, 100 queries per chunk
			// - insert many could work, but I couldn't find a good way to do this in a single SQL query
			if let Err(e) = ctx
				.library_ctx()
				.db
				.file_path()
				.update(
					file_path::location_id_id(
						state.init.location.id,
						*cas_lookup.get(&created_file.cas_id).unwrap(),
					),
					vec![file_path::file_id::set(Some(created_file.id))],
				)
				.exec()
				.await
			{
				info!("Error updating file_id: {:#?}", e);
			}
		}

		// set the step data cursor to the last row of this chunk
		if let Some(last_row) = file_paths.last() {
			data.cursor.file_path_id = last_row.id;
		} else {
			return Ok(());
		}
		// }

		ctx.progress(vec![
			JobReportUpdate::CompletedTaskCount(state.step_number),
			JobReportUpdate::Message(format!(
				"Processed {} of {} orphan files",
				state.step_number * CHUNK_SIZE,
				data.total_count
			)),
		]);

		// let _remaining = count_orphan_file_paths(&ctx.core_ctx, location.id.into()).await?;
		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"Finalizing identifier job at {}, total of {} tasks",
			data.location_path.display(),
			data.task_count
		);

		Ok(())
	}
}

fn orphan_path_filters(location_id: i32) -> Vec<file_path::WhereParam> {
	vec![
		file_path::file_id::equals(None),
		file_path::is_dir::equals(false),
		file_path::location_id::equals(location_id),
	]
}

#[derive(Deserialize, Serialize, Debug)]
struct CountRes {
	count: Option<usize>,
}

async fn get_orphan_file_paths(
	ctx: &LibraryContext,
	cursor: &FilePathIdAndLocationIdCursor,
	location_id: i32,
) -> Result<Vec<file_path::Data>, prisma_client_rust::QueryError> {
	info!(
		"discovering {} orphan file paths at cursor: {:?}",
		CHUNK_SIZE, cursor
	);
	ctx.db
		.file_path()
		.find_many(orphan_path_filters(location_id))
		.order_by(file_path::id::order(Direction::Asc))
		.cursor(cursor.into())
		.take(CHUNK_SIZE as i64)
		.skip(1)
		.exec()
		.await
}

#[derive(Deserialize, Serialize, Debug)]
struct CreateFile {
	pub cas_id: String,
	pub size_in_bytes: i64,
	pub date_created: DateTime<FixedOffset>,
}

#[derive(Deserialize, Serialize, Debug)]
struct FileCreated {
	pub id: i32,
	pub cas_id: String,
}

async fn assemble_object_metadata(
	location_path: impl AsRef<Path>,
	file_path: &file_path::Data,
) -> Result<CreateFile, io::Error> {
	let path = location_path
		.as_ref()
		.join(file_path.materialized_path.as_str());

	info!("Reading path: {:?}", path);

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
