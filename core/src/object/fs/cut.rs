use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitData, JobInitOutput, JobReportUpdate, JobResult,
		JobRunErrors, JobState, JobStepOutput, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::push_location_relative_path,
	object::fs::{construct_target_filename, error::FileSystemJobsError},
	prisma::{file_path, location},
	util::error::FileIOError,
};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs, io};
use tracing::{trace, warn};

use super::{fetch_source_and_target_location_paths, get_many_files_datas, FileData};

pub struct FileCutterJob {}

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct FileCutterJobInit {
	pub source_location_id: location::id::Type,
	pub target_location_id: location::id::Type,
	pub sources_file_path_ids: Vec<file_path::id::Type>,
	pub target_location_relative_directory_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileCutterJobData {
	full_target_directory_path: PathBuf,
}

impl JobInitData for FileCutterJobInit {
	type Job = FileCutterJob;
}

#[async_trait::async_trait]
impl StatefulJob for FileCutterJob {
	type Init = FileCutterJobInit;
	type Data = FileCutterJobData;
	type Step = FileData;
	type RunMetadata = ();

	const NAME: &'static str = "file_cutter";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		init: &Self::Init,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let Library { db, .. } = &ctx.library;

		let (sources_location_path, targets_location_path) =
			fetch_source_and_target_location_paths(
				db,
				init.source_location_id,
				init.target_location_id,
			)
			.await?;

		let full_target_directory_path = push_location_relative_path(
			targets_location_path,
			&init.target_location_relative_directory_path,
		);

		*data = Some(FileCutterJobData {
			full_target_directory_path,
		});

		let steps =
			get_many_files_datas(db, &sources_location_path, &init.sources_file_path_ids).await?;

		ctx.progress(vec![JobReportUpdate::TaskCount(steps.len())]);

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		_: &Self::Init,
		CurrentStep {
			step: file_data,
			step_number,
			..
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let full_output = data
			.full_target_directory_path
			.join(construct_target_filename(file_data, &None)?);

		let res = if file_data.full_path == full_output {
			// File is already here, do nothing
			Ok(().into())
		} else {
			match fs::metadata(&full_output).await {
				Ok(_) => {
					warn!(
						"Skipping {} as it would be overwritten",
						full_output.display()
					);

					Ok(JobRunErrors(vec![FileSystemJobsError::WouldOverwrite(
						full_output.into_boxed_path(),
					)
					.to_string()])
					.into())
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					trace!(
						"Cutting {} to {}",
						file_data.full_path.display(),
						full_output.display()
					);

					fs::rename(&file_data.full_path, &full_output)
						.await
						.map_err(|e| FileIOError::from((&file_data.full_path, e)))?;

					Ok(().into())
				}

				Err(e) => return Err(FileIOError::from((&full_output, e)).into()),
			}
		};

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(step_number + 1)]);

		res
	}

	async fn finalize(&self, ctx: &WorkerContext, state: &JobState<Self>) -> JobResult {
		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
