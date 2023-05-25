use super::{utils::library, Ctx, R};
use crate::invalidate_query;
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_crypto::{
	types::{Algorithm, HashingAlgorithm},
	Protected,
};
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Type, Deserialize)]
struct SetupArgs {
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
	password: Protected<String>,
}

#[derive(Type, Deserialize)]
struct UnlockArgs {
	password: Protected<String>,
	secret_key: Option<Protected<String>>,
}

#[derive(Type, Deserialize)]
struct MountArgs {
	uuid: Uuid,
	password: Protected<String>,
}

#[derive(Type, Deserialize)]
struct UpdateNameArgs {
	uuid: Uuid,
	name: String,
}

#[derive(Type, Deserialize)]
struct RestoreArgs {
	password: Protected<String>,
	secret_key: Protected<String>,
	path: PathBuf,
}

#[derive(Type, Deserialize)]
struct AddArgs {
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
	password: Protected<String>,
	word: Option<Protected<String>>,
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.with2(library())
				.query(|(_, library), _: ()| async move { Ok(library.key_manager.list().await?) })
		})
		.procedure("isUnlocked", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library.key_manager.is_unlocked().await)
			})
		})
		.procedure("isSetup", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(!library.db.key().find_many(vec![]).exec().await?.is_empty())
			})
		})
		.procedure("setup", {
			R.with2(library())
				.mutation(|(_, library), args: SetupArgs| async move {
					let sk = library
						.key_manager
						.initial_setup(args.algorithm, args.hashing_algorithm, args.password)
						.await?;

					invalidate_query!(library, "keys.isSetup");
					invalidate_query!(library, "keys.isUnlocked");

					Ok(sk)
				})
		})
		.procedure("mount", {
			R.with2(library())
				.mutation(|(_, library), args: MountArgs| async move {
					library.key_manager.mount(args.uuid, args.password).await?;

					// we also need to dispatch jobs that automatically decrypt preview media and metadata here

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("unmount", {
			R.with2(library())
				.mutation(|(_, library), uuid: Uuid| async move {
					library.key_manager.unmount(uuid).await?;
					// we also need to delete all in-memory decrypted data associated with this key

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("lock", {
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					library.key_manager.lock().await?;

					invalidate_query!(library, "keys.isUnlocked");
					Ok(())
				})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), uuid: Uuid| async move {
					library.key_manager.delete(uuid).await?;

					// we also need to delete all in-memory decrypted data associated with this key

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("unlock", {
			R.with2(library())
				.mutation(|(_, library), args: UnlockArgs| async move {
					library
						.key_manager
						.unlock(args.password, args.secret_key)
						.await?;

					invalidate_query!(library, "keys.isUnlocked");

					Ok(())
				})
		})
		.procedure("updateName", {
			R.with2(library())
				.mutation(|(_, library), args: UpdateNameArgs| async move {
					library
						.key_manager
						.update_key_name(args.uuid, args.name)
						.await?;

					invalidate_query!(library, "keys.list");

					Ok(())
				})
		})
		.procedure("unmountAll", {
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					let amount: u32 = library
						.key_manager
						.unmount_all()
						.await?
						.try_into()
						.map_err(|_| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Unable to convert updated key amount from i64 -> u32".to_string(),
							)
						})?;

					invalidate_query!(library, "keys.list");

					Ok(amount)
				})
		})
		.procedure("add", {
			R.with2(library())
				.mutation(|(_, library), args: AddArgs| async move {
					library
						.key_manager
						.insert_new(
							args.algorithm,
							args.hashing_algorithm,
							args.password,
							args.word,
						)
						.await?;

					invalidate_query!(library, "keys.list");

					Ok(())
				})
		})
		.procedure("backupKeystore", {
			R.with2(library())
				.mutation(|(_, library), path: PathBuf| async move {
					library.key_manager.backup_to_file(path).await?;

					Ok(())
				})
		})
		.procedure("restoreKeystore", {
			R.with2(library())
				.mutation(|(_, library), args: RestoreArgs| async move {
					let amount: u32 = library
						.key_manager
						.restore_from_file(args.path, args.password, args.secret_key)
						.await?
						.try_into()
						.map_err(|_| {
							rspc::Error::new(
								ErrorCode::InternalServerError,
								"Unable to convert updated key amount from i64 -> u32".to_string(),
							)
						})?;

					invalidate_query!(library, "keys.list");

					Ok(())
				})
		})
		.procedure("changeMasterPassword", {
			R.with2(library())
				.mutation(|(_, library), args: SetupArgs| async move { todo!() })
		})
}
