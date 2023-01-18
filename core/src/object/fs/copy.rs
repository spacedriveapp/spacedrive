use super::{context_menu_fs_info, get_path_from_location_id, FsInfo, ObjectType};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash, path::PathBuf};

pub struct FileCopierJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobState {
	pub target_path: PathBuf, // target dir prefix too
	pub source_path: PathBuf,
	pub root_type: ObjectType,
	pub target_file_name: String,
}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileCopierJobInit {
	pub source_location_id: i32,
	pub source_path_id: i32,
	pub target_location_id: i32,
	pub target_path: PathBuf,
	pub target_file_name_suffix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobStep {
	pub source_fs_info: FsInfo,
}

const JOB_NAME: &str = "file_copier";

#[async_trait::async_trait]
impl StatefulJob for FileCopierJob {
	type Data = FileCopierJobState;
	type Init = FileCopierJobInit;
	type Step = FileCopierJobStep;

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

		let target_file_name = state.init.target_file_name_suffix.clone().map_or_else(
			|| {
				source_fs_info
					.obj_path
					.clone()
					.file_name()
					.unwrap()
					.to_str()
					.unwrap()
					.to_string()
			},
			|s| {
				// should turn /root/x.tar.gz into /root/x.tar - clone.gz (file_prefix is still unstable :|)
				// need to get the original file name, add `s` onto it and preserve the extension fully
				let mut path = source_fs_info.obj_path.clone();
				path.pop();
				path.push(
					source_fs_info
						.obj_path
						.clone()
						.file_stem()
						.unwrap()
						.to_str()
						.unwrap()
						.to_string() + &s,
				);

				// if source has extension, add it back to our target file name
				source_fs_info.obj_path.clone().extension().map(|x| {
					path.set_file_name(
						path.file_name().unwrap().to_str().unwrap().to_string()
							+ "." + x.to_str().unwrap(),
					)
				});

				path.to_str().unwrap().to_string()
			},
		);

		state.data = Some(FileCopierJobState {
			target_path: full_target_path,
			source_path: source_fs_info.obj_path.clone(),
			root_type: source_fs_info.obj_type.clone(),
			target_file_name,
		});

		state.steps = VecDeque::new();
		state.steps.push_back(FileCopierJobStep { source_fs_info });

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = state.steps[0].clone();
		let info = &step.source_fs_info;

		let job_state = state.data.clone().ok_or(JobError::MissingData {
			value: String::from("job state"),
		})?;

		match info.obj_type {
			ObjectType::File => {
				let mut path = job_state.target_path.clone();
				path.push(job_state.target_file_name.clone());

				if job_state.root_type == ObjectType::Directory {
					path.push(
						info.obj_path
							.strip_prefix(job_state.source_path.clone())
							.unwrap(),
					);
				}

				std::fs::copy(info.obj_path.clone(), path.clone())?;
			}
			ObjectType::Directory => {
				for entry in std::fs::read_dir(info.obj_path.clone())? {
					let entry = entry?;
					if entry.metadata()?.is_dir() {
						state.steps.push_back(FileCopierJobStep {
							source_fs_info: FsInfo {
								obj_id: None,
								obj_name: String::new(),
								obj_path: entry.path(),
								obj_type: ObjectType::Directory,
							},
						});

						let mut path = job_state.target_path.clone();
						path.push(job_state.target_file_name.clone());
						path.push(
							entry
								.path()
								.strip_prefix(job_state.source_path.clone())
								.unwrap(),
						);

						std::fs::create_dir_all(path)?;
					} else {
						state.steps.push_back(FileCopierJobStep {
							source_fs_info: FsInfo {
								obj_id: None,
								obj_name: entry.file_name().to_str().unwrap().to_string(),
								obj_path: entry.path(),
								obj_type: ObjectType::File,
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
