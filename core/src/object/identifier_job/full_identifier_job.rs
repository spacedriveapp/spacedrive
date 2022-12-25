use crate::{
	invalidate_query,
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::LibraryContext,
	prisma::{file_path, location},
};

use std::path::PathBuf;

use prisma_client_rust::Direction;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{identifier_job_step, IdentifierJobError, CHUNK_SIZE};

pub const FULL_IDENTIFIER_JOB_NAME: &str = "file_identifier";

pub struct FullFileIdentifierJob {}

// FileIdentifierJobInit takes file_paths without a file_id and uniquely identifies them
// first: generating the cas_id and extracting metadata
// finally: creating unique file records, and linking them to their file_paths
#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct FullFileIdentifierJobInit {
	pub location_id: i32,
	pub sub_path: Option<PathBuf>, // subpath to start from
}

#[derive(Serialize, Deserialize, Debug)]
struct FilePathIdAndLocationIdCursor {
	file_path_id: i32,
	location_id: i32,
}

impl From<&FilePathIdAndLocationIdCursor> for file_path::UniqueWhereParam {
	fn from(cursor: &FilePathIdAndLocationIdCursor) -> Self {
		file_path::location_id_id(cursor.location_id, cursor.file_path_id)
	}
}

#[derive(Serialize, Deserialize)]
pub struct FullFileIdentifierJobState {
	location: location::Data,
	location_path: PathBuf,
	cursor: FilePathIdAndLocationIdCursor,
	report: FileIdentifierReport,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FileIdentifierReport {
	location_path: String,
	total_orphan_paths: usize,
	total_objects_created: usize,
	total_objects_linked: usize,
	total_objects_ignored: usize,
}

#[async_trait::async_trait]
impl StatefulJob for FullFileIdentifierJob {
	type Init = FullFileIdentifierJobInit;
	type Data = FullFileIdentifierJobState;
	type Step = ();

	fn name(&self) -> &'static str {
		FULL_IDENTIFIER_JOB_NAME
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		info!("Identifying orphan File Paths...");

		let location_id = state.init.location_id;

		let location = ctx
			.library_ctx
			.db
			.location()
			.find_unique(location::id::equals(location_id))
			.exec()
			.await?
			.ok_or(IdentifierJobError::MissingLocation(state.init.location_id))?;

		let location_path = location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.ok_or(IdentifierJobError::LocationLocalPath(location_id))?;

		let orphan_count = count_orphan_file_paths(&ctx.library_ctx, location_id).await?;
		info!("Found {} orphan file paths", orphan_count);

		let task_count = (orphan_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!(
			"Found {} orphan Paths. Will execute {} tasks...",
			orphan_count, task_count
		);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let first_path_id = ctx
			.library_ctx
			.db
			.file_path()
			.find_first(orphan_path_filters(location_id, None))
			.exec()
			.await?
			.map(|d| d.id)
			.unwrap_or(1);

		state.data = Some(FullFileIdentifierJobState {
			report: FileIdentifierReport {
				location_path: location_path.to_str().unwrap_or("").to_string(),
				total_orphan_paths: orphan_count,
				..Default::default()
			},
			location,
			location_path,
			cursor: FilePathIdAndLocationIdCursor {
				file_path_id: first_path_id,
				location_id: state.init.location_id,
			},
		});

		state.steps = (0..task_count).map(|_| ()).collect();

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let data = state
			.data
			.as_mut()
			.expect("Critical error: missing data on job state");

		// get chunk of orphans to process
		let file_paths =
			get_orphan_file_paths(&ctx.library_ctx, &data.cursor, data.location.id).await?;

		// if no file paths found, abort entire job early, there is nothing to do
		// if we hit this error, there is something wrong with the data/query
		if file_paths.is_empty() {
			return Err(JobError::EarlyFinish {
				name: self.name().to_string(),
				reason: "Expected orphan Paths not returned from database query for this chunk"
					.to_string(),
			});
		}

		info!(
			"Processing {:?} orphan Paths. ({} completed of {})",
			file_paths.len(),
			state.step_number,
			data.report.total_orphan_paths
		);

		let (total_objects_created, total_objects_linked) = identifier_job_step(
			&ctx.library_ctx,
			state.init.location_id,
			&data.location_path,
			&file_paths,
		)
		.await?;
		data.report.total_objects_created += total_objects_created;
		data.report.total_objects_linked += total_objects_linked;

		// set the step data cursor to the last row of this chunk
		if let Some(last_row) = file_paths.last() {
			data.cursor.file_path_id = last_row.id;
		}

		ctx.progress(vec![
			JobReportUpdate::CompletedTaskCount(state.step_number),
			JobReportUpdate::Message(format!(
				"Processed {} of {} orphan Paths",
				state.step_number * CHUNK_SIZE,
				data.report.total_orphan_paths
			)),
		]);

		invalidate_query!(ctx.library_ctx, "locations.getExplorerData");

		// let _remaining = count_orphan_file_paths(&ctx.core_ctx, location_id.into()).await?;
		Ok(())
	}

	async fn finalize(&self, _ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		info!("Finalizing identifier job: {:#?}", data.report);

		Ok(Some(serde_json::to_value(&data.report)?))
	}
}

fn orphan_path_filters(location_id: i32, file_path_id: Option<i32>) -> Vec<file_path::WhereParam> {
	let mut params = vec![
		file_path::object_id::equals(None),
		file_path::is_dir::equals(false),
		file_path::location_id::equals(location_id),
	];
	// this is a workaround for the cursor not working properly
	if let Some(file_path_id) = file_path_id {
		params.push(file_path::id::gte(file_path_id));
	}
	params
}

async fn count_orphan_file_paths(
	ctx: &LibraryContext,
	location_id: i32,
) -> Result<usize, prisma_client_rust::QueryError> {
	Ok(ctx
		.db
		.file_path()
		.count(vec![
			file_path::object_id::equals(None),
			file_path::is_dir::equals(false),
			file_path::location_id::equals(location_id),
		])
		.exec()
		.await? as usize)
}

async fn get_orphan_file_paths(
	ctx: &LibraryContext,
	cursor: &FilePathIdAndLocationIdCursor,
	location_id: i32,
) -> Result<Vec<file_path::Data>, prisma_client_rust::QueryError> {
	info!(
		"Querying {} orphan Paths at cursor: {:?}",
		CHUNK_SIZE, cursor
	);
	ctx.db
		.file_path()
		.find_many(orphan_path_filters(location_id, Some(cursor.file_path_id)))
		.order_by(file_path::id::order(Direction::Asc))
		// .cursor(cursor.into())
		.take(CHUNK_SIZE as i64)
		// .skip(1)
		.exec()
		.await
}
