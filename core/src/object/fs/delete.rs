use super::{context_menu_fs_info, FsInfo, ObjectType};
use crate::job::{
	JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash};

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

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDeleterJobStep {
	pub fs_info: FsInfo,
}

#[async_trait::async_trait]
impl StatefulJob for FileDeleterJob {
	type Data = FileDeleterJobState;
	type Init = FileDeleterJobInit;
	type Step = FileDeleterJobStep;

	const NAME: &'static str = "file_deleter";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let fs_info = context_menu_fs_info(
			&ctx.library_ctx.db,
			state.init.location_id,
			state.init.path_id,
		)
		.await?;

		state.steps = VecDeque::new();
		state.steps.push_back(FileDeleterJobStep { fs_info });

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		let info = &step.fs_info;

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be deleted (and where?)

		match info.obj_type {
			ObjectType::File => tokio::fs::remove_file(info.obj_path.clone()).await,
			ObjectType::Directory => tokio::fs::remove_dir_all(info.obj_path.clone()).await,
		}?;

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&self, _ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
