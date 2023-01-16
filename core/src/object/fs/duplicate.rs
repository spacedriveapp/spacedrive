use super::{context_menu_fs_info, FsInfo, ObjectType};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash};

pub struct FileDuplicatorJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDuplicatorJobState {}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileDuplicatorJobInit {
	pub location_id: i32,
	pub path_id: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDuplicatorJobStep {
	pub fs_info: FsInfo,
}

const JOB_NAME: &str = "file_duplicator";

#[async_trait::async_trait]
impl StatefulJob for FileDuplicatorJob {
	type Data = FileDuplicatorJobState;
	type Init = FileDuplicatorJobInit;
	type Step = FileDuplicatorJobStep;

	fn name(&self) -> &'static str {
		JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let fs_info = context_menu_fs_info(
			&ctx.library_ctx.db,
			state.init.location_id,
			state.init.path_id,
		)
		.await?;

		state.steps = VecDeque::new();
		state.steps.push_back(FileDuplicatorJobStep { fs_info });

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		let info = &step.fs_info;

		match info.obj_type {
			ObjectType::File => {
				let mut output_path = info.obj_path.clone();
				output_path.set_file_name(
					info.obj_path
						.clone()
						.file_stem()
						.unwrap()
						.to_str()
						.unwrap()
						.to_string() + "-Copy" + "."
						+ info
							.obj_path
							.extension()
							.map_or_else(|| "", |x| x.to_str().unwrap()),
				);
				std::fs::copy(info.obj_path.clone(), output_path)
			}
			ObjectType::Directory => todo!(),
		}?;

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
