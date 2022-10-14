use serde::{Deserialize, Serialize};

use crate::job::{JobError, JobResult, JobState, StatefulJob, WorkerContext};

// The Validator is able to:
// - generate a full byte checksum for Objects in a Location
// - generate checksums for all Objects missing without one
// - compare two objects and return true if they are the same
pub struct ObjectValidatorJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobState {
	object_count: usize,
}

// The validator can
#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobInit {
	location_id: Option<i32>,
}

#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJob {
	type Data = ObjectValidatorJobState;
	type Init = ObjectValidatorJobInit;
	type Step = ();

	fn name(&self) -> &'static str {
		"object_validator"
	}

	async fn init(
		&self,
		_ctx: WorkerContext,
		_state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		Ok(())
	}

	async fn execute_step(
		&self,
		_ctx: WorkerContext,
		_state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
