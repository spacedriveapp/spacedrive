use crate::{
	invalidate_query,
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	prisma::{file_path, location},
	util::{db::maybe_missing, error::FileIOError},
};

use std::hash::Hash;

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::fs;

use super::{get_location_path_from_location_id, get_many_files_datas, FileData};

pub struct FileDeleterJob {}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileDeleterJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
}

impl JobInitData for FileDeleterJobInit {
	type Job = FileDeleterJob;
}

#[async_trait::async_trait]
impl StatefulJob for FileDeleterJob {
	type Init = FileDeleterJobInit;
	type Data = ();
	type Step = FileData;

	const NAME: &'static str = "file_deleter";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		state.steps = get_many_files_datas(
			db,
			get_location_path_from_location_id(db, state.init.location_id).await?,
			&state.init.file_path_ids,
		)
		.await?
		.into_iter()
		.collect();

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be deleted (and where?)

		if maybe_missing(step.file_path.is_dir, "file_path.is_dir")? {
			fs::remove_dir_all(&step.full_path).await
		} else {
			fs::remove_file(&step.full_path).await
		}
		.map_err(|e| FileIOError::from((&step.full_path, e)))?;

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
