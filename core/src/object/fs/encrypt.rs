use std::{
	collections::VecDeque,
	fs::{self, File},
	io::Read,
	path::PathBuf,
};

use chrono::FixedOffset;
use sd_crypto::{
	crypto::stream::{Algorithm, StreamEncryption},
	header::{file::FileHeader, keyslot::Keyslot},
	primitives::{
		generate_master_key, LATEST_FILE_HEADER, LATEST_KEYSLOT, LATEST_METADATA,
		LATEST_PREVIEW_MEDIA,
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;

use crate::{
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	prisma::{file_path, location, object},
};

pub struct FileEncryptorJob;

#[derive(Serialize, Deserialize, Debug)]
enum ObjectType {
	File,
	Directory,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobState {}

#[derive(Serialize, Deserialize, Type, Hash)]
pub struct FileEncryptorJobInit {
	pub location_id: i32,
	pub path_id: i32,
	pub key_uuid: uuid::Uuid,
	pub algorithm: Algorithm,
	pub metadata: bool,
	pub preview_media: bool,
	pub output_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobStep {
	obj_name: String,
	obj_path: PathBuf,
	obj_type: ObjectType,
	obj_id: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
	pub path_id: i32,
	pub name: String,
	pub hidden: bool,
	pub favourite: bool,
	pub important: bool,
	pub note: Option<String>,
	pub date_created: chrono::DateTime<FixedOffset>,
	pub date_modified: chrono::DateTime<FixedOffset>,
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

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		// enumerate files to encrypt
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
			obj_id: item.object_id,
		});

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];

		match step.obj_type {
			ObjectType::File => {
				// handle overwriting checks, and making sure there's enough available space

				let user_key = ctx
					.library_ctx
					.key_manager
					.access_keymount(state.init.key_uuid)?
					.hashed_key;

				let user_key_details = ctx
					.library_ctx
					.key_manager
					.access_keystore(state.init.key_uuid)?;

				let output_path = state.init.output_path.clone().map_or_else(
					|| {
						let mut path = step.obj_path.clone();
						let extension = path.extension().map_or_else(
							|| Ok("sdenc".to_string()),
							|extension| {
								Ok::<String, JobError>(
									extension
										.to_str()
										.ok_or(JobError::EarlyFinish {
											name: self.name().to_string(),
											reason: "path isn't valid UTF-8".to_string(),
										})?
										.to_string() + ".sdenc",
								)
							},
						)?;

						path.set_extension(extension);
						Ok::<PathBuf, JobError>(path)
					},
					Ok,
				)?;

				let mut reader = File::open(step.obj_path.clone())?;
				let mut writer = File::create(output_path)?;

				let master_key = generate_master_key();

				let mut header = FileHeader::new(
					LATEST_FILE_HEADER,
					state.init.algorithm,
					vec![Keyslot::new(
						LATEST_KEYSLOT,
						state.init.algorithm,
						user_key_details.hashing_algorithm,
						user_key_details.content_salt,
						user_key,
						master_key.clone(),
					)?],
				);

				if state.init.metadata || state.init.preview_media {
					// if any are requested, we can make the query as it'll be used at least once
					if let Some(obj_id) = step.obj_id {
						let object = ctx
							.library_ctx
							.db
							.object()
							.find_unique(object::id::equals(obj_id))
							.exec()
							.await?
							.ok_or(JobError::JobDataNotFound(String::from(
								"can't find information about the object",
							)))?;

						if state.init.metadata {
							let metadata = Metadata {
								path_id: state.init.path_id,
								name: step.obj_name.clone(),
								hidden: object.hidden,
								favourite: object.favorite,
								important: object.important,
								note: object.note,
								date_created: object.date_created,
								date_modified: object.date_modified,
							};

							header.add_metadata(
								LATEST_METADATA,
								state.init.algorithm,
								master_key.clone(),
								&metadata,
							)?;
						}

						// if state.init.preview_media
						// 	&& (object.has_thumbnail
						// 		|| object.has_video_preview || object.has_thumbstrip)

						// may not be the best - pvm isn't guaranteed to be webp
						let pvm_path = ctx
							.library_ctx
							.config()
							.data_directory()
							.join("thumbnails")
							.join(object.cas_id + ".webp");

						if fs::metadata(pvm_path.clone()).is_ok() {
							let mut pvm_bytes = Vec::new();
							let mut pvm_file = File::open(pvm_path)?;
							pvm_file.read_to_end(&mut pvm_bytes)?;

							header.add_preview_media(
								LATEST_PREVIEW_MEDIA,
								state.init.algorithm,
								master_key.clone(),
								&pvm_bytes,
							)?;
						}
					} else {
						warn!(
							"skipping metadata/preview media inclusion, no associated object found"
						)
					}
				}

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

	async fn finalize(&self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
