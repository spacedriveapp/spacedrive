use std::{collections::VecDeque, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};

pub struct DeleteFilesJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteFilesJobInit {
	pub location_id: i32,
	pub object_id: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteFilesJobState {
	root_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteFilesJobStep {
	// should maybe be file_path_with_data::Data
	name: String,
	kind: DeleteFilesJobStepKind,
}

#[derive(Serialize, Deserialize, Debug)]
enum DeleteFilesJobStepKind {
	File,
	Directory,
}

pub const ENCRYPT_JOB_NAME: &str = "encryptor";

#[async_trait::async_trait]
impl StatefulJob for DeleteFilesJob {
	type Init = DeleteFilesJobInit;
	type Data = DeleteFilesJobState;
	type Step = DeleteFilesJobStep;

	fn name(&self) -> &'static str {
		ENCRYPT_JOB_NAME
	}

	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let library = ctx.library_ctx();

		let location = library
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.unwrap(); // the root path

		let location_path = location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.unwrap_or_default();

		state.data = Some(DeleteFilesJobState {
			root_path: location_path,
		});

		let item = library
			.db
			.file_path()
			.find_first(vec![file_path::object_id::equals(Some(
				state.init.object_id,
			))])
			.exec()
			.await?
			.unwrap();

		let item_name = item.materialized_path;

		let item_type = if item.is_dir {
			DeleteFilesJobStepKind::Directory
		} else {
			DeleteFilesJobStepKind::File
		};

		let mut steps = VecDeque::new();

		steps.push_back(DeleteFilesJobStep {
			name: item_name,
			kind: item_type,
		});

		state.steps = steps;

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		let step = &state.steps[0];

		let mut path = data.root_path.clone();
		path.push(step.name.clone());

		match step.kind {
			DeleteFilesJobStepKind::Directory => fs::remove_dir_all(path),
			DeleteFilesJobStepKind::File => fs::remove_file(path),
		}
		.unwrap();

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		info!("Finished deleting files at {}", data.root_path.display());
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
