use crate::{
	invalidate_query,
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
};

use std::hash::Hash;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{context_menu_fs_info, FsInfo};

pub struct FileDeleterJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDeleterJobState {}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileDeleterJobInit {
	pub location_id: i32,
	pub path_id: i32,
}

impl JobInitData for FileDeleterJobInit {
	type Job = FileDeleterJob;
}

#[async_trait::async_trait]
impl StatefulJob for FileDeleterJob {
	type Init = FileDeleterJobInit;
	type Data = FileDeleterJobState;
	type Step = FsInfo;

	const NAME: &'static str = "file_deleter";

	fn new() -> Self {
		Self {}
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let fs_info =
			context_menu_fs_info(&ctx.library.db, state.init.location_id, state.init.path_id)
				.await?;

		state.steps = [fs_info].into_iter().collect();

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let info = &state.steps[0];

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be deleted (and where?)

		if info.path_data.is_dir {
			tokio::fs::remove_dir_all(info.fs_path.clone()).await
		} else {
			tokio::fs::remove_file(info.fs_path.clone()).await
		}?;

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		invalidate_query!(ctx.library, "locations.getExplorerData");

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
