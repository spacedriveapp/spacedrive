// use crate::{
// 	invalidate_query,
// 	job::{
// 		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
// 	},
// 	library::Library,
// 	location::{file_path_helper:: location::id::Type},
// 	util::error::FileIOError,
// };

// use sd_crypto::{crypto::Decryptor, header::file::FileHeader, Protected};

// use serde::{Deserialize, Serialize};
// use specta::Type;
// use tokio::fs::File;

// use super::{get_location_path_from_location_id, get_many_files_datas, FileData, BYTES_EXT};
// pub struct FileDecryptorJob;

// // decrypt could have an option to restore metadata (and another specific option for file name? - would turn "output file" into "output path" in the UI)
// #[derive(Serialize, Deserialize, Debug, Type, Hash)]
// pub struct FileDecryptorJobInit {
// 	pub location_id: location::id::Type,
// 	pub file_path_ids: Vec<file_path::id::Type>,
// 	pub mount_associated_key: bool,
// 	pub password: Option<String>, // if this is set, we can assume the user chose password decryption
// 	pub save_to_library: Option<bool>,
// }

// impl JobInitData for FileDecryptorJobInit {
// 	type Job = FileDecryptorJob;
// }

// #[async_trait::async_trait]
// impl StatefulJob for FileDecryptorJob {
// 	type Init = FileDecryptorJobInit;
// 	type Data = ();
// 	type Step = FileData;

// 	const NAME: &'static str = "file_decryptor";

// 	fn new() -> Self {
// 		Self {}
// 	}

// 	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
// 		let Library { db, .. } = &*ctx.library;

// 		state.steps = get_many_files_datas(
// 			db,
// 			get_location_path_from_location_id(db, state.init.location_id).await?,
// 			&state.init.file_path_ids,
// 		)
// 		.await?
// 		.into();

// 		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

// 		Ok(())
// 	}

// 	async fn execute_step(
// 		&self,
// 		ctx: WorkerContext,
// 		state: &mut JobState<Self>,
// 	) -> Result<(), JobError> {
// 		let step = &state.steps[0];
// 		let key_manager = &ctx.library.key_manager;

// 		// handle overwriting checks, and making sure there's enough available space
// 		let output_path = {
// 			let mut path = step.full_path.clone();
// 			let extension = path.extension().map_or("decrypted", |ext| {
// 				if ext == BYTES_EXT {
// 					""
// 				} else {
// 					"decrypted"
// 				}
// 			});
// 			path.set_extension(extension);
// 			path
// 		};

// 		let mut reader = File::open(&step.full_path)
// 			.await
// 			.map_err(|e| FileIOError::from((&step.full_path, e)))?;
// 		let mut writer = File::create(&output_path)
// 			.await
// 			.map_err(|e| FileIOError::from((output_path, e)))?;

// 		let (header, aad) = FileHeader::from_reader(&mut reader).await?;

// 		let master_key = if let Some(password) = state.init.password.clone() {
// 			if let Some(save_to_library) = state.init.save_to_library {
// 				// we can do this first, as `find_key_index` requires a successful decryption (just like `decrypt_master_key`)
// 				let password_bytes = Protected::new(password.as_bytes().to_vec());

// 				if save_to_library {
// 					let index = header.find_key_index(password_bytes.clone()).await?;

// 					// inherit the encryption algorithm from the keyslot
// 					key_manager
// 						.add_to_keystore(
// 							Protected::new(password),
// 							header.algorithm,
// 							header.keyslots[index].hashing_algorithm,
// 							false,
// 							false,
// 							Some(header.keyslots[index].salt),
// 						)
// 						.await?;
// 				}

// 				header.decrypt_master_key(password_bytes).await?
// 			} else {
// 				return Err(JobError::JobDataNotFound(String::from(
// 					"Password decryption selected, but save to library boolean was not included",
// 				)));
// 			}
// 		} else {
// 			if state.init.mount_associated_key {
// 				for key in key_manager.dump_keystore().iter().filter(|x| {
// 					header
// 						.keyslots
// 						.iter()
// 						.any(|k| k.content_salt == x.content_salt)
// 				}) {
// 					key_manager.mount(key.uuid).await.ok();
// 				}
// 			}

// 			let keys = key_manager.enumerate_hashed_keys();

// 			header.decrypt_master_key_from_prehashed(keys).await?
// 		};

// 		let decryptor = Decryptor::new(master_key, header.nonce, header.algorithm)?;

// 		decryptor
// 			.decrypt_streams(&mut reader, &mut writer, &aad)
// 			.await?;

// 		// need to decrypt preview media/metadata, and maybe add an option in the UI so the user can chosoe to restore these values
// 		// for now this can't easily be implemented, as we don't know what the new object id for the file will be (we know the old one, but it may differ)

// 		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
// 			state.step_number + 1,
// 		)]);

// 		Ok(())
// 	}

// 	async fn finalize(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
// 		invalidate_query!(ctx.library, "search.paths");

// 		// mark job as successful
// 		Ok(Some(serde_json::to_value(&state.init)?))
// 	}
// }
