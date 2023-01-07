use std::{collections::VecDeque, fs::File, path::PathBuf};

use sd_crypto::{crypto::stream::StreamDecryption, header::file::FileHeader, Protected};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location},
};
pub struct FileDecryptorJob;
#[derive(Serialize, Deserialize, Debug)]
pub struct FileDecryptorJobState {}

// decrypt could have an option to restore metadata (and another specific option for file name? - would turn "output file" into "output path" in the UI)
#[derive(Serialize, Deserialize, Debug, Type, Hash)]
pub struct FileDecryptorJobInit {
	pub location_id: i32,
	pub path_id: i32,
	pub output_path: Option<PathBuf>,
	pub password: Option<String>, // if this is set, we can assume the user chose password decryption
	pub save_to_library: Option<bool>,
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

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		// enumerate files to decrypt
		// populate the steps with them (local file paths)
		let location = ctx
			.library_ctx
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.ok_or(JobError::EarlyFinish {
				name: self.name().to_string(),
				reason: "can't find location".to_string(),
			})?;
		let root_path =
			location
				.local_path
				.as_ref()
				.map(PathBuf::from)
				.ok_or(JobError::EarlyFinish {
					name: self.name().to_string(),
					reason: "can't get path as pathbuf".to_string(),
				})?;
		let item = ctx
			.library_ctx
			.db
			.file_path()
			.find_unique(file_path::location_id_id(
				state.init.location_id,
				state.init.path_id,
			))
			.exec()
			.await?
			.ok_or(JobError::EarlyFinish {
				name: self.name().to_string(),
				reason: "can't find file_path with location id and path id".to_string(),
			})?;

		let obj_name = item.materialized_path;

		let mut obj_path = root_path.clone();
		obj_path.push(obj_name.clone());

		state.steps = VecDeque::new();
		state
			.steps
			.push_back(FileDecryptorJobStep { obj_name, obj_path });

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		// handle overwriting checks, and making sure there's enough available space

		let output_path = state.init.output_path.clone().map_or_else(
			|| {
				let mut path = step.obj_path.clone();
				let extension = path.extension().map_or("decrypted", |ext| {
					if ext == ".sdenc" {
						""
					} else {
						"decrypted"
					}
				});
				path.set_extension(extension);
				path
			},
			|p| p,
		);

		let mut reader = File::open(step.obj_path.clone())?;
		let mut writer = File::create(output_path)?;

		let (header, aad) = FileHeader::from_reader(&mut reader)?;

		let master_key = if let Some(password) = state.init.password.clone() {
			if let Some(save_to_library) = state.init.save_to_library {
				let password = Protected::new(password.into_bytes());

				// we can do this first, as `find_key_index` requires a successful decryption (just like `decrypt_master_key`)
				if save_to_library {
					let index = header.find_key_index(password.clone())?;

					// inherit the encryption algorithm from the keyslot
					ctx.library_ctx.key_manager.add_to_keystore(
						password.clone(),
						header.algorithm,
						header.keyslots[index].hashing_algorithm,
						false,
						false,
						Some(header.keyslots[index].salt),
					)?;
				}

				header.decrypt_master_key(password)?
			} else {
				return Err(JobError::JobDataNotFound(String::from(
					"Password decryption selected, but save to library boolean was not included",
				)));
			}
		} else {
			let keys = ctx.library_ctx.key_manager.enumerate_hashed_keys();

			header.decrypt_master_key_from_prehashed(keys)?
		};

		let decryptor = StreamDecryption::new(master_key, &header.nonce, header.algorithm)?;

		decryptor.decrypt_streams(&mut reader, &mut writer, &aad)?;

		// need to decrypt preview media/metadata, and maybe add an option in the UI so the user can chosoe to restore these values
		// for now this can't easily be implemented, as we don't know what the new object id for the file will be (we know the old one, but it may differ)

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
