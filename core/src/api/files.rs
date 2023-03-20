use crate::{
	api::utils::library,
	invalidate_query,
	job::Job,
	object::fs::{
		copy::{FileCopierJob, FileCopierJobInit},
		cut::{FileCutterJob, FileCutterJobInit},
		decrypt::{FileDecryptorJob, FileDecryptorJobInit},
		delete::{FileDeleterJob, FileDeleterJobInit},
		encrypt::{FileEncryptorJob, FileEncryptorJobInit},
		erase::{FileEraserJob, FileEraserJobInit},
	},
	prisma::object,
};

use rspc::{alpha::AlphaRouter, Type};
use serde::Deserialize;
use tokio::sync::oneshot;

use super::{t, Ctx};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	t.router()
		.procedure("get", {
			#[derive(Type, Deserialize)]
			pub struct GetArgs {
				pub id: i32,
			}

			t.with(library())
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

			t.with(library())
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

					invalidate_query!(library, "locations.getExplorerData");
					invalidate_query!(library, "tags.getExplorerData");

					Ok(())
				})
		})
		.procedure("setFavorite", {
			#[derive(Type, Deserialize)]
			pub struct SetFavoriteArgs {
				pub id: i32,
				pub favorite: bool,
			}

			t.with(library())
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

					invalidate_query!(library, "locations.getExplorerData");
					invalidate_query!(library, "tags.getExplorerData");

					Ok(())
				})
		})
		.procedure("delete", {
			t.with(library())
				.mutation(|(_, library), id: i32| async move {
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
		.procedure("encryptFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileEncryptorJobInit| async move {
					library.spawn_job(Job::new(args, FileEncryptorJob {})).await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("decryptFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileDecryptorJobInit| async move {
					library.spawn_job(Job::new(args, FileDecryptorJob {})).await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("deleteFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileDeleterJobInit| async move {
					library.spawn_job(Job::new(args, FileDeleterJob {})).await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("eraseFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileEraserJobInit| async move {
					library.spawn_job(Job::new(args, FileEraserJob {})).await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("duplicateFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					let (done_tx, done_rx) = oneshot::channel();

					library
						.spawn_job(Job::new(
							args,
							FileCopierJob {
								done_tx: Some(done_tx),
							},
						))
						.await;

					let _ = done_rx.await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("copyFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					let (done_tx, done_rx) = oneshot::channel();

					library
						.spawn_job(Job::new(
							args,
							FileCopierJob {
								done_tx: Some(done_tx),
							},
						))
						.await;

					let _ = done_rx.await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
		.procedure("cutFiles", {
			t.with(library())
				.mutation(|(_, library), args: FileCutterJobInit| async move {
					library.spawn_job(Job::new(args, FileCutterJob {})).await;
					invalidate_query!(library, "locations.getExplorerData");

					Ok(())
				})
		})
}
