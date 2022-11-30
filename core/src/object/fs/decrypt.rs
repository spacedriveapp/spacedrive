use std::{collections::VecDeque, path::PathBuf};

use sd_crypto::{crypto::stream::StreamDecryption, header::file::FileHeader};
use serde::{Deserialize, Serialize};

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};

pub struct FileDecryptorJob;
#[derive(Serialize, Deserialize, Debug)]
pub struct FileDecryptorJobState {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDecryptorJobInit {
	pub location_id: i32,
	pub object_id: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileDecryptorJobStep {
	obj_name: String,
	obj_path: PathBuf,
}

const JOB_NAME: &str = "file_decryptor";

#[async_trait::async_trait]
impl StatefulJob for FileDecryptorJob {
	type Data = FileDecryptorJobState;
	type Init = FileDecryptorJobInit;
	type Step = FileDecryptorJobStep;

	fn name(&self) -> &'static str {
		JOB_NAME
	}

	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// enumerate files to decrypt
		// populate the steps with them (local file paths)
		let library = ctx.library_ctx();

		let location = library
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.expect("critical error: can't find location");

		let root_path = location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.unwrap_or_default();

		let item = library
			.db
			.file_path()
			.find_first(vec![file_path::object_id::equals(Some(
				state.init.object_id,
			))])
			.exec()
			.await?
			.expect("critical error: can't find object");

		let obj_name = item.materialized_path;

		let mut obj_path = root_path.clone();
		obj_path.push(obj_name.clone());

		state.steps = VecDeque::new();
		state
			.steps
			.push_back(FileDecryptorJobStep { obj_name, obj_path });

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// get the key from the key manager
		// decrypt the file

		let step = &state.steps[0];
		// handle overwriting checks, and making sure there's enough available space

		let keys = ctx.library_ctx().key_manager.enumerate_hashed_keys();

		let mut output_path = step.obj_path.clone();

		// i really can't decide on the functionality of this
		// maybe we should open a dialog in JS, and have the default as the file name without the ".sdx",
		// this would let the user choose
		// we don't do any overwriting checks as of yet, maybe these should be front-end though
		let extension = if let Some(ext) = output_path.extension() {
			if ext == ".sdx" {
				"decrypted"
			} else {
				"decrypted"
			}
		} else {
			"decrypted"
		};

		output_path.set_extension(extension);

		let mut reader = std::fs::File::open(step.obj_path.clone())?;
		let mut writer = std::fs::File::create(output_path)?;

		let (header, aad) = FileHeader::deserialize(&mut reader).unwrap();

		let master_key = header.decrypt_master_key_from_prehashed(keys).unwrap();

		let decryptor = StreamDecryption::new(master_key, &header.nonce, header.algorithm).unwrap();

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &aad)
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
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
