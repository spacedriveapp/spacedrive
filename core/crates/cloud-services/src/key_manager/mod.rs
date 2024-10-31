use crate::Error;

use sd_cloud_schema::{
	sync::{groups, KeyHash},
	NodeId, SecretKey as IrohSecretKey,
};
use sd_crypto::{cloud::secret_key::SecretKey, CryptoRng};
use sd_utils::error::FileIOError;

use std::{
	fmt,
	path::{Path, PathBuf},
};

use tokio::{fs, sync::RwLock};

mod key_store;

use key_store::KeyStore;

const KEY_FILE_NAME: &str = "space.keys";

pub struct KeyManager {
	master_key: SecretKey,
	keys_file_path: PathBuf,
	store: RwLock<KeyStore>,
}

impl KeyManager {
	pub async fn new(
		master_key: SecretKey,
		iroh_secret_key: IrohSecretKey,
		data_directory: impl AsRef<Path> + Send,
		rng: &mut CryptoRng,
	) -> Result<Self, Error> {
		async fn inner(
			master_key: SecretKey,
			iroh_secret_key: IrohSecretKey,
			keys_file_path: PathBuf,
			rng: &mut CryptoRng,
		) -> Result<KeyManager, Error> {
			let store = KeyStore::new(iroh_secret_key);
			store.encrypt(&master_key, rng, &keys_file_path).await?;

			Ok(KeyManager {
				master_key,
				keys_file_path,
				store: RwLock::new(store),
			})
		}

		inner(
			master_key,
			iroh_secret_key,
			data_directory.as_ref().join(KEY_FILE_NAME),
			rng,
		)
		.await
	}

	pub async fn load(
		master_key: SecretKey,
		data_directory: impl AsRef<Path> + Send,
	) -> Result<Self, Error> {
		async fn inner(
			master_key: SecretKey,
			keys_file_path: PathBuf,
		) -> Result<KeyManager, Error> {
			Ok(KeyManager {
				store: RwLock::new(
					KeyStore::decrypt(
						&master_key,
						fs::metadata(&keys_file_path).await.map_err(|e| {
							FileIOError::from((
								&keys_file_path,
								e,
								"Failed to read space keys file",
							))
						})?,
						&keys_file_path,
					)
					.await?,
				),
				master_key,
				keys_file_path,
			})
		}

		inner(master_key, data_directory.as_ref().join(KEY_FILE_NAME)).await
	}

	pub async fn iroh_secret_key(&self) -> IrohSecretKey {
		self.store.read().await.iroh_secret_key()
	}

	pub async fn node_id(&self) -> NodeId {
		self.store.read().await.node_id()
	}

	pub async fn add_key(
		&self,
		group_pub_id: groups::PubId,
		key: SecretKey,
		rng: &mut CryptoRng,
	) -> Result<(), Error> {
		let mut store = self.store.write().await;
		store.add_key(group_pub_id, key);
		// Keeping the write lock here, this way we ensure that we can't corrupt the file
		store
			.encrypt(&self.master_key, rng, &self.keys_file_path)
			.await
	}

	pub async fn add_key_with_hash(
		&self,
		group_pub_id: groups::PubId,
		key: SecretKey,
		key_hash: KeyHash,
		rng: &mut CryptoRng,
	) -> Result<(), Error> {
		let mut store = self.store.write().await;
		store.add_key_with_hash(group_pub_id, key, key_hash);
		// Keeping the write lock here, this way we ensure that we can't corrupt the file
		store
			.encrypt(&self.master_key, rng, &self.keys_file_path)
			.await
	}

	pub async fn remove_group(
		&self,
		group_pub_id: groups::PubId,
		rng: &mut CryptoRng,
	) -> Result<(), Error> {
		let mut store = self.store.write().await;
		store.remove_group(group_pub_id);
		// Keeping the write lock here, this way we ensure that we can't corrupt the file
		store
			.encrypt(&self.master_key, rng, &self.keys_file_path)
			.await
	}

	pub async fn add_many_keys(
		&self,
		group_pub_id: groups::PubId,
		keys: impl IntoIterator<
				Item = SecretKey,
				IntoIter = impl DoubleEndedIterator<Item = SecretKey> + Send,
			> + Send,
		rng: &mut CryptoRng,
	) -> Result<(), Error> {
		let mut store = self.store.write().await;
		store.add_many_keys(group_pub_id, keys);
		// Keeping the write lock here, this way we ensure that we can't corrupt the file
		store
			.encrypt(&self.master_key, rng, &self.keys_file_path)
			.await
	}

	pub async fn get_latest_key(
		&self,
		group_pub_id: groups::PubId,
	) -> Option<(KeyHash, SecretKey)> {
		self.store.read().await.get_latest_key(group_pub_id)
	}

	pub async fn get_key(&self, group_pub_id: groups::PubId, hash: &KeyHash) -> Option<SecretKey> {
		self.store.read().await.get_key(group_pub_id, hash)
	}

	pub async fn get_group_keys(&self, group_pub_id: groups::PubId) -> Vec<SecretKey> {
		self.store.read().await.get_group_keys(group_pub_id)
	}
}

impl fmt::Debug for KeyManager {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("KeyManager")
			.field("master_key", &"[REDACTED]")
			.field("keys_file_path", &self.keys_file_path)
			.field("store", &"[REDACTED]")
			.finish()
	}
}
