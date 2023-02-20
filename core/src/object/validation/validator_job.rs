use serde::{Deserialize, Serialize};
use serde_json::json;

use std::{collections::VecDeque, path::PathBuf};

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::LibraryContext,
	prisma::{file_path, location},
	sync,
};

use tracing::info;

use super::hash::file_checksum;

pub const VALIDATOR_JOB_NAME: &str = "object_validator";

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
	id
	materialized_path
	integrity_checksum
	location: select {
		id
		pub_id
	}
	object: select {
		id
	}
});

#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJob {
	type Init = ObjectValidatorJobInit;
	type Data = ObjectValidatorJobState;
	type Step = file_path_and_object::Data;

	fn name(&self) -> &'static str {
		VALIDATOR_JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let db = &ctx.library_ctx.db;

		state.steps = db
			.file_path()
			.find_many(vec![
				file_path::location_id::equals(state.init.location_id),
				file_path::is_dir::equals(false),
				file_path::integrity_checksum::equals(None),
			])
			.select(file_path_and_object::select())
			.exec()
			.await?
			.into_iter()
			.collect::<VecDeque<_>>();

		let location = db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.unwrap();

		state.data = Some(ObjectValidatorJobState {
			root_path: location.path.into(),
			task_count: state.steps.len(),
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let LibraryContext { db, sync, .. } = &ctx.library_ctx;

		let file_path = &state.steps[0];
		let data = state.data.as_ref().expect("fatal: missing job state");

		// this is to skip files that already have checksums
		// i'm unsure what the desired behaviour is in this case
		// we can also compare old and new checksums here
		// This if is just to make sure, we already queried objects where integrity_checksum is null
		if file_path.integrity_checksum.is_none() {
			let checksum = file_checksum(data.root_path.join(&file_path.materialized_path)).await?;

			sync.write_op(
				db,
				sync.owned_update(
					sync::file_path::SyncId {
						id: file_path.id,
						location: sync::location::SyncId {
							pub_id: file_path.location.pub_id.clone(),
						},
					},
					[("integrity_checksum", json!(Some(&checksum)))],
				),
				db.file_path().update(
					file_path::location_id_id(file_path.location.id, file_path.id),
					vec![file_path::integrity_checksum::set(Some(checksum))],
				),
			)
			.await?;
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&mut self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
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
