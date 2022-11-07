use std::str::FromStr;

use sd_crypto::{crypto::stream::Algorithm, keys::{hashing::HashingAlgorithm, keymanager::KeyManager}, Protected, primitives::generate_passphrase};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{invalidate_query, prisma::key};

use super::{utils::LibraryRequest, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct KeyAddArgs {
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
	key: String,
	library_sync: bool,
}

#[derive(Type, Deserialize)]
pub struct KeyNameUpdateArgs {
	uuid: uuid::Uuid,
	name: String,
}

#[derive(Type, Deserialize)]
pub struct SetMasterPasswordArgs {
	password: String,
	secret_key: String,
}

#[derive(Type, Deserialize)]
pub struct OnboardingArgs {
	algorithm: Algorithm,
	hashing_algorithm: HashingAlgorithm,
}

#[derive(Type, Serialize)]
pub struct OnboardingKeys {
	passphrase: String,
	secret_key: String,
}

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.dump_keystore()) })
		})
		// do not unlock the key manager until this route returns true
		.library_query("hasMasterPassword", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.has_master_password()?) })
		})
		// this is so we can show the key as mounted in the UI
		.library_query("listMounted", |t| {
			t(|_, _: (), library| async move { Ok(library.key_manager.get_mounted_uuids()) })
		})
		.library_mutation("mount", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.mount(key_uuid)?;
				// we also need to dispatch jobs that automatically decrypt preview media and metadata here
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		.library_mutation("updateKeyName", |t| {
			t(|_, args: KeyNameUpdateArgs, library| async move {
				library
					.db
					.key()
					.update(
						key::uuid::equals(args.uuid.to_string()),
						vec![key::SetParam::SetName(Some(args.name))],
					)
					.exec()
					.await?;

				Ok(())
			})
		})
		.library_mutation("unmount", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.unmount(key_uuid)?;
				// we also need to delete all in-memory decrypted data associated with this key
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
		.library_mutation("clearMasterPassword", |t| {
			t(|_, _: (), library| async move {
				library.key_manager.clear_master_password()?;
				// we also need to delete all in-memory decrypted data associated with this key
				invalidate_query!(library, "keys.hasMasterPassword");
				Ok(())
			})
		})
		.library_mutation("deleteFromLibrary", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.remove_key(key_uuid)?;

				library
					.db
					.key()
					.delete(key::uuid::equals(key_uuid.to_string()))
					.exec()
					.await?;

				// we also need to delete all in-memory decrypted data associated with this key
				invalidate_query!(library, "keys.list");
				invalidate_query!(library, "keys.listMounted");
				invalidate_query!(library, "keys.getDefault");
				Ok(())
			})
		})
		.library_mutation("onboarding", |t| {
			t(|_, args: OnboardingArgs, library| async move {
				// if this returns an error, the user MUST re-enter the correct password
				let passphrase = generate_passphrase();
				let bundle = KeyManager::onboarding(Protected::new(passphrase.expose().as_bytes().to_vec()), args.algorithm, args.hashing_algorithm)?;
				
				let verification_key = bundle.verification_key;

				// remove old nil-id keys if they were set
				// they possibly won't be, it depends on how we handle this during onboarding
				library
					.db
					.key()
					.delete_many(vec![key::uuid::equals(uuid::Uuid::nil().to_string())])
					.exec()
					.await?;

				library
					.db
					.key()
					.create(
						verification_key.uuid.to_string(),
						verification_key.algorithm.serialize().to_vec(),
						verification_key.hashing_algorithm.serialize().to_vec(),
						verification_key.salt.to_vec(),
						verification_key.content_salt.to_vec(),
						verification_key.master_key.to_vec(),
						verification_key.master_key_nonce.to_vec(),
						verification_key.key_nonce.to_vec(),
						verification_key.key.to_vec(),
						vec![],
					)
					.exec()
					.await?;
				
				let secret_key = base64::encode(bundle.secret_key.expose());

				let keys = OnboardingKeys { passphrase: passphrase.expose().clone(), secret_key };

				Ok(keys)
			})
		})
		.library_mutation("setMasterPassword", |t| {
			t(|_, args: SetMasterPasswordArgs, library| async move {
				// if this returns an error, the user MUST re-enter the correct password
				library
					.key_manager
					.set_master_password(Protected::new(args.password), Protected::new(args.secret_key))?;

				let automount = library
					.db
					.key()
					.find_many(vec![key::automount::equals(true)])
					.exec()
					.await?;

				for key in automount {
					library
						.key_manager
						.mount(uuid::Uuid::from_str(&key.uuid).map_err(|_| {
							rspc::Error::new(
								rspc::ErrorCode::InternalServerError,
								"Error deserializing UUID from string".into(),
							)
						})?)?;
				}

				invalidate_query!(library, "keys.hasMasterPassword");
						
				Ok(())
			})
		})
		.library_mutation("setDefault", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.set_default(key_uuid)?;

				// if an old default is set, unset it as the default
				let old_default = library
					.db
					.key()
					.find_first(vec![key::default::equals(true)])
					.exec()
					.await?;

				if let Some(key) = old_default {
					library
						.db
						.key()
						.update(
							key::uuid::equals(key.uuid),
							vec![key::SetParam::SetDefault(false)],
						)
						.exec()
						.await?;
				}

				let new_default = library
					.db
					.key()
					.find_unique(key::uuid::equals(key_uuid.to_string()))
					.exec()
					.await?;

				// if the new default key is stored in the library, update it as the default
				if let Some(default) = new_default {
					library
						.db
						.key()
						.update(
							key::uuid::equals(default.uuid),
							vec![key::SetParam::SetDefault(true)],
						)
						.exec()
						.await?;
				}

				invalidate_query!(library, "keys.getDefault");
				Ok(())
			})
		})
		.library_query("getDefault", |t| {
			t(|_, _: (), library| async move {
				let default = library.key_manager.get_default();

				if let Ok(default_key) = default {
					Ok(Some(default_key))
				} else {
					Ok(None)
				}
			})
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
				let uuid = library.key_manager.add_to_keystore(
					Protected::new(args.key.as_bytes().to_vec()),
					args.algorithm,
					args.hashing_algorithm,
				)?;

				let stored_key = library.key_manager.access_keystore(uuid)?;

				if args.library_sync {
					library
					.db
					.key()
					.create(
						uuid.to_string(),
						args.algorithm.serialize().to_vec(),
						args.hashing_algorithm.serialize().to_vec(),
						stored_key.salt.to_vec(),
						stored_key.content_salt.to_vec(),
						stored_key.master_key.to_vec(),
						stored_key.master_key_nonce.to_vec(),
						stored_key.key_nonce.to_vec(),
						stored_key.key.to_vec(),
						vec![],
					)
					.exec()
					.await?;
				}

				// mount the key
				library.key_manager.mount(uuid)?;

				invalidate_query!(library, "keys.list");
				invalidate_query!(library, "keys.listMounted");
				Ok(())
			})
		})
}
