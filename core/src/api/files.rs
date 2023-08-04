use crate::{
	api::utils::library,
	invalidate_query,
	job::Job,
	library::LoadedLibrary,
	location::{
		file_path_helper::{
			file_path_to_isolate, file_path_to_isolate_with_id, FilePathError, IsolatedFilePathData,
		},
		find_location, LocationError,
	},
	object::fs::{
		copy::FileCopierJobInit, cut::FileCutterJobInit, delete::FileDeleterJobInit,
		erase::FileEraserJobInit,
	},
	prisma::{file_path, location, object},
};

use std::path::Path;

use chrono::Utc;
use futures::future::join_all;
use regex::Regex;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tokio::fs;
use tracing::error;

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
						.include(object::include!({ file_paths media_data }))
						.exec()
						.await?)
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
				.mutation(|(_, library), id: i32| async move {
					library
						.db
						.object()
						.update(
							object::id::equals(id),
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
					Job::new(args)
						.spawn(&node, &library)
						.await
						.map_err(Into::into)
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
					library: &LoadedLibrary,
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
					library: &LoadedLibrary,
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
