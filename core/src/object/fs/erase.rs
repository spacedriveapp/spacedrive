use crate::job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext};

use std::{hash::Hash, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};
use tracing::{trace, warn};

use super::{context_menu_fs_info, FsInfo};

pub struct FileEraserJob {}

#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileEraserJobInit {
	pub location_id: i32,
	pub path_id: i32,
	pub passes: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileEraserJobStep {
	Directory { path: PathBuf },
	File { path: PathBuf },
}

impl From<FsInfo> for FileEraserJobStep {
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

pub const ERASE_JOB_NAME: &str = "file_eraser";

#[async_trait::async_trait]
impl StatefulJob for FileEraserJob {
	type Init = FileEraserJobInit;
	type Data = FsInfo;
	type Step = FileEraserJobStep;

	fn name(&self) -> &'static str {
		ERASE_JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let fs_info = context_menu_fs_info(
			&ctx.library_ctx.db,
			state.init.location_id,
			state.init.path_id,
		)
		.await?;

		state.data = Some(fs_info.clone());

		state.steps = [fs_info.into()].into_iter().collect();

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];

		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

		match step {
			FileEraserJobStep::File { path } => {
				let mut file = OpenOptions::new()
					.read(true)
					.write(true)
					.open(&path)
					.await?;
				let file_len = file.metadata().await?.len();

				sd_crypto::fs::erase::erase(&mut file, file_len as usize, state.init.passes)
					.await?;
				file.set_len(0).await?;
				file.flush().await?;
				drop(file);

				trace!("Erasing file: {:?}", path);

				tokio::fs::remove_file(&path).await?;
			}
			FileEraserJobStep::Directory { path } => {
				let mut dir = tokio::fs::read_dir(&path).await?;

				while let Some(entry) = dir.next_entry().await? {
					state.steps.push_back(if entry.metadata().await?.is_dir() {
						FileEraserJobStep::Directory { path: entry.path() }
					} else {
						FileEraserJobStep::File { path: entry.path() }
					});

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
		if let Some(ref info) = state.data {
			if info.path_data.is_dir {
				tokio::fs::remove_dir_all(&info.fs_path).await?;
			}
		} else {
			warn!("missing job state, unable to fully finalise erase job");
		}

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
