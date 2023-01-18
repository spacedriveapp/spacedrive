use crate::{
	invalidate_query,
	job::Job,
	object::fs::{
		copy::{FileCopierJob, FileCopierJobInit},
		cut::{FileCutterJob, FileCutterJobInit},
		decrypt::{FileDecryptorJob, FileDecryptorJobInit},
		delete::{FileDeleterJob, FileDeleterJobInit},
		duplicate::{FileDuplicatorJob, FileDuplicatorJobInit},
		encrypt::{FileEncryptorJob, FileEncryptorJobInit},
		erase::{FileEraserJob, FileEraserJobInit},
	},
	prisma::object,
};

use rspc::Type;
use serde::Deserialize;

use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_query("get", |t| {
			#[derive(Type, Deserialize)]
			pub struct GetArgs {
				pub id: i32,
			}
			t(|_, args: GetArgs, library| async move {
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

			t(|_, args: SetNoteArgs, library| async move {
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

			t(|_, args: SetFavoriteArgs, library| async move {
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
			t(|_, id: i32, library| async move {
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
			t(|_, args: FileEncryptorJobInit, library| async move {
				library.spawn_job(Job::new(args, FileEncryptorJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("decryptFiles", |t| {
			t(|_, args: FileDecryptorJobInit, library| async move {
				library.spawn_job(Job::new(args, FileDecryptorJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("deleteFiles", |t| {
			t(|_, args: FileDeleterJobInit, library| async move {
				library.spawn_job(Job::new(args, FileDeleterJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("eraseFiles", |t| {
			t(|_, args: FileEraserJobInit, library| async move {
				library.spawn_job(Job::new(args, FileEraserJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("duplicateFiles", |t| {
			t(|_, args: FileCopierJobInit, library| async move {
				library.spawn_job(Job::new(args, FileCopierJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("copyFiles", |t| {
			t(|_, args: FileCopierJobInit, library| async move {
				library.spawn_job(Job::new(args, FileCopierJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("cutFiles", |t| {
			t(|_, args: FileCutterJobInit, library| async move {
				library.spawn_job(Job::new(args, FileCutterJob {})).await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
}
