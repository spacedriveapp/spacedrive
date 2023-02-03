use sd_crypto::primitives::SECRET_KEY_IDENTIFIER;
use std::{path::PathBuf, str::FromStr};
use tokio::fs::File;

use sd_crypto::keys::keymanager::{StoredKey, StoredKeyType};
use sd_crypto::{crypto::stream::Algorithm, keys::hashing::HashingAlgorithm, Error, Protected};
use serde::Deserialize;
use specta::Type;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::util::db::write_storedkey_to_db;
use crate::{invalidate_query, prisma::key};

use super::{utils::LibraryRequest, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct KeyAddArgs {
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
	key: Protected<String>,
	library_sync: bool,
	automount: bool,
}

#[derive(Type, Deserialize)]
pub struct UnlockKeyManagerArgs {
	password: Protected<String>,
	secret_key: Protected<String>,
}

#[derive(Type, Deserialize)]
pub struct RestoreBackupArgs {
	password: Protected<String>,
	secret_key: Protected<String>,
	path: PathBuf,
}

#[derive(Type, Deserialize)]
pub struct MasterPasswordChangeArgs {
	password: Protected<String>,
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
}

#[derive(Type, Deserialize)]
pub struct AutomountUpdateArgs {
	uuid: Uuid,
	status: bool,
}

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.dump_keystore()) })
		})
		// do not unlock the key manager until this route returns true
		.library_query("isUnlocked", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.is_unlocked().await?) })
		})
		// this is so we can show the key as mounted in the UI
		.library_query("listMounted", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.get_mounted_uuids()) })
		})
		.library_query("getKey", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				Ok(library
					.key_manager
					.get_key(key_uuid)
					.await?
					.expose()
					.clone())
			})
		})
		.library_mutation("mount", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				library.key_manager.mount(key_uuid).await?;
				// we also need to dispatch jobs that automatically decrypt preview media and metadata here
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		.library_query("keyringHasSecretKey", |t| {
			t(|_, _: (), library| async move {
				Ok(library
					.key_manager
					.keyring_contains_valid_secret_key(library.id)
					.await
					.is_ok())
			})
		})
		.library_query("getSecretKey", |t| {
			t(|_, _: (), library| async move {
				Ok(library
					.key_manager
					.keyring_retrieve(library.id, SECRET_KEY_IDENTIFIER.to_string())
					.await?
					.expose()
					.clone())
			})
		})
		.library_mutation("unmount", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				library.key_manager.unmount(key_uuid)?;
				// we also need to delete all in-memory decrypted data associated with this key
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		.library_mutation("clearMasterPassword", |t| {
			t(|_, _: (), library| async move {
				// This technically clears the root key, but it means the same thing to the frontend
				library.key_manager.clear_root_key().await?;

				invalidate_query!(library, "keys.isUnlocked");
				Ok(())
			})
		})
		.library_mutation("syncKeyToLibrary", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				let key = library.key_manager.sync_to_database(key_uuid)?;

				// does not check that the key doesn't exist before writing
				write_storedkey_to_db(&library.db, &key).await?;

				invalidate_query!(library, "keys.list");
				Ok(())
			})
		})
		.library_mutation("updateAutomountStatus", |t| {
			t(|_, args: AutomountUpdateArgs, library| async move {
				if !library.key_manager.is_memory_only(args.uuid)? {
					library
						.key_manager
						.change_automount_status(args.uuid, args.status)?;

					library
						.db
						.key()
						.update(
							key::uuid::equals(args.uuid.to_string()),
							vec![key::SetParam::SetAutomount(args.status)],
						)
						.exec()
						.await?;

					invalidate_query!(library, "keys.list");
				}

				Ok(())
			})
		})
		.library_mutation("deleteFromLibrary", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				if !library.key_manager.is_memory_only(key_uuid)? {
					library
						.db
						.key()
						.delete(key::uuid::equals(key_uuid.to_string()))
						.exec()
						.await?;
				}

				library.key_manager.remove_key(key_uuid).await?;

				// we also need to delete all in-memory decrypted data associated with this key
				invalidate_query!(library, "keys.list");
				invalidate_query!(library, "keys.listMounted");
				invalidate_query!(library, "keys.getDefault");
				Ok(())
			})
		})
		.library_mutation("unlockKeyManager", |t| {
			t(|_, args: UnlockKeyManagerArgs, library| async move {
				let secret_key = (!args.secret_key.expose().is_empty()).then_some(args.secret_key);

				library
					.key_manager
					.unlock(args.password, secret_key, library.id, || {
						invalidate_query!(library, "keys.isKeyManagerUnlocking")
					})
					.await?;

				invalidate_query!(library, "keys.isUnlocked");

				let automount = library
					.db
					.key()
					.find_many(vec![key::automount::equals(true)])
					.exec()
					.await?;

				for key in automount {
					library
						.key_manager
						.mount(Uuid::from_str(&key.uuid).map_err(|_| Error::Serialization)?)
						.await?;

					invalidate_query!(library, "keys.listMounted");
				}

				Ok(())
			})
		})
		.library_mutation("setDefault", |t| {
			t(|_, key_uuid: Uuid, library| async move {
				library.key_manager.set_default(key_uuid).await?;

				library
					.db
					.key()
					.update_many(
						vec![key::default::equals(true)],
						vec![key::SetParam::SetDefault(false)],
					)
					.exec()
					.await?;

				library
					.db
					.key()
					.update(
						key::uuid::equals(key_uuid.to_string()),
						vec![key::SetParam::SetDefault(true)],
					)
					.exec()
					.await?;

				invalidate_query!(library, "keys.getDefault");
				Ok(())
			})
		})
		.library_query("getDefault", |t| {
			t(|_, _: (), library| async move { library.key_manager.get_default().await.ok() })
		})
		.library_query("isKeyManagerUnlocking", |t| {
			t(|_, _: (), library| async move { library.key_manager.is_unlocking().await.ok() })
		})
		.library_mutation("unmountAll", |t| {
			t(|_, _: (), library| async move {
				library.key_manager.empty_keymount();
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		// this also mounts the key
		.library_mutation("add", |t| {
			t(|_, args: KeyAddArgs, library| async move {
				// register the key with the keymanager
				let uuid = library
					.key_manager
					.add_to_keystore(
						args.key,
						args.algorithm,
						args.hashing_algorithm,
						!args.library_sync,
						args.automount,
						None,
					)
					.await?;

				if args.library_sync {
					write_storedkey_to_db(&library.db, &library.key_manager.access_keystore(uuid)?)
						.await?;

					if args.automount {
						library
							.db
							.key()
							.update(
								key::uuid::equals(uuid.to_string()),
								vec![key::SetParam::SetAutomount(true)],
							)
							.exec()
							.await?;
					}
				}

				library.key_manager.mount(uuid).await?;

				invalidate_query!(library, "keys.list");
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		.library_mutation("backupKeystore", |t| {
			t(|_, path: PathBuf, library| async move {
				// dump all stored keys that are in the key manager (maybe these should be taken from prisma as this will include even "non-sync with library" keys)
				let mut stored_keys = library.key_manager.dump_keystore();

				// include the verification key at the time of backup
				stored_keys.push(library.key_manager.get_verification_key().await?);

				// exclude all memory-only keys
				stored_keys.retain(|k| !k.memory_only);

				let mut output_file = File::create(path).await.map_err(Error::Io)?;
				output_file
					.write_all(&serde_json::to_vec(&stored_keys).map_err(|_| Error::Serialization)?)
					.await
					.map_err(Error::Io)?;
				Ok(())
			})
		})
		.library_mutation("restoreKeystore", |t| {
			t(|_, args: RestoreBackupArgs, library| async move {
				let mut input_file = File::open(args.path).await.map_err(Error::Io)?;

				let mut backup = Vec::new();

				input_file
					.read_to_end(&mut backup)
					.await
					.map_err(Error::Io)?;

				let stored_keys: Vec<StoredKey> =
					serde_json::from_slice(&backup).map_err(|_| Error::Serialization)?;

				let updated_keys = library
					.key_manager
					.import_keystore_backup(args.password, args.secret_key, &stored_keys)
					.await?;

				for key in &updated_keys {
					write_storedkey_to_db(&library.db, key).await?;
				}

				invalidate_query!(library, "keys.list");
				invalidate_query!(library, "keys.listMounted");

				Ok(updated_keys.len())
			})
		})
		.library_mutation("changeMasterPassword", |t| {
			t(|_, args: MasterPasswordChangeArgs, library| async move {
				let verification_key = library
					.key_manager
					.change_master_password(
						args.password,
						args.algorithm,
						args.hashing_algorithm,
						library.id,
					)
					.await?;

				// remove old nil-id keys if they were set
				library
					.db
					.key()
					.delete_many(vec![key::key_type::equals(
						serde_json::to_string(&StoredKeyType::Root).unwrap(),
					)])
					.exec()
					.await?;

				// write the new verification key
				write_storedkey_to_db(&library.db, &verification_key).await?;

				Ok(())
			})
		})
}
