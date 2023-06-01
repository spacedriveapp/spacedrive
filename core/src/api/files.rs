use crate::{
	api::utils::library,
	invalidate_query,
	location::{file_path_helper::IsolatedFilePathData, find_location, LocationError},
	object::fs::{
		copy::FileCopierJobInit, cut::FileCutterJobInit, decrypt::FileDecryptorJobInit,
		delete::FileDeleterJobInit, encrypt::FileEncryptorJobInit, erase::FileEraserJobInit,
	},
	prisma::{location, object},
};

use chrono::Utc;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use std::path::Path;
use tokio::fs;

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
							vec![object::favorite::set(args.favorite)],
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), id: i32| async move {
					library
						.db
						.object()
						.delete(object::id::equals(id))
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
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
				.mutation(|(_, library), id: i32| async move {
					library
						.db
						.object()
						.update(
							object::id::equals(id),
							vec![object::date_accessed::set(None)],
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.paths");
					Ok(())
				})
		})
		.procedure("encryptFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileEncryptorJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("decryptFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileDecryptorJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("deleteFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileDeleterJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("eraseFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileEraserJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("duplicateFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("copyFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("cutFiles", {
			R.with2(library())
				.mutation(|(_, library), args: FileCutterJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("renameFile", {
			#[derive(Type, Deserialize)]
			pub struct RenameFileArgs {
				pub location_id: i32,
				pub file_name: String,
				pub new_file_name: String,
			}

			R.with2(library()).mutation(
				|(_, library),
				 RenameFileArgs {
				     location_id,
				     file_name,
				     new_file_name,
				 }: RenameFileArgs| async move {
					let location = find_location(&library, location_id)
						.select(location::select!({ path }))
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(location_id))?;

					let location_path = Path::new(&location.path);
					fs::rename(
						location_path.join(IsolatedFilePathData::from_relative_str(
							location_id,
							&file_name,
						)),
						location_path.join(IsolatedFilePathData::from_relative_str(
							location_id,
							&new_file_name,
						)),
					)
					.await
					.map_err(|e| {
						rspc::Error::with_cause(
							ErrorCode::Conflict,
							"Failed to rename file".to_string(),
							e,
						)
					})?;

					invalidate_query!(library, "search.objects");

					Ok(())
				},
			)
		})
}
