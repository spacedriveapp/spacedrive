use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::trace;

use super::{context_menu_fs_info, get_path_from_location_id, osstr_to_string, FsInfo};

pub struct FileCopierJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCopierJobState {
	pub target_path: PathBuf, // target dir prefix too
	pub source_fs_info: FsInfo,
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
pub enum FileCopierJobStep {
	Directory { path: PathBuf },
	File { path: PathBuf },
}

impl From<FsInfo> for FileCopierJobStep {
	fn from(value: FsInfo) -> Self {
		if value.path_data.is_dir {
			Self::Directory {
				path: value.fs_path,
			}
		} else {
			Self::File {
				path: value.fs_path,
			}
		}
	}
}

pub const COPY_JOB_NAME: &str = "file_copier";

#[async_trait::async_trait]
impl StatefulJob for FileCopierJob {
	type Init = FileCopierJobInit;
	type Data = FileCopierJobState;
	type Step = FileCopierJobStep;

	fn name(&self) -> &'static str {
		COPY_JOB_NAME
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
		full_target_path.push(&state.init.target_path);

		// extension wizardry for cloning and such
		// if no suffix has been selected, just use the file name
		// if a suffix is provided and it's a directory, use the directory name + suffix
		// if a suffix is provided and it's a file, use the (file name + suffix).extension
		let file_name = osstr_to_string(source_fs_info.fs_path.file_name())?;

		let target_file_name = state.init.target_file_name_suffix.as_ref().map_or_else(
			|| Ok::<_, JobError>(file_name.clone()),
			|suffix| {
				Ok(if source_fs_info.path_data.is_dir {
					format!("{file_name}{suffix}")
				} else {
					osstr_to_string(source_fs_info.fs_path.file_stem())?
						+ suffix + &source_fs_info.fs_path.extension().map_or_else(
						|| Ok(String::new()),
						|ext| ext.to_str().map(|e| format!(".{e}")).ok_or(JobError::OsStr),
					)?
				})
			},
		)?;

		full_target_path.push(target_file_name);

		state.data = Some(FileCopierJobState {
			target_path: full_target_path,
			source_fs_info: source_fs_info.clone(),
		});

		state.steps = [source_fs_info.into()].into_iter().collect();

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];

		let job_state = state.data.as_ref().ok_or(JobError::MissingData {
			value: String::from("job state"),
		})?;

		match step {
			FileCopierJobStep::File { path } => {
				let mut target_path = job_state.target_path.clone();

				if job_state.source_fs_info.path_data.is_dir {
					// if root type is a dir, we need to preserve structure by making paths relative
					target_path.push(
						path.strip_prefix(&job_state.source_fs_info.fs_path)
							.map_err(|_| JobError::Path)?,
					);
				}

				trace!("Copying from {:?} to {:?}", path, target_path);

				tokio::fs::copy(&path, &target_path).await?;
			}
			FileCopierJobStep::Directory { path } => {
				// if this is the very first path, create the target dir
				// fixes copying dirs with no child directories
				if job_state.source_fs_info.path_data.is_dir
					&& &job_state.source_fs_info.fs_path == path
				{
					tokio::fs::create_dir_all(&job_state.target_path).await?;
				}

				let mut dir = tokio::fs::read_dir(&path).await?;

				while let Some(entry) = dir.next_entry().await? {
					if entry.metadata().await?.is_dir() {
						state
							.steps
							.push_back(FileCopierJobStep::Directory { path: entry.path() });

						tokio::fs::create_dir_all(
							job_state.target_path.join(
								entry
									.path()
									.strip_prefix(&job_state.source_fs_info.fs_path)
									.map_err(|_| JobError::Path)?,
							),
						)
						.await?;
					} else {
						state
							.steps
							.push_back(FileCopierJobStep::File { path: entry.path() });
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
