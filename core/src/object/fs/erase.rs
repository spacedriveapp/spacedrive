use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunMetadata, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::IsolatedFilePathData,
	prisma::{file_path, location},
	util::{db::maybe_missing, error::FileIOError},
};

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
	error::FileSystemJobsError, get_file_data_from_isolated_file_path,
	get_location_path_from_location_id, get_many_files_datas, FileData,
};

#[serde_as]
#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct FileEraserJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub passes: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEraserJobData {
	location_path: PathBuf,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct FileEraserJobRunMetadata {
	diretories_to_remove: Vec<PathBuf>,
}

impl JobRunMetadata for FileEraserJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.diretories_to_remove
			.extend(new_data.diretories_to_remove);
	}
}

#[async_trait::async_trait]
impl StatefulJob for FileEraserJobInit {
	type Data = FileEraserJobData;
	type Step = FileData;
	type RunMetadata = FileEraserJobRunMetadata;

	const NAME: &'static str = "file_eraser";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		let location_path = get_location_path_from_location_id(db, init.location_id).await?;

		let steps = get_many_files_datas(db, &location_path, &init.file_path_ids).await?;

		*data = Some(FileEraserJobData { location_path });

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
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

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
				.diretories_to_remove
				.push(step.full_path.clone());

			Ok((more_steps, new_metadata).into())
		} else {
			let mut file = OpenOptions::new()
				.read(true)
				.write(true)
				.open(&step.full_path)
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;
			let file_len = file
				.metadata()
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?
				.len();

			sd_crypto::fs::erase::erase(&mut file, file_len as usize, init.passes).await?;

			file.set_len(0)
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;
			file.flush()
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;
			drop(file);

			trace!("Erasing file: {}", step.full_path.display());

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
				.diretories_to_remove
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
