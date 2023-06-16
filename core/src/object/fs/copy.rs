use crate::{
	extract_job_data, invalidate_query,
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::IsolatedFilePathData,
	prisma::{file_path, location},
	util::{
		db::{maybe_missing, MissingFieldError},
		error::FileIOError,
	},
};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs, io};
use tracing::{trace, warn};

use super::{
	construct_target_filename, error::FileSystemJobsError, fetch_source_and_target_location_paths,
	get_file_data_from_isolated_file_path, get_many_files_datas, FileData,
};

pub struct FileCopierJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobState {
	sources_location_path: PathBuf,
}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileCopierJobInit {
	pub source_location_id: location::id::Type,
	pub target_location_id: location::id::Type,
	pub sources_file_path_ids: Vec<file_path::id::Type>,
	pub target_location_relative_directory_path: PathBuf,
	pub target_file_name_suffix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobStep {
	pub source_file_data: FileData,
	pub target_full_path: PathBuf,
}

impl JobInitData for FileCopierJobInit {
	type Job = FileCopierJob;
}

#[async_trait::async_trait]
impl StatefulJob for FileCopierJob {
	type Init = FileCopierJobInit;
	type Data = FileCopierJobState;
	type Step = FileCopierJobStep;

	const NAME: &'static str = "file_copier";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		let (sources_location_path, targets_location_path) =
			fetch_source_and_target_location_paths(
				db,
				state.init.source_location_id,
				state.init.target_location_id,
			)
			.await?;

		state.steps = get_many_files_datas(
			db,
			&sources_location_path,
			&state.init.sources_file_path_ids,
		)
		.await?
		.into_iter()
		.flat_map(|file_data| {
			// add the currently viewed subdirectory to the location root
			let mut full_target_path =
				targets_location_path.join(&state.init.target_location_relative_directory_path);

			full_target_path.push(construct_target_filename(
				&file_data,
				&state.init.target_file_name_suffix,
			)?);

			Ok::<_, MissingFieldError>(FileCopierJobStep {
				source_file_data: file_data,
				target_full_path: full_target_path,
			})
		})
		.collect();

		state.data = Some(FileCopierJobState {
			sources_location_path,
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let FileCopierJobStep {
			source_file_data,
			target_full_path,
		} = &state.steps[0];

		let data = extract_job_data!(state);

		if maybe_missing(source_file_data.file_path.is_dir, "file_path.is_dir")? {
			fs::create_dir_all(target_full_path)
				.await
				.map_err(|e| FileIOError::from((target_full_path, e)))?;

			let mut read_dir = fs::read_dir(&source_file_data.full_path)
				.await
				.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))?;

			// Can't use the `steps` borrow from here ownwards, or you feel the wrath of the borrow checker
			while let Some(children_entry) = read_dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&state.steps[0].source_file_data.full_path, e)))?
			{
				let children_path = children_entry.path();
				let target_children_full_path = state.steps[0].target_full_path.join(
					children_path
						.strip_prefix(&state.steps[0].source_file_data.full_path)
						.map_err(|_| JobError::Path)?,
				);

				// Currently not supporting file_name suffixes children files in a directory being copied
				state.steps.push_back(FileCopierJobStep {
					target_full_path: target_children_full_path,
					source_file_data: get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&data.sources_location_path,
						&IsolatedFilePathData::new(
							state.init.source_location_id,
							&data.sources_location_path,
							&children_path,
							children_entry
								.metadata()
								.await
								.map_err(|e| FileIOError::from((&children_path, e)))?
								.is_dir(),
						)
						.map_err(FileSystemJobsError::from)?,
					)
					.await?,
				});

				ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);
			}
		} else {
			if source_file_data.full_path.parent().ok_or(JobError::Path)?
				== target_full_path.parent().ok_or(JobError::Path)?
			{
				return Err(FileSystemJobsError::MatchingSrcDest(
					source_file_data.full_path.clone().into_boxed_path(),
				)
				.into());
			}

			match fs::metadata(target_full_path).await {
				Ok(_) => {
					// only skip as it could be half way through a huge directory copy and run into an issue
					warn!(
						"Skipping {} as it would be overwritten",
						target_full_path.display()
					);
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					trace!(
						"Copying from {} to {}",
						source_file_data.full_path.display(),
						target_full_path.display()
					);

					fs::copy(&source_file_data.full_path, &target_full_path)
						.await
						.map_err(|e| FileIOError::from((target_full_path, e)))?;
				}
				Err(e) => return Err(FileIOError::from((target_full_path, e)).into()),
			}
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&mut self, ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
