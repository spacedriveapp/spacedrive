use serde::{Deserialize, Serialize};

use crate::job::{JobError, JobResult, JobState, StatefulJob, WorkerContext};

pub struct FileEncryptorJob {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobState {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobInit {
	location_id: Option<i32>,
	key_uuid: uuid::Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobStep {}

const JOB_NAME: &str = "file_encryptor";

#[async_trait::async_trait]
impl StatefulJob for FileEncryptorJob {
	type Data = FileEncryptorJobState;
	type Init = FileEncryptorJobInit;
	type Step = FileEncryptorJobStep;

	fn name(&self) -> &'static str {
		JOB_NAME
	}

	async fn init(
		&self,
		_ctx: WorkerContext,
		_state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// enumerate files to encrypt
		// populate the steps with them (local file paths)

		Ok(())
	}

	async fn execute_step(
		&self,
		_ctx: WorkerContext,
		_state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// get the key from the key manager
		// encrypt the file

		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
