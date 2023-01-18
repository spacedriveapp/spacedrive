use super::{context_menu_fs_info, get_path_from_location_id, FsInfo};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash, path::PathBuf};

pub struct FileCutterJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileCutterJobState {}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileCutterJobInit {
	pub source_location_id: i32,
	pub source_path_id: i32,
	pub target_location_id: i32,
	pub target_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileCutterJobStep {
	pub source_fs_info: FsInfo,
	pub target_directory: PathBuf,
}

const JOB_NAME: &str = "file_cutter";

#[async_trait::async_trait]
impl StatefulJob for FileCutterJob {
	type Data = FileCutterJobState;
	type Init = FileCutterJobInit;
	type Step = FileCutterJobStep;

	fn name(&self) -> &'static str {
		JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let source_fs_info = context_menu_fs_info(
			&ctx.library_ctx.db,
			state.init.source_location_id,
			state.init.source_path_id,
		)
		.await?;

		let mut full_target_path =
			get_path_from_location_id(&ctx.library_ctx.db, state.init.target_location_id).await?;
		full_target_path.push(state.init.target_path.clone());

		state.steps = VecDeque::new();
		state.steps.push_back(FileCutterJobStep {
			source_fs_info,
			target_directory: full_target_path,
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		let source_info = &step.source_fs_info;

		let mut full_output = step.target_directory.clone();
		full_output.push(
			source_info
				.obj_path
				.clone()
				.file_name()
				.ok_or(JobError::OsStr)?,
		);

		dbg!(source_info.obj_path.clone());
		dbg!(full_output.clone());

		tokio::fs::rename(source_info.obj_path.clone(), full_output.clone()).await?;

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
