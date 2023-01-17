use super::{context_menu_fs_info, FsInfo, ObjectType};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash, path::PathBuf};

pub struct FileDuplicatorJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileDuplicatorJobState {
	pub root_path: PathBuf,
	pub root_type: ObjectType,
}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileDuplicatorJobInit {
	pub location_id: i32,
	pub path_id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

		state.data = Some(FileDuplicatorJobState {
			root_path: fs_info.obj_path.clone(),
			root_type: fs_info.obj_type.clone(),
		});

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
		let step = state.steps[0].clone();
		let info = &step.fs_info;

		// temporary
		let job_state = if let Some(st) = state.data.clone() {
			st
		} else {
			return Err(JobError::CryptoError(sd_crypto::Error::MediaLengthParse));
		};

		let mut root_path = if job_state.root_type == ObjectType::File {
			let mut output_path = info.obj_path.clone();
			output_path.set_file_name(
				info.obj_path
					.file_stem()
					.unwrap()
					.to_str()
					.unwrap()
					.to_string() + "-Copy"
					+ &info.obj_path.extension().map_or_else(
						|| String::from(""),
						|x| String::from(".") + x.to_str().unwrap(),
					),
			);
			output_path
		} else {
			let mut output_path = job_state.root_path.clone();
			output_path.set_file_name(
				output_path
					.file_stem()
					.unwrap()
					.to_str()
					.unwrap()
					.to_string() + "-Copy",
			);
			output_path
		};

		match info.obj_type {
			ObjectType::File => {
				let mut path = root_path.clone();

				if job_state.root_type == ObjectType::Directory {
					path.push(
						info.obj_path
							.strip_prefix(job_state.root_path.clone())
							.unwrap(),
					);
				}

				std::fs::copy(info.obj_path.clone(), path.clone())?;
			}
			ObjectType::Directory => {
				for entry in std::fs::read_dir(info.obj_path.clone())? {
					let entry = entry?;
					if entry.metadata()?.is_dir() {
						let obj_type = ObjectType::Directory;
						state.steps.push_back(FileDuplicatorJobStep {
							fs_info: FsInfo {
								obj_id: None,
								obj_name: String::new(),
								obj_path: entry.path(),
								obj_type,
							},
						});

						let mut path = root_path.clone();
						path.push(
							entry
								.path()
								.strip_prefix(job_state.root_path.clone())
								.unwrap(),
						);
						std::fs::create_dir_all(path)?;
					} else {
						let obj_type = ObjectType::File;
						state.steps.push_back(FileDuplicatorJobStep {
							fs_info: FsInfo {
								obj_id: None,
								obj_name: entry.file_name().to_str().unwrap().to_string(),
								obj_path: entry.path(),
								obj_type,
							},
						});
					};

					ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);
				}
			}
		};

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);
		Ok(())
	}

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
