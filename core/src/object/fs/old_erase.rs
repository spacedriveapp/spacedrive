use crate::{
	invalidate_query,
	library::Library,
	location::get_location_path_from_location_id,
	old_job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunMetadata, JobStepOutput,
		StatefulJob, WorkerContext,
	},
};

use sd_core_file_path_helper::IsolatedFilePathData;

use sd_prisma::prisma::{file_path, location};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{hash::Hash, path::PathBuf};

use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use tokio::{
	fs::{self, OpenOptions},
	io::AsyncWriteExt,
};
use tracing::trace;

use super::{
	error::FileSystemJobsError, get_file_data_from_isolated_file_path, get_many_files_datas,
	FileData,
};

#[serde_as]
#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct OldFileEraserJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub passes: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldFileEraserJobData {
	location_path: PathBuf,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct FileEraserJobRunMetadata {
	directories_to_remove: Vec<PathBuf>,
}

impl JobRunMetadata for FileEraserJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.directories_to_remove
			.extend(new_data.directories_to_remove);
	}
}

#[async_trait::async_trait]
impl StatefulJob for OldFileEraserJobInit {
	type Data = OldFileEraserJobData;
	type Step = FileData;
	type RunMetadata = FileEraserJobRunMetadata;

	const NAME: &'static str = "file_eraser";

	fn target_location(&self) -> location::id::Type {
		self.location_id
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		let location_path = get_location_path_from_location_id(db, init.location_id)
			.await
			.map_err(FileSystemJobsError::from)?;

		let steps = get_many_files_datas(db, &location_path, &init.file_path_ids).await?;

		*data = Some(OldFileEraserJobData { location_path });

		Ok((Default::default(), steps).into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, .. }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurrences/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

		let mut new_metadata = Self::RunMetadata::default();

		if maybe_missing(step.file_path.is_dir, "file_path.is_dir")? {
			let mut more_steps = Vec::new();

			let mut dir = tokio::fs::read_dir(&step.full_path)
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;

			while let Some(children_entry) = dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?
			{
				let children_path = children_entry.path();

				more_steps.push(
					get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&data.location_path,
						&IsolatedFilePathData::new(
							init.location_id,
							&data.location_path,
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
				);
			}
			new_metadata
				.directories_to_remove
				.push(step.full_path.clone());

			Ok((more_steps, new_metadata).into())
		} else {
			{
				let mut file = OpenOptions::new()
					.read(true)
					.write(true)
					.open(&step.full_path)
					.await
					.map_err(|e| FileIOError::from((&step.full_path, e)))?;
				// let file_len = file
				// 	.metadata()
				// 	.await
				// 	.map_err(|e| FileIOError::from((&step.full_path, e)))?
				// 	.len();

				trace!(
					path = %step.full_path.display(),
					passes = init.passes,
					"Overwriting file;",
				);

				// TODO: File is only being truncated and not actually erased,
				// we should provide a way for securely overwriting the file with random data
				file.set_len(0)
					.await
					.map_err(|e| FileIOError::from((&step.full_path, e)))?;
				file.flush()
					.await
					.map_err(|e| FileIOError::from((&step.full_path, e)))?;
			}

			fs::remove_file(&step.full_path)
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;

			Ok(None.into())
		}
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		_data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		try_join_all(
			run_metadata
				.directories_to_remove
				.iter()
				.cloned()
				.map(|data| async {
					fs::remove_dir_all(&data)
						.await
						.map_err(|e| FileIOError::from((data, e)))
				}),
		)
		.await?;

		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(serde_json::to_value(init)?))
	}
}
