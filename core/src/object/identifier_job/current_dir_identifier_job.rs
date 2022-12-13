use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};
use std::path::PathBuf;

use crate::object::identifier_job::identifier_job_step;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::IdentifierJobError;

file_path::select!(file_path_id_only { id });

// we break this job into chunks of 100 to improve performance
static CHUNK_SIZE: usize = 100;
pub const CURRENT_DIR_IDENTIFIER_JOB_NAME: &str = "current_dir_file_identifier";

pub struct CurrentDirFileIdentifierJob {}

#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct CurrentDirFileIdentifierJobInit {
	pub location_id: i32,
	pub root_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct CurrentDirFileIdentifierJobState {
	total_count: usize,
	task_count: usize,
	location_path: PathBuf,
}

#[async_trait::async_trait]
impl StatefulJob for CurrentDirFileIdentifierJob {
	type Init = CurrentDirFileIdentifierJobInit;
	type Data = CurrentDirFileIdentifierJobState;
	type Step = Vec<file_path::Data>;

	fn name(&self) -> &'static str {
		CURRENT_DIR_IDENTIFIER_JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		info!(
			"Identifying orphan paths for directory \"{}\"",
			state.init.root_path.display()
		);

		let location = ctx
			.library_ctx
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.ok_or(IdentifierJobError::MissingLocation(state.init.location_id))?;

		let parent_directory_id = ctx
			.library_ctx
			.db
			.file_path()
			.find_first(vec![
				file_path::location_id::equals(state.init.location_id),
				file_path::materialized_path::equals(
					state
						.init
						.root_path
						.to_str()
						.expect("Found non-UTF-8 path")
						.to_string(),
				),
				file_path::is_dir::equals(true),
			])
			.select(file_path_id_only::select())
			.exec()
			.await?
			.ok_or_else(|| IdentifierJobError::MissingRootFilePath(state.init.root_path.clone()))?
			.id;

		let location_path = location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.ok_or(IdentifierJobError::LocationLocalPath(location.id))?;

		let orphan_paths = ctx
			.library_ctx
			.db
			.file_path()
			.find_many(orphan_path_filters(
				state.init.location_id,
				parent_directory_id,
			))
			.exec()
			.await?;

		// if no file paths found, abort entire job early
		if orphan_paths.is_empty() {
			return Err(JobError::EarlyFinish {
				name: self.name().to_string(),
				reason: format!(
					"No orphan paths for path \"{}\"",
					state.init.root_path.display()
				),
			});
		}

		let total_count = orphan_paths.len();
		let task_count = (total_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!(
			"Found {} orphan file paths on path \"{}\". Will execute {} tasks...",
			total_count,
			state.init.root_path.display(),
			task_count
		);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		state.data = Some(CurrentDirFileIdentifierJobState {
			total_count,
			task_count,
			location_path,
		});

		state.steps = orphan_paths
			.into_iter()
			.chunks(CHUNK_SIZE)
			.into_iter()
			.map(|chunk| chunk.collect::<Vec<_>>())
			.collect();

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let file_paths = &state.steps[0];
		let data = state
			.data
			.as_mut()
			.expect("Critical error: missing data on job state");

		info!(
			"Processing {:?} orphan Paths. ({} completed of {})",
			file_paths.len(),
			state.step_number,
			data.task_count
		);

		identifier_job_step(
			&ctx.library_ctx,
			state.init.location_id,
			&data.location_path,
			file_paths,
		)
		.await?;

		ctx.progress(vec![
			JobReportUpdate::CompletedTaskCount(state.step_number),
			JobReportUpdate::Message(format!(
				"Processed {} of {} orphan paths at \"{}\"",
				state.step_number * CHUNK_SIZE,
				data.total_count,
				data.location_path.display()
			)),
		]);

		Ok(())
	}

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"Finalizing current directory identifier job at {}, total of {} tasks",
			state.init.root_path.display(),
			data.task_count
		);

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}

fn orphan_path_filters(location_id: i32, parent_id: i32) -> Vec<file_path::WhereParam> {
	vec![
		file_path::object_id::equals(None),
		file_path::is_dir::equals(false),
		file_path::location_id::equals(location_id),
		file_path::parent_id::equals(Some(parent_id)),
	]
}
