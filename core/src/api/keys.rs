use super::{utils::library, Ctx, R};
use crate::{crypto::KeyType, invalidate_query};
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
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this lists all of the keys
				Ok(library.key_manager.list(KeyType::User).await?)
			})
		})
		.procedure("listRoot", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this lists all of the root keys
				// root keys are used to unlock the key manager
				// something like a master password
				Ok(library.key_manager.list(KeyType::Root).await?)
			})
		})
		.procedure("isUnlocked", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this states if the KM is unlocked or not
				Ok(library.key_manager.is_unlocked().await)
			})
		})
		.procedure("isUnlocking", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this states if the key manager is unlocking or not
				Ok(library.key_manager.is_unlocking().await?)
			})
		})
		.procedure("isSetup", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this determines if the key manager is set up or not
				Ok(!library.db.key().find_many(vec![]).exec().await?.is_empty())
			})
		})
		.procedure("reset", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				// this is used for resetting the key manager
				// all keys are cleared
				Ok(library.key_manager.reset().await?)
			})
		})
		.procedure("setup", {
			R.with2(library())
				// this is used for setting up the key manager, it also unlocks it
				// the returned secret key needs to be shown to the user
				.mutation(|(_, library), args: SetupArgs| async move {
					let secret_key = library
						.key_manager
						.initial_setup(args.algorithm, args.hashing_algorithm, args.password)
						.await?;

					invalidate_query!(library, "keys.isSetup");
					invalidate_query!(library, "keys.isUnlocked");

					Ok(secret_key)
				})
		})
		.procedure("mount", {
			R.with2(library())
				// this is used for mounting a key, it requires the key itself and the uuid
				.mutation(|(_, library), args: MountArgs| async move {
					library.key_manager.mount(args.uuid, args.password).await?;

					// we also need to dispatch jobs that automatically decrypt preview media and metadata here

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("unmount", {
			R.with2(library())
				// this is used for unmounting a key
				.mutation(|(_, library), uuid: Uuid| async move {
					library.key_manager.unmount(uuid).await?;
					// we also need to delete all in-memory decrypted data associated with this key

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("lock", {
			R.with2(library())
				// this is used for locking the key manager, so that the master password must be entered again
				.mutation(|(_, library), _: ()| async move {
					library.key_manager.lock().await?;

					invalidate_query!(library, "keys.isUnlocked");
					Ok(())
				})
		})
		.procedure("delete", {
			R.with2(library())
				// this is used for deleting a key, it also unmounts the key
				.mutation(|(_, library), uuid: Uuid| async move {
					library.key_manager.delete(uuid).await?;

					// we also need to delete all in-memory decrypted data associated with this key

					invalidate_query!(library, "keys.list");
					Ok(())
				})
		})
		.procedure("unlock", {
			// this is used for unlocking the key manager. revamped OS keyring support is a WIP
			// so users will have to provide their secret key
			// in the future it will be optional
			R.with2(library())
				.mutation(|(_, library), args: UnlockArgs| async move {
					library
						.key_manager
						.unlock(args.password, args.secret_key)
						.await?;

					invalidate_query!(library, "keys.isUnlocked");
					invalidate_query!(library, "keys.isUnlocking");

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
				// this is used for adding a new key
				// users can optionally provide a "word", for if they're re-adding this key from a previous library
				// the word must be at least 3 characters long
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
			// this is used for backing up all keys and root keys to the specified path
			R.with2(library())
				.mutation(|(_, library), path: PathBuf| async move {
					library.key_manager.backup_to_file(path).await?;

					Ok(())
				})
		})
		.procedure("restoreKeystore", {
			// this is used for restoring a previous backup, it requires the master password and secret key from the time of the backup
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

					Ok(amount)
				})
		})
		.procedure("addRootKey", {
			// this is used for adding a new master password
			// the returned secret key needs to be displayed to the user
			R.with2(library())
				.mutation(|(_, library), args: SetupArgs| async move {
					let secret_key = library
						.key_manager
						.add_root_key(args.algorithm, args.hashing_algorithm, args.password)
						.await?;

					invalidate_query!(library, "keys.listRoot");

					Ok(secret_key)
				})
		})
}
