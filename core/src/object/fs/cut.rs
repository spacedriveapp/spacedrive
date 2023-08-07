use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunErrors, JobStepOutput, StatefulJob,
		WorkerContext,
	},
	library::LoadedLibrary,
	location::file_path_helper::push_location_relative_path,
	object::fs::{construct_target_filename, error::FileSystemJobsError},
	prisma::{file_path, location},
	util::error::FileIOError,
};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::{fs, io};
use tracing::{trace, warn};

use super::{fetch_source_and_target_location_paths, get_many_files_datas, FileData};

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

#[async_trait::async_trait]
impl StatefulJob for FileCutterJobInit {
	type Data = FileCutterJobData;
	type Step = FileData;
	type RunMetadata = ();

	const NAME: &'static str = "file_cutter";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let LoadedLibrary { db, .. } = &*ctx.library;

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

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		_: &WorkerContext,
		CurrentStep {
			step: file_data, ..
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let full_output = data
			.full_target_directory_path
			.join(construct_target_filename(file_data, &None)?);

		if file_data.full_path == full_output {
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
		}
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		_data: &Option<Self::Data>,
		_run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(json!({ "init": init })))
	}
}
