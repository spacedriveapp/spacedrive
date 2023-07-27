// use crate::{
// 	invalidate_query,
// 	job::*,
// 	library::Library,
// 	location::{file_path_helper:: location::id::Type},
// 	util::error::{FileIOError, NonUtf8PathError},
// };

// use sd_crypto::{
// 	crypto::Encryptor,
// 	header::{file::FileHeader, keyslot::Keyslot},
// 	primitives::{LATEST_FILE_HEADER, LATEST_KEYSLOT, LATEST_METADATA, LATEST_PREVIEW_MEDIA},
// 	types::{Algorithm, Key},
// };

// use chrono::FixedOffset;
// use serde::{Deserialize, Serialize};
// use specta::Type;
// use tokio::{
// 	fs::{self, File},
// 	io,
// };
// use tracing::{error, warn};
// use uuid::Uuid;

// use super::{
// 	error::FileSystemJobsError, get_location_path_from_location_id, get_many_files_datas, FileData,
// 	BYTES_EXT,
// };

// pub struct FileEncryptorJob;

// #[derive(Serialize, Deserialize, Type, Hash)]
// pub struct FileEncryptorJobInit {
// 	pub location_id: location::id::Type,
// 	pub file_path_ids: Vec<file_path::id::Type>,
// 	pub key_uuid: Uuid,
// 	pub algorithm: Algorithm,
// 	pub metadata: bool,
// 	pub preview_media: bool,
// }

// #[derive(Serialize, Deserialize)]
// pub struct Metadata {
// 	pub file_path_id: file_path::id::Type,
// 	pub name: String,
// 	pub hidden: bool,
// 	pub favorite: bool,
// 	pub important: bool,
// 	pub note: Option<String>,
// 	pub date_created: chrono::DateTime<FixedOffset>,
// }

// impl JobInitData for FileEncryptorJobInit {
// 	type Job = FileEncryptorJob;
// }

// #[async_trait::async_trait]
// impl StatefulJob for FileEncryptorJob {
// 	type Init = FileEncryptorJobInit;
// 	type Data = ();
// 	type Step = FileData;

// 	const NAME: &'static str = "file_encryptor";

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

// 		let Library { key_manager, .. } = &*ctx.library;

// 		if !step.file_path.is_dir {
// 			// handle overwriting checks, and making sure there's enough available space

// 			let user_key = key_manager
// 				.access_keymount(state.init.key_uuid)
// 				.await?
// 				.hashed_key;

// 			let user_key_details = key_manager.access_keystore(state.init.key_uuid).await?;

// 			let output_path = {
// 				let mut path = step.full_path.clone();
// 				let extension = path.extension().map_or_else(
// 					|| Ok("bytes".to_string()),
// 					|extension| {
// 						Ok::<String, JobError>(format!(
// 							"{}{BYTES_EXT}",
// 							extension.to_str().ok_or(FileSystemJobsError::FilePath(
// 								NonUtf8PathError(step.full_path.clone().into_boxed_path()).into()
// 							))?
// 						))
// 					},
// 				)?;

// 				path.set_extension(extension);
// 				path
// 			};

// 			let _guard = ctx
// 				.library
// 				.location_manager()
// 				.temporary_ignore_events_for_path(
// 					state.init.location_id,
// 					ctx.library.clone(),
// 					&output_path,
// 				)
// 				.await
// 				.map_or_else(
// 					|e| {
// 						error!(
// 							"Failed to make location manager ignore the path {}; Error: {e:#?}",
// 							output_path.display()
// 						);
// 						None
// 					},
// 					Some,
// 				);

// 			let mut reader = File::open(&step.full_path)
// 				.await
// 				.map_err(|e| FileIOError::from((&step.full_path, e)))?;
// 			let mut writer = File::create(&output_path)
// 				.await
// 				.map_err(|e| FileIOError::from((output_path, e)))?;

// 			let master_key = Key::generate();

// 			let mut header = FileHeader::new(
// 				LATEST_FILE_HEADER,
// 				state.init.algorithm,
// 				vec![
// 					Keyslot::new(
// 						LATEST_KEYSLOT,
// 						state.init.algorithm,
// 						user_key_details.hashing_algorithm,
// 						user_key_details.content_salt,
// 						user_key,
// 						master_key.clone(),
// 					)
// 					.await?,
// 				],
// 			)?;

// 			if state.init.metadata || state.init.preview_media {
// 				// if any are requested, we can make the query as it'll be used at least once
// 				if let Some(ref object) = step.file_path.object {
// 					if state.init.metadata {
// 						let metadata = Metadata {
// 							file_path_id: step.file_path.id,
// 							name: step.file_path.materialized_path.clone(),
// 							hidden: object.hidden,
// 							favorite: object.favorite,
// 							important: object.important,
// 							note: object.note.clone(),
// 							date_created: object.date_created,
// 						};

// 						header
// 							.add_metadata(
// 								LATEST_METADATA,
// 								state.init.algorithm,
// 								master_key.clone(),
// 								&metadata,
// 							)
// 							.await?;
// 					}

// 					// if state.init.preview_media
// 					// 	&& (object.has_thumbnail
// 					// 		|| object.has_video_preview || object.has_thumbstrip)

// 					// may not be the best - preview media (thumbnail) isn't guaranteed to be webp
// 					let thumbnail_path = ctx
// 						.library
// 						.config()
// 						.data_directory()
// 						.join("thumbnails")
// 						.join(
// 							step.file_path
// 								.cas_id
// 								.as_ref()
// 								.ok_or(JobError::MissingCasId)?,
// 						)
// 						.with_extension("wepb");

// 					match fs::read(&thumbnail_path).await {
// 						Ok(thumbnail_bytes) => {
// 							header
// 								.add_preview_media(
// 									LATEST_PREVIEW_MEDIA,
// 									state.init.algorithm,
// 									master_key.clone(),
// 									&thumbnail_bytes,
// 								)
// 								.await?;
// 						}
// 						Err(e) if e.kind() == io::ErrorKind::NotFound => {
// 							// If the file just doesn't exist, then we don't care
// 						}
// 						Err(e) => {
// 							return Err(FileIOError::from((thumbnail_path, e)).into());
// 						}
// 					}
// 				} else {
// 					// should use container encryption if it's a directory
// 					warn!("skipping metadata/preview media inclusion, no associated object found")
// 				}
// 			}

// 			header.write(&mut writer).await?;

// 			let encryptor = Encryptor::new(master_key, header.nonce, header.algorithm)?;

// 			encryptor
// 				.encrypt_streams(&mut reader, &mut writer, &header.generate_aad())
// 				.await?;
// 		} else {
// 			warn!(
// 				"encryption is skipping {}/{} as it isn't a file",
// 				step.file_path.materialized_path, step.file_path.name
// 			)
// 		}

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
