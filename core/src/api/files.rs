use crate::{
	api::utils::library,
	invalidate_query,
	library::Library,
	location::{get_location_path_from_location_id, LocationError},
	object::{
		fs::{
			error::FileSystemJobsError, find_available_filename_for_duplicate,
			old_copy::OldFileCopierJobInit, old_cut::OldFileCutterJobInit,
			old_delete::OldFileDeleterJobInit, old_erase::OldFileEraserJobInit,
		},
		// media::{exif_media_data_from_prisma_data, ffmpeg_data_from_prisma_data},
	},
	old_job::OldJob,
};

use sd_core_file_path_helper::{FilePathError, IsolatedFilePathData};
use sd_core_heavy_lifting::media_processor::{exif_media_data, ffmpeg_media_data};
use sd_core_prisma_helpers::{
	file_path_to_isolate, file_path_to_isolate_with_id, object_with_file_paths,
	object_with_media_data,
};

use sd_file_ext::kind::ObjectKind;
use sd_images::ConvertibleExtension;
use sd_media_metadata::{ExifMetadata, FFmpegMetadata};
use sd_prisma::{
	prisma::{file_path, location, object},
	prisma_sync,
};
use sd_sync::{sync_db_entry, sync_db_nullable_entry, sync_entry, OperationFactory};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{
	ffi::OsString,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, FixedOffset, Utc};
use futures::future::join_all;
use regex::Regex;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs, io, task::spawn_blocking};
use tracing::{error, warn};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use trash;

use super::{Ctx, R};

const UNTITLED_FOLDER_STR: &str = "Untitled Folder";
const UNTITLED_FILE_STR: &str = "Untitled";
const UNTITLED_TEXT_FILE_STR: &str = "Untitled.txt";

#[derive(Type, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FileCreateContextTypes {
	Empty,
	Text,
}

#[derive(Serialize, Type)]
pub(crate) enum MediaData {
	Exif(ExifMetadata),
	FFmpeg(FFmpegMetadata),
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			#[derive(Type, Serialize)]
			pub struct ObjectWithFilePaths2 {
				pub id: i32,
				pub pub_id: Vec<u8>,
				pub kind: Option<i32>,
				pub key_id: Option<i32>,
				pub hidden: Option<bool>,
				pub favorite: Option<bool>,
				pub important: Option<bool>,
				pub note: Option<String>,
				pub date_created: Option<DateTime<FixedOffset>>,
				pub date_accessed: Option<DateTime<FixedOffset>>,
				pub file_paths: Vec<object_with_file_paths::file_paths::Data>,
			}

			impl ObjectWithFilePaths2 {
				pub fn from_db(item: object_with_file_paths::Data) -> Self {
					Self {
						id: item.id,
						pub_id: item.pub_id,
						kind: item.kind,
						key_id: item.key_id,
						hidden: item.hidden,
						favorite: item.favorite,
						important: item.important,
						note: item.note,
						date_created: item.date_created,
						date_accessed: item.date_accessed,
						file_paths: item.file_paths,
					}
				}
			}

			R.with2(library())
				.query(|(_, library), object_id: i32| async move {
					Ok(library
						.db
						.object()
						.find_unique(object::id::equals(object_id))
						.include(object_with_file_paths::include())
						.exec()
						.await?
						.map(ObjectWithFilePaths2::from_db))
				})
		})
		.procedure("getMediaData", {
			R.with2(library())
				.query(|(_, library), args: object::id::Type| async move {
					library
						.db
						.object()
						.find_unique(object::id::equals(args))
						.include(object_with_media_data::include())
						.exec()
						.await?
						.and_then(|obj| {
							Some(match obj.kind {
								Some(v) if v == ObjectKind::Image as i32 => MediaData::Exif(
									exif_media_data::from_prisma_data(obj.exif_data?),
								),
								Some(v)
									if v == ObjectKind::Audio as i32
										|| v == ObjectKind::Video as i32 =>
								{
									MediaData::FFmpeg(ffmpeg_media_data::from_prisma_data(
										obj.ffmpeg_data?,
									))
								}
								_ => return None, // No media data
							})
						})
						.ok_or_else(|| {
							rspc::Error::new(ErrorCode::NotFound, "Object not found".to_string())
						})
				})
		})
		.procedure("getPath", {
			R.with2(library())
				.query(|(_, library), id: i32| async move {
					let isolated_path = IsolatedFilePathData::try_from(
						library
							.db
							.file_path()
							.find_unique(file_path::id::equals(id))
							.select(file_path_to_isolate::select())
							.exec()
							.await?
							.ok_or(LocationError::FilePath(FilePathError::IdNotFound(id)))?,
					)
					.map_err(LocationError::MissingField)?;

					let location_id = isolated_path.location_id();
					let location_path =
						get_location_path_from_location_id(&library.db, location_id).await?;

					Ok(Path::new(&location_path)
						.join(&isolated_path)
						.to_str()
						.map(|str| str.to_string()))
				})
		})
		.procedure("setNote", {
			#[derive(Type, Deserialize)]
			pub struct SetNoteArgs {
				pub id: i32,
				pub note: Option<String>,
			}

			R.with2(library())
				.mutation(|(_, library), args: SetNoteArgs| async move {
					let Library { db, sync, .. } = library.as_ref();

					let object = db
						.object()
						.find_unique(object::id::equals(args.id))
						.select(object::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(
								rspc::ErrorCode::NotFound,
								"Object not found".to_string(),
							)
						})?;

					let (sync_param, db_param) = sync_db_nullable_entry!(args.note, object::note);

					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::object::SyncId {
								pub_id: object.pub_id,
							},
							[sync_param],
						),
						db.object()
							.update(object::id::equals(args.id), vec![db_param])
							.select(object::select!({ id })),
					)
					.await?;

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("setFavorite", {
			#[derive(Type, Deserialize)]
			pub struct SetFavoriteArgs {
				pub id: i32,
				pub favorite: bool,
			}

			R.with2(library())
				.mutation(|(_, library), args: SetFavoriteArgs| async move {
					let Library { sync, db, .. } = library.as_ref();

					let object = db
						.object()
						.find_unique(object::id::equals(args.id))
						.select(object::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(
								rspc::ErrorCode::NotFound,
								"Object not found".to_string(),
							)
						})?;

					let (sync_param, db_param) = sync_db_entry!(args.favorite, object::favorite);

					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::object::SyncId {
								pub_id: object.pub_id,
							},
							[sync_param],
						),
						db.object()
							.update(object::id::equals(args.id), vec![db_param])
							.select(object::select!({ id })),
					)
					.await?;

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("createFolder", {
			#[derive(Type, Deserialize)]
			pub struct CreateFolderArgs {
				pub location_id: location::id::Type,
				pub sub_path: Option<PathBuf>,
				pub name: Option<String>,
			}
			R.with2(library()).mutation(
				|(_, library),
				 CreateFolderArgs {
				     location_id,
				     sub_path,
				     name,
				 }: CreateFolderArgs| async move {
					let mut path =
						get_location_path_from_location_id(&library.db, location_id).await?;

					if let Some(sub_path) = sub_path
						.as_ref()
						.and_then(|sub_path| sub_path.strip_prefix("/").ok())
					{
						path.push(sub_path);
					}

					path.push(name.as_deref().unwrap_or(UNTITLED_FOLDER_STR));

					create_directory(path, &library).await
				},
			)
		})
		.procedure("createFile", {
			#[derive(Type, Deserialize)]
			pub struct CreateFileArgs {
				pub location_id: location::id::Type,
				pub sub_path: Option<PathBuf>,
				pub name: Option<String>,
				pub context: FileCreateContextTypes,
			}
			R.with2(library()).mutation(
				|(_, library),
				 CreateFileArgs {
				     location_id,
				     sub_path,
				     context,
				     name,
				 }: CreateFileArgs| async move {
					let mut path =
						get_location_path_from_location_id(&library.db, location_id).await?;

					if let Some(sub_path) = sub_path
						.as_ref()
						.and_then(|sub_path| sub_path.strip_prefix("/").ok())
					{
						path.push(sub_path);
					}

					match context {
						FileCreateContextTypes::Empty => {
							path.push(name.as_deref().unwrap_or(UNTITLED_FILE_STR))
						}
						FileCreateContextTypes::Text => {
							path.push(name.as_deref().unwrap_or(UNTITLED_TEXT_FILE_STR))
						}
					}

					create_file(path, &library).await
				},
			)
		})
		.procedure("updateAccessTime", {
			R.with2(library())
				.mutation(|(_, library), ids: Vec<i32>| async move {
					let Library { sync, db, .. } = library.as_ref();

					let objects = db
						.object()
						.find_many(vec![object::id::in_vec(ids)])
						.select(object::select!({ id pub_id }))
						.exec()
						.await?;

					let date_accessed = Utc::now().into();

					let (ops, object_ids) = objects
						.into_iter()
						.map(|object| {
							(
								sync.shared_update(
									prisma_sync::object::SyncId {
										pub_id: object.pub_id,
									},
									[sync_entry!(date_accessed, object::date_accessed)],
								),
								object.id,
							)
						})
						.unzip::<_, _, Vec<_>, Vec<_>>();

					if !ops.is_empty() && !object_ids.is_empty() {
						sync.write_ops(
							db,
							(
								ops,
								db.object().update_many(
									vec![object::id::in_vec(object_ids)],
									vec![object::date_accessed::set(Some(date_accessed))],
								),
							),
						)
						.await?;

						invalidate_query!(library, "search.paths");
						invalidate_query!(library, "search.objects");
					}

					Ok(())
				})
		})
		.procedure("removeAccessTime", {
			R.with2(library())
				.mutation(|(_, library), object_ids: Vec<i32>| async move {
					let Library { db, sync, .. } = library.as_ref();

					let objects = db
						.object()
						.find_many(vec![object::id::in_vec(object_ids)])
						.select(object::select!({ id pub_id }))
						.exec()
						.await?;

					let (ops, object_ids) = objects
						.into_iter()
						.map(|object| {
							(
								sync.shared_update(
									prisma_sync::object::SyncId {
										pub_id: object.pub_id,
									},
									[sync_entry!(nil, object::date_accessed)],
								),
								object.id,
							)
						})
						.unzip::<_, _, Vec<_>, Vec<_>>();

					if !ops.is_empty() && !object_ids.is_empty() {
						sync.write_ops(
							db,
							(
								ops,
								db.object().update_many(
									vec![object::id::in_vec(object_ids)],
									vec![object::date_accessed::set(None)],
								),
							),
						)
						.await?;

						invalidate_query!(library, "search.objects");
						invalidate_query!(library, "search.paths");
					}

					Ok(())
				})
		})
		// .procedure("encryptFiles", {
		// 	R.with2(library())
		// 		.mutation(|(node, library), args: FileEncryptorJobInit| async move {
		// 			Job::new(args).spawn(&node, &library).await.map_err(Into::into)
		// 		})
		// })
		// .procedure("decryptFiles", {
		// 	R.with2(library())
		// 		.mutation(|(node, library), args: FileDecryptorJobInit| async move {
		// 			Job::new(args).spawn(&node, &library).await.map_err(Into::into)
		// 		})
		// })
		.procedure("deleteFiles", {
			R.with2(library())
				.mutation(|(node, library), args: OldFileDeleterJobInit| async move {
					match args.file_path_ids.len() {
						0 => Ok(()),
						1 => {
							let (maybe_location, maybe_file_path) = library
								.db
								._batch((
									library
										.db
										.location()
										.find_unique(location::id::equals(args.location_id))
										.select(location::select!({ path })),
									library
										.db
										.file_path()
										.find_unique(file_path::id::equals(args.file_path_ids[0]))
										.select(file_path_to_isolate::select()),
								))
								.await?;

							let location_path = maybe_location
								.ok_or(LocationError::IdNotFound(args.location_id))?
								.path
								.ok_or(LocationError::MissingPath(args.location_id))?;

							let file_path = maybe_file_path.ok_or(LocationError::FilePath(
								FilePathError::IdNotFound(args.file_path_ids[0]),
							))?;

							let full_path = Path::new(&location_path).join(
								IsolatedFilePathData::try_from(&file_path)
									.map_err(LocationError::MissingField)?,
							);

							match if maybe_missing(file_path.is_dir, "file_path.is_dir")
								.map_err(LocationError::MissingField)?
							{
								fs::remove_dir_all(&full_path).await
							} else {
								fs::remove_file(&full_path).await
							} {
								Ok(()) => Ok(()),
								Err(e) if e.kind() == io::ErrorKind::NotFound => {
									warn!(
										path = %full_path.display(),
										"File not found in the file system, will remove from database;",
									);

									let file_path_pub_id = library
										.db
										.file_path()
										.find_unique(file_path::id::equals(args.file_path_ids[0]))
										.select(file_path::select!({ pub_id }))
										.exec()
										.await?
										.ok_or(LocationError::FilePath(FilePathError::IdNotFound(
											args.file_path_ids[0],
										)))?
										.pub_id;

									library
										.sync
										.write_op(
											&library.db,
											library.sync.shared_delete(
												prisma_sync::file_path::SyncId {
													pub_id: file_path_pub_id,
												},
											),
											library.db.file_path().delete(file_path::id::equals(
												args.file_path_ids[0],
											)),
										)
										.await
										.map_err(LocationError::from)?;

									Ok(())
								}
								Err(e) => {
									Err(LocationError::from(FileIOError::from((full_path, e)))
										.into())
								}
							}
						}
						_ => OldJob::new(args)
							.spawn(&node, &library)
							.await
							.map_err(Into::into),
					}
				})
		})
		.procedure("moveToTrash", {
			R.with2(library())
				.mutation(|(node, library), args: OldFileDeleterJobInit| async move {
					if cfg!(target_os = "ios") || cfg!(target_os = "android") {
						return Err(rspc::Error::new(
							ErrorCode::MethodNotSupported,
							"Moving to trash is not supported on this platform".to_string(),
						));
					}

					match args.file_path_ids.len() {
						0 => Ok(()),
						1 => {
							let (maybe_location, maybe_file_path) = library
								.db
								._batch((
									library
										.db
										.location()
										.find_unique(location::id::equals(args.location_id))
										.select(location::select!({ path })),
									library
										.db
										.file_path()
										.find_unique(file_path::id::equals(args.file_path_ids[0]))
										.select(file_path_to_isolate::select()),
								))
								.await?;

							let location_path = maybe_location
								.ok_or(LocationError::IdNotFound(args.location_id))?
								.path
								.ok_or(LocationError::MissingPath(args.location_id))?;

							let file_path = maybe_file_path.ok_or(LocationError::FilePath(
								FilePathError::IdNotFound(args.file_path_ids[0]),
							))?;

							let full_path = Path::new(&location_path).join(
								IsolatedFilePathData::try_from(&file_path)
									.map_err(LocationError::MissingField)?,
							);

							#[cfg(not(any(target_os = "ios", target_os = "android")))]
							trash::delete(&full_path).map_err(|e| {
								FileIOError::from((
									full_path,
									match e {
										#[cfg(all(unix, not(target_os = "macos")))]
										trash::Error::FileSystem { path: _, source: e } => e,
										_ => io::Error::other(e),
									},
									"Failed to delete file",
								))
							})?;

							Ok(())
						}
						_ => OldJob::new(args)
							.spawn(&node, &library)
							.await
							.map_err(Into::into),
					}
				})
		})
		.procedure("convertImage", {
			#[derive(Type, Deserialize)]
			struct ConvertImageArgs {
				location_id: location::id::Type,
				file_path_id: file_path::id::Type,
				delete_src: bool, // if set, we delete the src image after
				desired_extension: ConvertibleExtension,
				quality_percentage: Option<i32>, // 1% - 125%
			}
			R.with2(library())
				.mutation(|(_, library), args: ConvertImageArgs| async move {
					// TODO:(fogodev) I think this will have to be a Job due to possibly being too much CPU Bound for rspc

					let location_path =
						get_location_path_from_location_id(&library.db, args.location_id).await?;

					let isolated_path = IsolatedFilePathData::try_from(
						library
							.db
							.file_path()
							.find_unique(file_path::id::equals(args.file_path_id))
							.select(file_path_to_isolate::select())
							.exec()
							.await?
							.ok_or(LocationError::FilePath(FilePathError::IdNotFound(
								args.file_path_id,
							)))?,
					)?;

					let path = Path::new(&location_path).join(&isolated_path);

					if let Err(e) = fs::metadata(&path).await {
						if e.kind() == io::ErrorKind::NotFound {
							return Err(LocationError::FilePath(FilePathError::NotFound(
								path.into_boxed_path(),
							))
							.into());
						} else {
							return Err(FileIOError::from((
								path,
								e,
								"Got an error trying to read metadata from image to convert",
							))
							.into());
						}
					}

					args.quality_percentage.map(|x| x.clamp(1, 125));

					let path = Arc::new(path);

					let output_extension =
						Arc::new(OsString::from(args.desired_extension.to_string()));

					// TODO(fogodev): Refactor this if Rust get async scoped spawns someday
					let inner_path = Arc::clone(&path);
					let inner_output_extension = Arc::clone(&output_extension);
					let image = spawn_blocking(move || {
						sd_images::convert_image(inner_path.as_ref(), &inner_output_extension).map(
							|mut image| {
								if let Some(quality_percentage) = args.quality_percentage {
									image = image.resize(
										image.width()
											* (quality_percentage as f32 / 100_f32) as u32,
										image.height()
											* (quality_percentage as f32 / 100_f32) as u32,
										image::imageops::FilterType::Triangle,
									);
								}
								image
							},
						)
					})
					.await
					.map_err(|e| {
						error!(?e, "Failed to convert image;");
						rspc::Error::with_cause(
							ErrorCode::InternalServerError,
							"Had an internal problem converting image".to_string(),
							e,
						)
					})??;

					let output_path = path.with_extension(output_extension.as_ref());

					if fs::metadata(&output_path)
						.await
						.map(|_| true)
						.map_err(|e| {
							FileIOError::from(
							(
								&output_path,
								e,
								"Got an error trying to check if the desired converted file already exists"
							)
						)
						})? {
						return Err(rspc::Error::new(
							ErrorCode::Conflict,
							"There is already a file with same name and extension in this directory"
								.to_string(),
						));
					} else {
						fs::write(&output_path, image.as_bytes())
							.await
							.map_err(|e| {
								FileIOError::from((
									output_path,
									e,
									"There was an error while writing the image to the output path",
								))
							})?;
					}

					if args.delete_src {
						fs::remove_file(path.as_ref()).await.map_err(|e| {
							// Let's also invalidate the query here, because we succeeded in converting the file
							invalidate_query!(library, "search.paths");
							invalidate_query!(library, "search.objects");

							FileIOError::from((
								path.as_ref(),
								e,
								"There was an error while deleting the source image",
							))
						})?;
					}

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("getConvertibleImageExtensions", {
			R.query(|_, _: ()| async move { Ok(sd_images::all_compatible_extensions()) })
		})
		.procedure("eraseFiles", {
			R.with2(library())
				.mutation(|(node, library), args: OldFileEraserJobInit| async move {
					OldJob::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("copyFiles", {
			R.with2(library())
				.mutation(|(node, library), args: OldFileCopierJobInit| async move {
					OldJob::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("cutFiles", {
			R.with2(library())
				.mutation(|(node, library), args: OldFileCutterJobInit| async move {
					OldJob::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("renameFile", {
			#[derive(Type, Deserialize)]
			pub struct RenameOne {
				pub from_file_path_id: file_path::id::Type,
				pub to: String,
			}

			#[derive(Type, Deserialize)]
			pub struct RenameMany {
				pub from_pattern: FromPattern,
				pub to_pattern: String,
				pub from_file_path_ids: Vec<file_path::id::Type>,
			}

			#[derive(Type, Deserialize)]
			pub enum RenameKind {
				One(RenameOne),
				Many(RenameMany),
			}

			#[derive(Type, Deserialize)]
			pub struct RenameFileArgs {
				pub location_id: location::id::Type,
				pub kind: RenameKind,
			}

			impl RenameFileArgs {
				pub async fn rename_one(
					RenameOne {
						from_file_path_id,
						to,
					}: RenameOne,
					location_path: impl AsRef<Path>,
					library: &Library,
				) -> Result<(), rspc::Error> {
					let location_path = location_path.as_ref();
					let iso_file_path = IsolatedFilePathData::try_from(
						library
							.db
							.file_path()
							.find_unique(file_path::id::equals(from_file_path_id))
							.select(file_path_to_isolate::select())
							.exec()
							.await?
							.ok_or(LocationError::FilePath(FilePathError::IdNotFound(
								from_file_path_id,
							)))?,
					)
					.map_err(LocationError::MissingField)?;

					if iso_file_path.full_name() == to {
						return Ok(());
					}

					let (new_file_name, new_extension) =
						IsolatedFilePathData::separate_name_and_extension_from_str(&to)
							.map_err(LocationError::FilePath)?;

					let mut new_file_full_path = location_path.join(iso_file_path.parent());
					if !new_extension.is_empty() {
						new_file_full_path.push(format!("{}.{}", new_file_name, new_extension));
					} else {
						new_file_full_path.push(new_file_name);
					}

					match fs::metadata(&new_file_full_path).await {
						Ok(_) => {
							return Err(rspc::Error::new(
								ErrorCode::Conflict,
								"Renaming would overwrite a file".to_string(),
							));
						}

						Err(e) => {
							if e.kind() != std::io::ErrorKind::NotFound {
								return Err(rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to check if file exists".to_string(),
									e,
								));
							}

							fs::rename(location_path.join(&iso_file_path), new_file_full_path)
								.await
								.map_err(|e| {
									rspc::Error::with_cause(
										ErrorCode::InternalServerError,
										"Failed to rename file".to_string(),
										e,
									)
								})?;
						}
					}

					Ok(())
				}

				pub async fn rename_many(
					RenameMany {
						from_pattern,
						to_pattern,
						from_file_path_ids,
					}: RenameMany,
					location_path: impl AsRef<Path>,
					library: &Library,
				) -> Result<(), rspc::Error> {
					let location_path = location_path.as_ref();

					let Ok(from_regex) = Regex::new(&from_pattern.pattern) else {
						return Err(rspc::Error::new(
							rspc::ErrorCode::BadRequest,
							"Invalid `from` regex pattern".into(),
						));
					};

					let errors = join_all(
						library
							.db
							.file_path()
							.find_many(vec![file_path::id::in_vec(from_file_path_ids)])
							.select(file_path_to_isolate_with_id::select())
							.exec()
							.await?
							.into_iter()
							.flat_map(IsolatedFilePathData::try_from)
							.map(|iso_file_path| {
								let from = location_path.join(&iso_file_path);
								let mut to = location_path.join(iso_file_path.parent());
								let full_name = iso_file_path.full_name();
								let replaced_full_name = if from_pattern.replace_all {
									from_regex.replace_all(&full_name, &to_pattern)
								} else {
									from_regex.replace(&full_name, &to_pattern)
								}
								.to_string();

								to.push(&replaced_full_name);

								async move {
									if !IsolatedFilePathData::accept_file_name(&replaced_full_name)
									{
										Err(rspc::Error::new(
											ErrorCode::BadRequest,
											"Invalid file name".to_string(),
										))
									} else {
										fs::rename(&from, &to).await.map_err(|e| {
											error!(
												from = %from.display(),
												to = %to.display(),
												?e,
												"Failed to rename file;",
											);
											rspc::Error::with_cause(
												ErrorCode::Conflict,
												"Failed to rename file".to_string(),
												e,
											)
										})
									}
								}
							}),
					)
					.await
					.into_iter()
					.filter_map(Result::err)
					.collect::<Vec<_>>();

					if !errors.is_empty() {
						return Err(rspc::Error::new(
							rspc::ErrorCode::Conflict,
							errors
								.into_iter()
								.map(|e| e.to_string())
								.collect::<Vec<_>>()
								.join("\n"),
						));
					}

					Ok(())
				}
			}

			R.with2(library()).mutation(
				|(_, library), RenameFileArgs { location_id, kind }: RenameFileArgs| async move {
					let location_path =
						get_location_path_from_location_id(&library.db, location_id).await?;

					let res = match kind {
						RenameKind::One(one) => {
							RenameFileArgs::rename_one(one, location_path, &library).await
						}
						RenameKind::Many(many) => {
							RenameFileArgs::rename_many(many, location_path, &library).await
						}
					};

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					res
				},
			)
		})
}

pub(super) async fn create_directory(
	mut target_path: PathBuf,
	library: &Library,
) -> Result<String, rspc::Error> {
	match fs::metadata(&target_path).await {
		Ok(metadata) if metadata.is_dir() => {
			target_path = find_available_filename_for_duplicate(&target_path).await?;
		}
		Ok(_) => {
			return Err(FileSystemJobsError::WouldOverwrite(target_path.into_boxed_path()).into())
		}
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			// Everything is awesome!
		}
		Err(e) => {
			return Err(FileIOError::from((
				target_path,
				e,
				"Failed to access file system and get metadata on directory to be created",
			))
			.into())
		}
	};

	fs::create_dir(&target_path)
		.await
		.map_err(|e| FileIOError::from((&target_path, e, "Failed to create directory")))?;

	invalidate_query!(library, "search.objects");
	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.ephemeralPaths");

	Ok(target_path
		.file_name()
		.expect("Failed to get file name")
		.to_string_lossy()
		.to_string())
}

pub(super) async fn create_file(
	mut target_path: PathBuf,
	library: &Library,
) -> Result<String, rspc::Error> {
	match fs::metadata(&target_path).await {
		Ok(metadata) if metadata.is_file() => {
			target_path = find_available_filename_for_duplicate(&target_path).await?;
		}
		Ok(_) => {
			return Err(FileSystemJobsError::WouldOverwrite(target_path.into_boxed_path()).into())
		}
		Err(e) if e.kind() == io::ErrorKind::NotFound => {
			// Everything is awesome!
		}
		Err(e) => {
			return Err(FileIOError::from((
				target_path,
				e,
				"Failed to access file system and get metadata on file to be created",
			))
			.into())
		}
	};

	fs::File::create(&target_path)
		.await
		.map_err(|e| FileIOError::from((&target_path, e, "Failed to create file")))?;

	invalidate_query!(library, "search.objects");
	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.ephemeralPaths");

	Ok(target_path
		.file_name()
		.expect("Failed to get file name")
		.to_string_lossy()
		.to_string())
}

#[derive(Type, Deserialize)]
pub struct FromPattern {
	pub pattern: String,
	pub replace_all: bool,
}
