use super::{context_menu_fs_info, osstr_to_string, FsInfo, ObjectType};
use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::VecDeque, hash::Hash, path::PathBuf};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};
use tracing::warn;

pub struct FileEraserJob {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileEraserJobState {
	pub root_path: PathBuf,
	pub root_type: ObjectType,
}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileEraserJobInit {
	pub location_id: i32,
	pub path_id: i32,
	pub passes: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileEraserJobStep {
	pub fs_info: FsInfo,
}

const JOB_NAME: &str = "file_eraser";

#[async_trait::async_trait]
impl StatefulJob for FileEraserJob {
	type Data = FileEraserJobState;
	type Init = FileEraserJobInit;
	type Step = FileEraserJobStep;

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

		state.data = Some(FileEraserJobState {
			root_path: fs_info.obj_path.clone(),
			root_type: fs_info.obj_type.clone(),
		});

		state.steps = VecDeque::new();
		state.steps.push_back(FileEraserJobStep { fs_info });

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

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

		match info.obj_type {
			ObjectType::File => {
				let mut file = OpenOptions::new()
					.read(true)
					.write(true)
					.open(info.obj_path.clone())
					.await?;
				let file_len = file.metadata().await?.len();

				sd_crypto::fs::erase::erase(&mut file, file_len as usize, state.init.passes)
					.await?;
				file.set_len(0).await?;
				file.flush().await?;
				drop(file);

				tokio::fs::remove_file(info.obj_path.clone()).await?;
			}
			ObjectType::Directory => {
				let mut dir = tokio::fs::read_dir(&info.obj_path).await?;
				while let Some(entry) = dir.next_entry().await? {
					if entry.metadata().await?.is_dir() {
						let obj_type = ObjectType::Directory;
						state.steps.push_back(FileEraserJobStep {
							fs_info: FsInfo {
								obj_id: None,
								obj_name: String::new(),
								obj_path: entry.path(),
								obj_type,
							},
						});
					} else {
						let obj_type = ObjectType::File;
						state.steps.push_back(FileEraserJobStep {
							fs_info: FsInfo {
								obj_id: None,
								obj_name: osstr_to_string(Some(&entry.file_name()))?,
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
		if let Some(info) = state.data.clone() {
			if info.root_type == ObjectType::Directory {
				tokio::fs::remove_dir_all(info.root_path).await?;
			}
		} else {
			warn!("missing job state, unable to fully finalise erase job");
		}

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
