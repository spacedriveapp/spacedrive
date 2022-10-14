use serde::{Deserialize, Serialize};

use std::{collections::VecDeque, path::PathBuf};

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{self, file_path, location, object},
};

use tracing::info;

use super::hash::file_checksum;

// The Validator is able to:
// - generate a full byte checksum for Objects in a Location
// - generate checksums for all Objects missing without one
// - compare two objects and return true if they are the same
pub struct ObjectValidatorJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobState {
	pub root_path: PathBuf,
	pub task_count: usize,
}

// The validator can
#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobInit {
	pub location_id: i32,
	pub path: PathBuf,
	pub background: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobStep {
	pub path: file_path::Data,
}

#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJob {
	type Data = ObjectValidatorJobState;
	type Init = ObjectValidatorJobInit;
	type Step = ObjectValidatorJobStep;

	fn name(&self) -> &'static str {
		"object_validator"
	}

	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let library_ctx = ctx.library_ctx();

		state.steps = library_ctx
			.db
			.file_path()
			.find_many(vec![file_path::location_id::equals(state.init.location_id)])
			.exec()
			.await?
			.into_iter()
			.map(|path| ObjectValidatorJobStep { path })
			.collect::<VecDeque<_>>();

		let location = library_ctx
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.unwrap();

		state.data = Some(ObjectValidatorJobState {
			root_path: location.local_path.as_ref().map(PathBuf::from).unwrap(),
			task_count: state.steps.len(),
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		let library_ctx = ctx.library_ctx();

		let data = state.data.as_ref().expect("fatal: missing job state");

		let path = data.root_path.join(&step.path.materialized_path);

		// skip directories
		if path.is_dir() {
			return Ok(());
		}

		if let Some(object_id) = step.path.object_id {
			// this is to skip files that already have checksums
			// i'm unsure what the desired behaviour is in this case
			// we can also compare old and new checksums here
			let object = library_ctx
				.db
				.object()
				.find_unique(object::id::equals(object_id))
				.exec()
				.await?
				.unwrap();
			if object.integrity_checksum.is_some() {
				return Ok(());
			}

			let hash = file_checksum(path).await?;

			library_ctx
				.db
				.object()
				.update(
					object::id::equals(object_id),
					vec![prisma::object::SetParam::SetIntegrityChecksum(Some(hash))],
				)
				.exec()
				.await?;
		}

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
		info!(
			"finalizing validator job at {}: {} tasks",
			data.root_path.display(),
			data.task_count
		);

		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
