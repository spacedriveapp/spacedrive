use crate::{
	invalidate_query,
	job::Job,
	location::fetch_location,
	object::fs::{
		decrypt::{FileDecryptorJob, FileDecryptorJobInit},
		encrypt::{FileEncryptorJob, FileEncryptorJobInit},
	},
	prisma::object,
};

use rspc::{ErrorCode, Type};
use serde::Deserialize;

use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_query("readMetadata", |t| {
			t(|_, _id: i32, _| async move {
				#[allow(unreachable_code)]
				Ok(todo!())
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
			#[derive(Type, Deserialize)]
			pub struct FileEncryptorJobArgs {
				pub id: i32,
				pub object_id: i32,
				pub key_uuid: uuid::Uuid,
			}

			t(|_, args: FileEncryptorJobArgs, library| async move {
				if fetch_location(&library, args.id).exec().await?.is_none() {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Location not found".into(),
					));
				}

				library
					.spawn_job(Job::new(
						FileEncryptorJobInit {
							location_id: args.id,
							object_id: args.object_id,
							key_uuid: args.key_uuid,
						},
						Box::new(FileEncryptorJob {}),
					))
					.await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
		.library_mutation("decryptFiles", |t| {
			#[derive(Type, Deserialize)]
			pub struct FileDecryptorJobArgs {
				pub id: i32,
				pub object_id: i32,
			}

			t(|_, args: FileDecryptorJobArgs, library| async move {
				if fetch_location(&library, args.id).exec().await?.is_none() {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Location not found".into(),
					));
				}

				library
					.spawn_job(Job::new(
						FileDecryptorJobInit {
							location_id: args.id,
							object_id: args.object_id,
						},
						Box::new(FileDecryptorJob {}),
					))
					.await;
				invalidate_query!(library, "locations.getExplorerData");

				Ok(())
			})
		})
}
