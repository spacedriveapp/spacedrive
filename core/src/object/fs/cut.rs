use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::trace;

use super::{context_menu_fs_info, get_path_from_location_id, FsInfo};

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

pub const CUT_JOB_NAME: &str = "file_cutter";

#[async_trait::async_trait]
impl StatefulJob for FileCutterJob {
	type Init = FileCutterJobInit;
	type Data = FileCutterJobState;
	type Step = FileCutterJobStep;

	fn name(&self) -> &'static str {
		CUT_JOB_NAME
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
		full_target_path.push(&state.init.target_path);

		state.steps = [FileCutterJobStep {
			source_fs_info,
			target_directory: full_target_path,
		}]
		.into_iter()
		.collect();

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

		let full_output = step
			.target_directory
			.join(source_info.fs_path.file_name().ok_or(JobError::OsStr)?);

		trace!("Cutting {:?} to {:?}", source_info.fs_path, full_output);

		tokio::fs::rename(&source_info.fs_path, &full_output).await?;

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&mut self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
