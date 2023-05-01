use crate::{
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{file_path_for_object_validator, MaterializedPath},
	prisma::{file_path, location},
	sync,
};

use std::{collections::VecDeque, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
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

impl JobInitData for ObjectValidatorJobInit {
	type Job = ObjectValidatorJob;
}

#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJob {
	type Init = ObjectValidatorJobInit;
	type Data = ObjectValidatorJobState;
	type Step = file_path_for_object_validator::Data;

	const NAME: &'static str = "object_validator";

	fn new() -> Self {
		Self {}
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		state.steps = db
			.file_path()
			.find_many(vec![
				file_path::location_id::equals(state.init.location_id),
				file_path::is_dir::equals(false),
				file_path::integrity_checksum::equals(None),
			])
			.select(file_path_for_object_validator::select())
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
		let Library { db, sync, .. } = &ctx.library;

		let file_path = &state.steps[0];
		let data = state.data.as_ref().expect("fatal: missing job state");

		// this is to skip files that already have checksums
		// i'm unsure what the desired behaviour is in this case
		// we can also compare old and new checksums here
		// This if is just to make sure, we already queried objects where integrity_checksum is null
		if file_path.integrity_checksum.is_none() {
			let checksum = file_checksum(data.root_path.join(&MaterializedPath::from((
				file_path.location.id,
				&file_path.materialized_path,
			))))
			.await?;

			sync.write_op(
				db,
				sync.shared_update(
					sync::file_path::SyncId {
						pub_id: file_path.pub_id.clone(),
					},
					file_path::integrity_checksum::NAME,
					json!(&checksum),
				),
				db.file_path().update(
					file_path::pub_id::equals(file_path.pub_id.clone()),
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
