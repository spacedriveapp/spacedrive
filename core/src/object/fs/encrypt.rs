use std::{collections::VecDeque, path::PathBuf};

use sd_crypto::{
	crypto::stream::StreamEncryption,
	header::{file::FileHeader, keyslot::Keyslot},
	primitives::{generate_master_key, LATEST_FILE_HEADER, LATEST_KEYSLOT},
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};

pub struct FileEncryptorJob;

#[derive(Serialize, Deserialize, Debug)]
enum ObjectType {
	File,
	Directory,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobState {}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobInit {
	pub location_id: i32,
	pub object_id: i32,
	pub key_uuid: uuid::Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobStep {
	obj_name: String,
	obj_path: PathBuf,
	obj_type: ObjectType,
}

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
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// enumerate files to encrypt
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

		// i don't know if this covers symlinks
		let obj_type = if item.is_dir {
			ObjectType::Directory
		} else {
			ObjectType::File
		};

		state.steps = VecDeque::new();
		state.steps.push_back(FileEncryptorJobStep {
			obj_name,
			obj_path,
			obj_type,
		});

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		// get the key from the key manager
		// encrypt the file

		let step = &state.steps[0];

		match step.obj_type {
			ObjectType::File => {
				// handle overwriting checks, and making sure there's enough available space

				let user_key = ctx
					.library_ctx()
					.key_manager
					.access_keymount(state.init.key_uuid)?
					.hashed_key;

				let user_key_details = ctx
					.library_ctx()
					.key_manager
					.access_keystore(state.init.key_uuid)?;

				let mut output_path = step.obj_path.clone();
				let extension = if let Some(ext) = output_path.extension() {
					ext.to_str().unwrap().to_string() + ".sd"
				} else {
					".sd".to_string()
				};

				output_path.set_extension(extension);

				let mut reader = std::fs::File::open(step.obj_path.clone())?;
				let mut writer = std::fs::File::create(output_path)?;

				let master_key = generate_master_key();

				let keyslots = vec![Keyslot::new(
					LATEST_KEYSLOT,
					user_key_details.algorithm,
					user_key_details.hashing_algorithm,
					user_key_details.content_salt,
					user_key,
					&master_key,
				)?];

				let header =
					FileHeader::new(LATEST_FILE_HEADER, user_key_details.algorithm, keyslots);

				header.write(&mut writer)?;

				let encryptor = StreamEncryption::new(master_key, &header.nonce, header.algorithm)?;

				encryptor.encrypt_streams(&mut reader, &mut writer, &header.generate_aad())?;
			}
			_ => warn!(
				"encryption is skipping {} as it isn't a file",
				step.obj_name
			),
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
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
