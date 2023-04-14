use crate::{
	invalidate_query,
	library::Library,
	location::{find_location, LocationError},
	object::fs::{
		copy::FileCopierJobInit, cut::FileCutterJobInit, decrypt::FileDecryptorJobInit,
		delete::FileDeleterJobInit, encrypt::FileEncryptorJobInit, erase::FileEraserJobInit,
	},
	prisma::{location, object},
};

use rspc::{ErrorCode, Type};
use serde::Deserialize;
use std::path::Path;
use tokio::fs;

use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_query("get", |t| {
			#[derive(Type, Deserialize)]
			pub struct GetArgs {
				pub id: i32,
			}
			t(|_, args: GetArgs, library: Library| async move {
				Ok(library
					.db
					.object()
					.find_unique(object::id::equals(args.id))
					.include(object::include!({ file_paths media_data }))
					.exec()
					.await?)
			})
		})
		.library_mutation("setNote", |t| {
			#[derive(Type, Deserialize)]
			pub struct SetNoteArgs {
				pub id: i32,
				pub note: Option<String>,
			}

			t(|_, args: SetNoteArgs, library: Library| async move {
				library
					.db
					.object()
					.update(
						object::id::equals(args.id),
						vec![object::note::set(args.note)],
					)
					.exec()
					.await?;

				invalidate_query!(library, "locations.getExplorerData");
				invalidate_query!(library, "tags.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("setFavorite", |t| {
			#[derive(Type, Deserialize)]
			pub struct SetFavoriteArgs {
				pub id: i32,
				pub favorite: bool,
			}

			t(|_, args: SetFavoriteArgs, library: Library| async move {
				library
					.db
					.object()
					.update(
						object::id::equals(args.id),
						vec![object::favorite::set(args.favorite)],
					)
					.exec()
					.await?;

				invalidate_query!(library, "locations.getExplorerData");
				invalidate_query!(library, "tags.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("delete", |t| {
			t(|_, id: i32, library: Library| async move {
				library
					.db
					.object()
					.delete(object::id::equals(id))
					.exec()
					.await?;

				invalidate_query!(library, "locations.getExplorerData");
				Ok(())
			})
		})
		.library_mutation("encryptFiles", |t| {
			t(
				|_, args: FileEncryptorJobInit, library: Library| async move {
					library.spawn_job(args).await;
					Ok(())
				},
			)
		})
		.library_mutation("decryptFiles", |t| {
			t(
				|_, args: FileDecryptorJobInit, library: Library| async move {
					library.spawn_job(args).await;
					Ok(())
				},
			)
		})
		.library_mutation("deleteFiles", |t| {
			t(|_, args: FileDeleterJobInit, library: Library| async move {
				library.spawn_job(args).await;
				Ok(())
			})
		})
		.library_mutation("eraseFiles", |t| {
			t(|_, args: FileEraserJobInit, library: Library| async move {
				library.spawn_job(args).await;
				Ok(())
			})
		})
		.library_mutation("duplicateFiles", |t| {
			t(|_, args: FileCopierJobInit, library: Library| async move {
				library.spawn_job(args).await;
				Ok(())
			})
		})
		.library_mutation("copyFiles", |t| {
			t(|_, args: FileCopierJobInit, library: Library| async move {
				library.spawn_job(args).await;
				Ok(())
			})
		})
		.library_mutation("cutFiles", |t| {
			t(|_, args: FileCutterJobInit, library: Library| async move {
				library.spawn_job(args).await;
				Ok(())
			})
		})
		.library_mutation("renameFile", |t| {
			#[derive(Type, Deserialize)]
			pub struct RenameFileArgs {
				pub location_id: i32,
				pub file_name: String,
				pub new_file_name: String,
			}

			t(|_, args: RenameFileArgs, library: Library| async move {
				let location = find_location(&library, args.location_id)
					.select(location::select!({ path }))
					.exec()
					.await?
					.ok_or(LocationError::IdNotFound(args.location_id))?;

				let location_path = Path::new(&location.path);
				fs::rename(
					location_path.join(&args.file_name),
					location_path.join(&args.new_file_name),
				)
				.await
				.map_err(|e| {
					rspc::Error::new(ErrorCode::Conflict, format!("Failed to rename file: {e}"))
				})?;

				invalidate_query!(library, "tags.getExplorerData");

				Ok(())
			})
		})
}
