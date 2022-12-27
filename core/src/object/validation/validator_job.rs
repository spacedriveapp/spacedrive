use serde::{Deserialize, Serialize};

use std::{collections::VecDeque, path::PathBuf};

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location, object},
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
#[derive(Serialize, Deserialize, Debug, Hash)]
pub struct ObjectValidatorJobInit {
	pub location_id: i32,
	pub path: PathBuf,
	pub background: bool,
}

file_path::select!(file_path_and_object {
	materialized_path
	object: select {
		id
		integrity_checksum
	}
});

#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJob {
	type Init = ObjectValidatorJobInit;
	type Data = ObjectValidatorJobState;
	type Step = file_path_and_object::Data;

	const NAME: &'static str = "object_validator";

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		state.steps = ctx
			.library_ctx
			.db
			.file_path()
			.find_many(vec![
				file_path::location_id::equals(state.init.location_id),
				file_path::is_dir::equals(false),
				file_path::object::is(vec![object::integrity_checksum::equals(None)]),
			])
			.select(file_path_and_object::select())
			.exec()
			.await?
			.into_iter()
			.collect::<VecDeque<_>>();

		let location = ctx
			.library_ctx
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
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		let data = state.data.as_ref().expect("fatal: missing job state");

		// this is to skip files that already have checksums
		// i'm unsure what the desired behaviour is in this case
		// we can also compare old and new checksums here
		if let Some(ref object) = step.object {
			// This if is just to make sure, we already queried objects where integrity_checksum is null
			if object.integrity_checksum.is_none() {
				ctx.library_ctx
					.db
					.object()
					.update(
						object::id::equals(object.id),
						vec![object::SetParam::SetIntegrityChecksum(Some(
							file_checksum(data.root_path.join(&step.materialized_path)).await?,
						))],
					)
					.exec()
					.await?;
			}
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&self, _ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
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
