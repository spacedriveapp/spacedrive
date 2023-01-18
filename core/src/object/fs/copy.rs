use super::{context_menu_fs_info, get_path_from_location_id, FsInfo, ObjectType};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, ffi::OsStr, hash::Hash, path::PathBuf};

pub struct FileCopierJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobState {
	pub target_path: PathBuf, // target dir prefix too
	pub source_path: PathBuf,
	pub root_type: ObjectType,
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

pub fn osstr_to_string(os_str: Option<&OsStr>) -> Result<String, JobError> {
	let string = os_str
		.ok_or(JobError::MissingData {
			value: "no in option OsStr".to_string(),
		})?
		.to_str()
		.ok_or(JobError::MissingData {
			value: "issue converting OsStr to &str".to_string(),
		})?
		.to_string();

	Ok(string)
}

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

		// add the currently viewed subdirectory to the location root
		full_target_path.push(state.init.target_path.clone());

		// extension wizardry for cloning and such
		// if no suffix has been selected, just use the file name
		// if a suffix is provided and it's a directory, use the directory name + suffix
		// if a suffix is provided and it's a file, use the (file name + suffix).extension
		let target_file_name = state.init.target_file_name_suffix.clone().map_or_else(
			|| {
				// if there's no provided suffix, just use source name
				osstr_to_string(source_fs_info.obj_path.clone().file_name())
			},
			|s| match source_fs_info.obj_type {
				ObjectType::Directory => {
					Ok(osstr_to_string(source_fs_info.obj_path.clone().file_name())? + &s)
				}
				ObjectType::File => Ok(osstr_to_string(
					source_fs_info.obj_path.clone().file_stem(),
				)? + &s + &source_fs_info.obj_path.extension().map_or(
					Ok::<_, JobError>("".to_string()),
					|x| {
						Ok(".".to_string()
							+ x.to_str().ok_or(JobError::MissingData {
								value: "issue converting OsStr to &str".to_string(),
							})?)
					},
				)?),
			},
		)?;

		full_target_path.push(target_file_name);

		state.data = Some(FileCopierJobState {
			target_path: full_target_path,
			source_path: source_fs_info.obj_path.clone(),
			root_type: source_fs_info.obj_type.clone(),
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

				if job_state.root_type == ObjectType::Directory {
					// if root type is a dir, we need to preserve structure by making paths relative
					path.push(
						info.obj_path
							.strip_prefix(job_state.source_path.clone())
							.unwrap(),
					);
				}

				std::fs::copy(info.obj_path.clone(), path.clone())?;
			}
			ObjectType::Directory => {
				// if this is the very first path, create the target dir
				// fixes copying dirs with no child directories
				if job_state.root_type == ObjectType::Directory
					&& job_state.source_path == info.obj_path
				{
					std::fs::create_dir_all(&job_state.target_path)?;
				}

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
