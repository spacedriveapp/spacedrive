use crate::{
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_file_identifier, get_existing_file_path_id, MaterializedPath,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::{chain_optional_iter, uuid_to_bytes},
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use prisma_client_rust::Direction;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::{
	finalize_file_identifier, process_identifier_file_paths, FileIdentifierJobError,
	FileIdentifierReport, CHUNK_SIZE,
};

pub struct ShallowFileIdentifierJob {}

/// `ShallowFileIdentifierJobInit` takes file_paths without a file_id from a specific path
/// (just direct children of this path) and uniquely identifies them:
/// - first: generating the cas_id and extracting metadata
/// - finally: creating unique file records, and linking them to their file_paths
#[derive(Serialize, Deserialize, Clone)]
pub struct ShallowFileIdentifierJobInit {
	pub location: location::Data,
	pub sub_path: PathBuf,
}

impl Hash for ShallowFileIdentifierJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		self.sub_path.hash(state);
	}
}

#[derive(Serialize, Deserialize)]
pub struct ShallowFileIdentifierJobState {
	cursor: i32,
	report: FileIdentifierReport,
	sub_path_id: Uuid,
}

impl JobInitData for ShallowFileIdentifierJobInit {
	type Job = ShallowFileIdentifierJob;
}

#[async_trait::async_trait]
impl StatefulJob for ShallowFileIdentifierJob {
	type Init = ShallowFileIdentifierJobInit;
	type Data = ShallowFileIdentifierJobState;
	type Step = ();

	const NAME: &'static str = "shallow_file_identifier";
	const IS_BACKGROUND: bool = true;

	fn new() -> Self {
		Self {}
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		info!("Identifying orphan File Paths...");

		let location_id = state.init.location.id;
		let location_path = Path::new(&state.init.location.path);

		let sub_path_id = if state.init.sub_path != Path::new("") {
			let full_path = ensure_sub_path_is_in_location(location_path, &state.init.sub_path)
				.await
				.map_err(FileIdentifierJobError::from)?;
			ensure_sub_path_is_directory(location_path, &state.init.sub_path)
				.await
				.map_err(FileIdentifierJobError::from)?;

			get_existing_file_path_id(
				&MaterializedPath::new(location_id, location_path, &full_path, true)
					.map_err(FileIdentifierJobError::from)?,
				db,
			)
			.await
			.map_err(FileIdentifierJobError::from)?
			.expect("Sub path should already exist in the database")
		} else {
			get_existing_file_path_id(
				&MaterializedPath::new(location_id, location_path, location_path, true)
					.map_err(FileIdentifierJobError::from)?,
				db,
			)
			.await
			.map_err(FileIdentifierJobError::from)?
			.expect("Location root path should already exist in the database")
		};

		let orphan_count = count_orphan_file_paths(db, location_id, sub_path_id).await?;

		// Initializing `state.data` here because we need a complete state in case of early finish
		state.data = Some(ShallowFileIdentifierJobState {
			report: FileIdentifierReport {
				location_path: location_path.to_path_buf(),
				total_orphan_paths: orphan_count,
				..Default::default()
			},
			cursor: 0,
			sub_path_id,
		});

		if orphan_count == 0 {
			return Err(JobError::EarlyFinish {
				name: <Self as StatefulJob>::NAME.to_string(),
				reason: "Found no orphan file paths to process".to_string(),
			});
		}

		info!("Found {} orphan file paths", orphan_count);

		let task_count = (orphan_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!(
			"Found {} orphan Paths. Will execute {} tasks...",
			orphan_count, task_count
		);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let first_path = db
			.file_path()
			.find_first(orphan_path_filters(location_id, None, sub_path_id))
			// .order_by(file_path::id::order(Direction::Asc))
			.select(file_path::select!({ id }))
			.exec()
			.await?
			.unwrap(); // SAFETY: We already validated before that there are orphans `file_path`s

		// SAFETY: We just initialized `state.data` above
		state.data.as_mut().unwrap().cursor = first_path.id;

		state.steps = (0..task_count).map(|_| ()).collect();

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let ShallowFileIdentifierJobState {
			ref mut cursor,
			ref mut report,
			ref sub_path_id,
		} = state
			.data
			.as_mut()
			.expect("Critical error: missing data on job state");

		let location = &state.init.location;

		// get chunk of orphans to process
		let file_paths = get_orphan_file_paths(
			&ctx.library.db,
			state.init.location.id,
			*cursor,
			*sub_path_id,
		)
		.await?;

		process_identifier_file_paths(
			<Self as StatefulJob>::NAME,
			location,
			&file_paths,
			state.step_number,
			cursor,
			report,
			ctx,
		)
		.await
	}

	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_file_identifier(
			&state
				.data
				.as_ref()
				.expect("critical error: missing data on job state")
				.report,
			ctx,
		)
	}
}

fn orphan_path_filters(
	location_id: i32,
	file_path_id: Option<i32>,
	sub_path_id: Uuid,
) -> Vec<file_path::WhereParam> {
	chain_optional_iter(
		[
			file_path::object_id::equals(None),
			file_path::is_dir::equals(false),
			file_path::location_id::equals(location_id),
			file_path::parent_id::equals(Some(uuid_to_bytes(sub_path_id))),
		],
		[file_path_id.map(file_path::id::gte)],
	)
}

async fn count_orphan_file_paths(
	db: &PrismaClient,
	location_id: i32,
	sub_path_id: Uuid,
) -> Result<usize, prisma_client_rust::QueryError> {
	db.file_path()
		.count(orphan_path_filters(location_id, None, sub_path_id))
		.exec()
		.await
		.map(|c| c as usize)
}

async fn get_orphan_file_paths(
	db: &PrismaClient,
	location_id: i32,
	cursor: i32,
	sub_path_id: Uuid,
) -> Result<Vec<file_path_for_file_identifier::Data>, prisma_client_rust::QueryError> {
	info!(
		"Querying {} orphan Paths at cursor: {:?}",
		CHUNK_SIZE, cursor
	);
	db.file_path()
		.find_many(orphan_path_filters(location_id, Some(cursor), sub_path_id))
		.order_by(file_path::id::order(Direction::Asc))
		// .cursor(cursor.into())
		.take(CHUNK_SIZE as i64)
		// .skip(1)
		.select(file_path_for_file_identifier::select())
		.exec()
		.await
}
