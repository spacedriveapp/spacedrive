use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunErrors, JobStepOutput, StatefulJob,
		WorkerContext,
	},
	library::LoadedLibrary,
	location::file_path_helper::{join_location_relative_path, IsolatedFilePathData},
	prisma::{file_path, location},
	util::{
		db::{maybe_missing, MissingFieldError},
		error::FileIOError,
	},
};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::{fs, io};
use tracing::{trace, warn};

use super::{
	construct_target_filename, error::FileSystemJobsError, fetch_source_and_target_location_paths,
	get_file_data_from_isolated_file_path, get_many_files_datas, FileData,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobData {
	sources_location_path: PathBuf,
}

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct FileCopierJobInit {
	pub source_location_id: location::id::Type,
	pub target_location_id: location::id::Type,
	pub sources_file_path_ids: Vec<file_path::id::Type>,
	pub target_location_relative_directory_path: PathBuf,
	pub target_file_name_suffix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileCopierJobStep {
	pub source_file_data: FileData,
	pub target_full_path: PathBuf,
}

#[async_trait::async_trait]
impl StatefulJob for FileCopierJobInit {
	type Data = FileCopierJobData;
	type Step = FileCopierJobStep;
	type RunMetadata = ();

	const NAME: &'static str = "file_copier";

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

		let steps = get_many_files_datas(db, &sources_location_path, &init.sources_file_path_ids)
			.await?
			.into_iter()
			.flat_map(|file_data| {
				// add the currently viewed subdirectory to the location root
				let mut full_target_path = join_location_relative_path(
					&targets_location_path,
					&init.target_location_relative_directory_path,
				);

				full_target_path.push(construct_target_filename(
					&file_data,
					&init.target_file_name_suffix,
				)?);

				Ok::<_, MissingFieldError>(FileCopierJobStep {
					source_file_data: file_data,
					target_full_path: full_target_path,
				})
			})
			.collect::<Vec<_>>();

		*data = Some(FileCopierJobData {
			sources_location_path,
		});

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep {
			step: FileCopierJobStep {
				source_file_data,
				target_full_path,
			},
			..
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;

		if maybe_missing(source_file_data.file_path.is_dir, "file_path.is_dir")? {
			let mut more_steps = Vec::new();

			fs::create_dir_all(target_full_path)
				.await
				.map_err(|e| FileIOError::from((target_full_path, e)))?;

			let mut read_dir = fs::read_dir(&source_file_data.full_path)
				.await
				.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))?;

			while let Some(children_entry) = read_dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))?
			{
				let children_path = children_entry.path();
				let target_children_full_path = target_full_path.join(
					children_path
						.strip_prefix(&source_file_data.full_path)
						.expect("We got the children path from the read_dir, so it should be a child of the source path"),
				);

				// Currently not supporting file_name suffixes children files in a directory being copied
				more_steps.push(FileCopierJobStep {
					target_full_path: target_children_full_path,
					source_file_data: get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&data.sources_location_path,
						&IsolatedFilePathData::new(
							init.source_location_id,
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
			}

			Ok(more_steps.into())
		} else if &source_file_data.full_path == target_full_path {
			// File is already here, do nothing
			Ok(().into())
		} else {
			match fs::metadata(target_full_path).await {
				Ok(_) => {
					// only skip as it could be half way through a huge directory copy and run into an issue
					warn!(
						"Skipping {} as it would be overwritten",
						target_full_path.display()
					);

					Ok(JobRunErrors(vec![FileSystemJobsError::WouldOverwrite(
						target_full_path.clone().into_boxed_path(),
					)
					.to_string()])
					.into())
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					trace!(
						"Copying from {} to {}",
						source_file_data.full_path.display(),
						target_full_path.display()
					);

					fs::copy(&source_file_data.full_path, &target_full_path)
						.await
						// Using the ? here because we don't want to increase the completed task
						// count in case of file system errors
						.map_err(|e| FileIOError::from((target_full_path, e)))?;

					Ok(().into())
				}
				Err(e) => return Err(FileIOError::from((target_full_path, e)).into()),
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
