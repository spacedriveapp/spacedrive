use crate::{
	extract_job_data_mut, invalidate_query,
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
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

pub struct FileEraserJob {}

#[serde_as]
#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileEraserJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub passes: usize,
}

impl JobInitData for FileEraserJobInit {
	type Job = FileEraserJob;
}

#[derive(Serialize, Deserialize)]
pub struct FileEraserJobData {
	location_path: PathBuf,
	diretories_to_remove: Vec<PathBuf>,
}

#[async_trait::async_trait]
impl StatefulJob for FileEraserJob {
	type Init = FileEraserJobInit;
	type Data = FileEraserJobData;
	type Step = FileData;

	const NAME: &'static str = "file_eraser";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		let location_path = get_location_path_from_location_id(db, state.init.location_id).await?;

		state.steps = get_many_files_datas(db, &location_path, &state.init.file_path_ids)
			.await?
			.into();

		state.data = Some(FileEraserJobData {
			location_path,
			diretories_to_remove: vec![],
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

		let step = &state.steps[0];

		// Had to use `state.steps[0]` all over the place to appease the borrow checker
		if maybe_missing(step.file_path.is_dir, "file_path.is_dir")? {
			let data = extract_job_data_mut!(state);

			let mut dir = tokio::fs::read_dir(&step.full_path)
				.await
				.map_err(|e| FileIOError::from((&step.full_path, e)))?;

			// Can't use the `step` borrow from here ownwards, or you feel the wrath of the borrow checker
			while let Some(children_entry) = dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&state.steps[0].full_path, e)))?
			{
				let children_path = children_entry.path();

				state.steps.push_back(
					get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&data.location_path,
						&IsolatedFilePathData::new(
							state.init.location_id,
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

				ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);
			}
			data.diretories_to_remove
				.push(state.steps[0].full_path.clone());
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

			sd_crypto::fs::erase::erase(&mut file, file_len as usize, state.init.passes).await?;

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
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&mut self, ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		try_join_all(
			extract_job_data_mut!(state)
				.diretories_to_remove
				.drain(..)
				.map(|data| async {
					fs::remove_dir_all(&data)
						.await
						.map_err(|e| FileIOError::from((data, e)))
				}),
		)
		.await?;

		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
