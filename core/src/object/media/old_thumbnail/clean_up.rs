use crate::library::LibraryId;

use sd_prisma::prisma::{file_path, PrismaClient};
use sd_utils::error::FileIOError;

use std::{collections::HashSet, ffi::OsString, path::PathBuf, sync::Arc};

use futures_concurrency::future::Join;
use tokio::{fs, spawn};
use tracing::{debug, error};

use super::{ThumbnailerError, EPHEMERAL_DIR, WEBP_EXTENSION};

// TODO(fogodev) Introduce a task using the new task system to clean up the thumbnails from time to time.

pub(super) async fn process_ephemeral_clean_up(
	thumbnails_directory: Arc<PathBuf>,
	existing_ephemeral_thumbs: HashSet<OsString>,
) {
	let ephemeral_thumbs_dir = thumbnails_directory.join(EPHEMERAL_DIR);

	spawn(async move {
		let mut to_remove = vec![];

		let mut read_ephemeral_thumbs_dir = fs::read_dir(&ephemeral_thumbs_dir)
			.await
			.map_err(|e| FileIOError::from((&ephemeral_thumbs_dir, e)))?;

		while let Some(shard_entry) = read_ephemeral_thumbs_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((&ephemeral_thumbs_dir, e)))?
		{
			let shard_path = shard_entry.path();
			if shard_entry
				.file_type()
				.await
				.map_err(|e| FileIOError::from((&shard_path, e)))?
				.is_dir()
			{
				let mut read_shard_dir = fs::read_dir(&shard_path)
					.await
					.map_err(|e| FileIOError::from((&shard_path, e)))?;

				while let Some(thumb_entry) = read_shard_dir
					.next_entry()
					.await
					.map_err(|e| FileIOError::from((&shard_path, e)))?
				{
					let thumb_path = thumb_entry.path();
					if thumb_path.extension() == Some(WEBP_EXTENSION.as_ref())
						&& !existing_ephemeral_thumbs.contains(&thumb_entry.file_name())
					{
						to_remove.push(async move {
							debug!(
								thumb_path = %thumb_path.display(),
								"Removing stale ephemeral thumbnail;",
							);

							fs::remove_file(&thumb_path).await.map_err(|e| {
								ThumbnailerError::FileIO(FileIOError::from((thumb_path, e)))
							})
						});
					}
				}
			}
		}

		Ok::<_, ThumbnailerError>(to_remove.join().await)
	})
	.await
	.map_or_else(
		|e| error!(?e, "Join error on ephemeral clean up;",),
		|fetching_res| {
			fetching_res.map_or_else(
				|e| error!(?e, "Error fetching ephemeral thumbs to be removed;"),
				|remove_results| {
					remove_results.into_iter().for_each(|remove_res| {
						if let Err(e) = remove_res {
							error!(?e, "Error on ephemeral clean up;");
						}
					})
				},
			)
		},
	)
}

pub(super) async fn process_indexed_clean_up(
	thumbnails_directory: Arc<PathBuf>,
	libraries_ids_and_databases: Vec<(LibraryId, Arc<PrismaClient>)>,
) {
	libraries_ids_and_databases
		.into_iter()
		.map(|(library_id, db)| {
			let library_thumbs_dir = thumbnails_directory.join(library_id.to_string());
			spawn(async move {
				let existing_thumbs = db
					.file_path()
					.find_many(vec![file_path::cas_id::not(None)])
					.select(file_path::select!({ cas_id }))
					.exec()
					.await?
					.into_iter()
					.map(|file_path| {
						OsString::from(format!(
							"{}.webp",
							file_path.cas_id.expect("we filtered right")
						))
					})
					.collect::<HashSet<_>>();

				let mut read_library_thumbs_dir = fs::read_dir(&library_thumbs_dir)
					.await
					.map_err(|e| FileIOError::from((&library_thumbs_dir, e)))?;

				let mut to_remove = vec![];

				while let Some(shard_entry) = read_library_thumbs_dir
					.next_entry()
					.await
					.map_err(|e| FileIOError::from((&library_thumbs_dir, e)))?
				{
					let shard_path = shard_entry.path();
					if shard_entry
						.file_type()
						.await
						.map_err(|e| FileIOError::from((&shard_path, e)))?
						.is_dir()
					{
						let mut read_shard_dir = fs::read_dir(&shard_path)
							.await
							.map_err(|e| FileIOError::from((&shard_path, e)))?;

						while let Some(thumb_entry) = read_shard_dir
							.next_entry()
							.await
							.map_err(|e| FileIOError::from((&shard_path, e)))?
						{
							let thumb_path = thumb_entry.path();
							if thumb_path.extension() == Some(WEBP_EXTENSION.as_ref())
								&& !existing_thumbs.contains(&thumb_entry.file_name())
							{
								to_remove.push(async move {
									debug!(
										thumb_path = %thumb_path.display(),
										"Removing stale indexed thumbnail;",
									);

									fs::remove_file(&thumb_path).await.map_err(|e| {
										ThumbnailerError::FileIO(FileIOError::from((thumb_path, e)))
									})
								});
							}
						}
					}
				}

				Ok::<_, ThumbnailerError>(to_remove.join().await)
			})
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|join_res| {
			join_res
				.map_err(|e| error!(?e, "Join error on indexed clean up;"))
				.ok()
		})
		.filter_map(|fetching_res| {
			fetching_res
				.map_err(|e| error!(?e, "Error fetching indexed thumbs to be removed;"))
				.ok()
		})
		.for_each(|remove_results| {
			remove_results.into_iter().for_each(|remove_res| {
				if let Err(e) = remove_res {
					error!(?e, "Error on indexed clean up;");
				}
			})
		})
}
