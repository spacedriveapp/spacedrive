use crate::{
	api::utils::library,
	invalidate_query,
	job::Job,
	library::Library,
	location::{
		file_path_helper::{
			file_path_to_isolate, file_path_to_isolate_with_id, FilePathError, IsolatedFilePathData,
		},
		find_location, LocationError,
	},
	object::{
		fs::{
			copy::FileCopierJobInit, cut::FileCutterJobInit, delete::FileDeleterJobInit,
			erase::FileEraserJobInit, get_location_path_from_location_id,
		},
		media::{
			media_data_extractor::{
				can_extract_media_data_for_image, extract_media_data, MediaDataError,
			},
			media_data_image_from_prisma_data,
		},
	},
	prisma::{file_path, location, object},
	util::{db::maybe_missing, error::FileIOError},
};

use sd_file_ext::{extensions::ImageExtension, kind::ObjectKind};
use sd_images::ConvertableExtensions;
use sd_media_metadata::MediaMetadata;

use std::{
	ffi::OsString,
	path::{Path, PathBuf},
	str::FromStr,
};

use chrono::Utc;
use futures::future::join_all;
use regex::Regex;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tokio::{
	fs::{self, File},
	io::{self, AsyncWriteExt},
};
use tracing::{error, warn};

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			#[derive(Type, Deserialize)]
			pub struct GetArgs {
				pub id: i32,
			}
			R.with2(library())
				.query(|(_, library), args: GetArgs| async move {
					Ok(library
						.db
						.object()
						.find_unique(object::id::equals(args.id))
						.include(object::include!({ file_paths }))
						.exec()
						.await?)
				})
		})
		.procedure("getMediaData", {
			R.with2(library())
				.query(|(_, library), args: object::id::Type| async move {
					library
						.db
						.object()
						.find_unique(object::id::equals(args))
						.select(object::select!({ id kind media_data }))
						.exec()
						.await?
						.and_then(|obj| {
							Some(match obj.kind {
								Some(v) if v == ObjectKind::Image as i32 => {
									MediaMetadata::Image(Box::new(
										media_data_image_from_prisma_data(obj.media_data?).ok()?,
									))
								}
								_ => return None, // TODO(brxken128): audio and video
							})
						})
						.ok_or_else(|| {
							rspc::Error::new(ErrorCode::NotFound, "Object not found".to_string())
						})
				})
		})
		.procedure("getEphemeralMediaData", {
			R.query(|_, full_path: PathBuf| async move {
				let Some(extension) = full_path.extension().and_then(|ext| ext.to_str()) else {
					return Ok(None);
				};

				// TODO(fogodev): change this when we have media data for audio and videos
				let image_extension = ImageExtension::from_str(extension).map_err(|e| {
					error!("Failed to parse image extension: {e:#?}");
					rspc::Error::new(ErrorCode::BadRequest, "Invalid image extension".to_string())
				})?;

				if !can_extract_media_data_for_image(&image_extension) {
					return Ok(None);
				}

				match extract_media_data(full_path).await {
					Ok(img_media_data) => Ok(Some(MediaMetadata::Image(Box::new(img_media_data)))),
					Err(MediaDataError::MediaData(sd_media_metadata::Error::NoExifDataOnPath(
						_,
					))) => Ok(None),
					Err(e) => Err(rspc::Error::with_cause(
						ErrorCode::InternalServerError,
						"Failed to extract media data".to_string(),
						e,
					)),
				}
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
					let location_path = find_location(&library, location_id)
						.select(location::select!({ path }))
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(location_id))?
						.path
						.ok_or(LocationError::MissingPath(location_id))?;

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
					library
						.db
						.object()
						.update(
							object::id::equals(args.id),
							vec![object::note::set(args.note)],
						)
						.exec()
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
					library
						.db
						.object()
						.update(
							object::id::equals(args.id),
							vec![object::favorite::set(Some(args.favorite))],
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("updateAccessTime", {
			R.with2(library())
				.mutation(|(_, library), ids: Vec<i32>| async move {
					library
						.db
						.object()
						.update_many(
							vec![object::id::in_vec(ids)],
							vec![object::date_accessed::set(Some(Utc::now().into()))],
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
					Ok(())
				})
		})
		.procedure("removeAccessTime", {
			R.with2(library())
				.mutation(|(_, library), object_ids: Vec<i32>| async move {
					library
						.db
						.object()
						.update_many(
							vec![object::id::in_vec(object_ids)],
							vec![object::date_accessed::set(None)],
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
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
				.mutation(|(node, library), args: FileDeleterJobInit| async move {
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

							let location_path = maybe_missing(
								maybe_location
									.ok_or(LocationError::IdNotFound(args.location_id))?
									.path,
								"location.path",
							)
							.map_err(LocationError::from)?;

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
										"File not found in the file system, will remove from database: {}",
										full_path.display()
									);
									library
										.db
										.file_path()
										.delete(file_path::id::equals(args.file_path_ids[0]))
										.exec()
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
						_ => Job::new(args)
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
				desired_extension: ConvertableExtensions,
				quality_percentage: Option<i32>, // 1% - 125%
			}
			R.with2(library())
				.mutation(|(_, library), args: ConvertImageArgs| async move {
					let location_path =
						get_location_path_from_location_id(&library.db, args.location_id)
							.await
							.map_err(|_| LocationError::IdNotFound(args.location_id))?;

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
					)
					.map_err(LocationError::MissingField)?;

					let path = Path::new(&location_path).join(&isolated_path);

					if fs::metadata(&path).await.is_err() {
						return Err(LocationError::PathNotFound(path.to_path_buf()))?;
					}

					args.quality_percentage.map(|x| x.clamp(1, 125));

					let output_extension = OsString::from(args.desired_extension.to_string());
					let mut image = sd_images::convert_image(&path, &output_extension)?;

					if let Some(quality_percentage) = args.quality_percentage {
						image = image.resize(
							image.width() * (quality_percentage as f32 / 100_f32) as u32,
							image.height() * (quality_percentage as f32 / 100_f32) as u32,
							image::imageops::FilterType::Triangle,
						);
					}

					let mut output_path = path.clone();
					if output_path.set_extension(output_extension) {
						return Err(rspc::Error::new(
							ErrorCode::InternalServerError,
							"There was an error while updating the extension".to_string(),
						));
					}

					if fs::metadata(&output_path).await.is_ok() {
						return Err(rspc::Error::new(
							ErrorCode::InternalServerError,
							"The output path already exists and this would overwrite it"
								.to_string(),
						));
					} else if let Ok(mut file) = File::create(&output_path).await {
						file.write_all(image.as_bytes()).await.map_err(|e| {
							rspc::Error::with_cause(
								ErrorCode::InternalServerError,
								"There was an error while writing the image to the output path"
									.to_string(),
								e,
							)
						})?;
					}

					if args.delete_src {
						fs::remove_file(&path).await.map_err(|e| {
							rspc::Error::with_cause(
								ErrorCode::InternalServerError,
								"There was an error while deleting the source image".to_string(),
								e,
							)
						})?;
					}

					invalidate_query!(library, "search.paths");

					Ok(())
				})
		})
		.procedure("getConvertableImageExtensions", {
			R.with2(library()).query(|(_, _library), _: ()| async move {
				Ok(sd_images::all_compatible_extensions())
			})
		})
		.procedure("eraseFiles", {
			R.with2(library())
				.mutation(|(node, library), args: FileEraserJobInit| async move {
					Job::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("duplicateFiles", {
			R.with2(library())
				.mutation(|(node, library), args: FileCopierJobInit| async move {
					Job::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("copyFiles", {
			R.with2(library())
				.mutation(|(node, library), args: FileCopierJobInit| async move {
					Job::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("cutFiles", {
			R.with2(library())
				.mutation(|(node, library), args: FileCutterJobInit| async move {
					Job::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("renameFile", {
			#[derive(Type, Deserialize)]
			pub struct FromPattern {
				pub pattern: String,
				pub replace_all: bool,
			}

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
								"File already exists".to_string(),
							))
						}
						Err(e) => {
							if e.kind() != std::io::ErrorKind::NotFound {
								return Err(rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to check if file exists".to_string(),
									e,
								));
							}
						}
					}

					fs::rename(location_path.join(&iso_file_path), new_file_full_path)
						.await
						.map_err(|e| {
							rspc::Error::with_cause(
								ErrorCode::Conflict,
								"Failed to rename file".to_string(),
								e,
							)
						})?;

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
													"Failed to rename file from: '{}' to: '{}'; Error: {e:#?}",
													from.display(),
													to.display()
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

			R.with2(library())
				.mutation(|(_, library), args: RenameFileArgs| async move {
					let location_path = find_location(&library, args.location_id)
						.select(location::select!({ path }))
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?
						.path
						.ok_or(LocationError::MissingPath(args.location_id))?;

					let res = match args.kind {
						RenameKind::One(one) => {
							RenameFileArgs::rename_one(one, location_path, &library).await
						}
						RenameKind::Many(many) => {
							RenameFileArgs::rename_many(many, location_path, &library).await
						}
					};

					invalidate_query!(library, "search.objects");

					res
				})
		})
}
