use std::str::FromStr;

use sd_crypto::{
	crypto::stream::Algorithm,
	keys::hashing::{HashingAlgorithm, Params},
	Protected,
};
use serde::Deserialize;
use specta::Type;

use crate::prisma::key;

use super::{utils::LibraryRequest, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct KeyAddArgs {
	algorithm: String,
	hashing_algorithm: String,
	key: String,
}

#[derive(Type, Deserialize)]
pub struct KeyNameUpdateArgs {
	uuid: uuid::Uuid,
	name: String,
}

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(
				|_, _: (), library| async move { Ok(library.db.key().find_many(vec![]).exec().await?) },
			)
		})
		// this is so we can show the key as mounted in the UI
		.library_query("listMounted", |t| {
			t(|_, _: (), library| async move {
				Ok(library.key_manager.lock().await.get_mounted_uuids())
			})
		})
		.library_query("mount", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.lock().await.mount(key_uuid).unwrap();
				// we also need to dispatch jobs that automatically decrypt preview media and metadata here

				Ok(())
			})
		})
		.library_query("updateKeyName", |t| {
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
		.library_query("unmount", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library.key_manager.lock().await.unmount(key_uuid).unwrap();
				// we also need to delete all in-memory decrypted data associated with this key

				Ok(())
			})
		})
		.library_query("setMasterPassword", |t| {
			t(|_, password: String, library| async move {
				// need to add master password checks in the keymanager itself to make sure it's correct
				// this can either unwrap&fail, or we can return the error. either way, the user will have to correct this
				// by entering the correct password
				// for now, automounting might have to serve as the master password checks

				library
					.key_manager
					.lock()
					.await
					.set_master_password(Protected::new(password.as_bytes().to_vec()))
					.unwrap();

				let automount = library.db.key().find_many(vec![key::automount::equals(true)]).exec().await?;

				for key in automount {
					library.key_manager.lock().await.mount(uuid::Uuid::from_str(&key.uuid).unwrap()).unwrap();
				}

				Ok(())
			})
		})
		.library_query("setDefault", |t| {
			t(|_, key_uuid: uuid::Uuid, library| async move {
				library
					.key_manager
					.lock()
					.await
					.set_default(key_uuid)
					.unwrap();

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

				// we allow this to error as the new default **SHOULD** exist
				let new_default = library
					.db
					.key()
					.find_unique(key::uuid::equals(key_uuid.to_string()))
					.exec()
					.await?
					.unwrap();
				library
					.db
					.key()
					.update(
						key::uuid::equals(new_default.uuid),
						vec![key::SetParam::SetDefault(true)],
					)
					.exec()
					.await?;

				Ok(())
			})
		})
		.library_query("getDefault", |t| {
			t(|_, _: (), library| async move {
				// `find_first` should be okay here as only one default key should ever be set
				// this is also stored in the keymanager but it's probably easier to get it from the DB
				Ok(library
					.db
					.key()
					.find_first(vec![key::default::equals(true)])
					.exec()
					.await?)
			})
		})
		.library_mutation("add", |t| {
			t(|_, args: KeyAddArgs, library| async move {
				// TODO(jake): remove this once we are able to get the master password from the UI
				// use the designated route for setting it
				library.key_manager.lock().await.set_master_password(Protected::new(b"password".to_vec())).unwrap();

				let algorithm = match &args.algorithm as &str {
					"XChaCha20Poly1305" => Algorithm::XChaCha20Poly1305,
					"Aes256Gcm" => Algorithm::Aes256Gcm,
					_ => unreachable!(),
				};

				// we need to get parameters from somewhere, possibly a global config setting - they're just set to standard for now
				// we're just mapping bcrypt to argon2id temporarily as i'm unsure whether or not we're actually adding bcrypt
				let hashing_algorithm = match &args.hashing_algorithm as &str {
					"Argon2id" => HashingAlgorithm::Argon2id(Params::Standard),
					"Bcrypt" => HashingAlgorithm::Argon2id(Params::Standard),
					_ => unreachable!(),
				};

				// register the key with the keymanager
				let uuid = library
					.key_manager
					.lock()
					.await
					.add_to_keystore(
						Protected::new(args.key.as_bytes().to_vec()),
						algorithm,
						hashing_algorithm,
					)
					.unwrap();

				let stored_key = library
					.key_manager
					.lock()
					.await
					.access_keystore(uuid)
					.unwrap();

				library
					.db
					.key()
					.create(
						uuid.to_string(),
						false,
						algorithm.serialize().to_vec(),
						hashing_algorithm.serialize().to_vec(),
						stored_key.salt.to_vec(),
						stored_key.content_salt.to_vec(),
						stored_key.master_key.to_vec(),
						stored_key.master_key_nonce.to_vec(),
						stored_key.key_nonce.to_vec(),
						stored_key.key.to_vec(),
						false,
						vec![],
					)
					.exec()
					.await?;

				// mount the key
				library.key_manager.lock().await.mount(uuid).unwrap();

				Ok(())
			})
		})
}
