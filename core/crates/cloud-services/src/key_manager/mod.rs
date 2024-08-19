use crate::Error;

use sd_cloud_schema::sync::KeyHash;
use sd_crypto::{cloud::secret_key::SecretKey, CryptoRng};
use sd_utils::error::FileIOError;

use std::{
	fmt,
	path::{Path, PathBuf},
};

use iroh_base::key::{NodeId, SecretKey as IrohSecretKey};
use tokio::{fs, io, sync::RwLock};

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
		data_directory: impl AsRef<Path> + Send,
		rng: &mut CryptoRng,
	) -> Result<Self, Error> {
		async fn inner(
			master_key: SecretKey,
			keys_file_path: PathBuf,
			rng: &mut CryptoRng,
		) -> Result<KeyManager, Error> {
			let store = match fs::metadata(&keys_file_path).await {
				Ok(metadata) => KeyStore::decrypt(&master_key, metadata, &keys_file_path).await?,

				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					// File not found, so we create a new one
					let store = KeyStore::new(rng);
					store.encrypt(&master_key, rng, &keys_file_path).await?;
					store
				}

				Err(e) => {
					return Err(FileIOError::from((
						keys_file_path,
						e,
						"Failed to read space keys file",
					))
					.into());
				}
			};

			Ok(KeyManager {
				master_key,
				keys_file_path,
				store: RwLock::new(store),
			})
		}

		inner(master_key, data_directory.as_ref().join(KEY_FILE_NAME), rng).await
	}

	pub async fn iroh_secret_key(&self) -> IrohSecretKey {
		self.store.read().await.iroh_secret_key()
	}

	pub async fn node_id(&self) -> NodeId {
		self.store.read().await.node_id()
	}

	pub async fn add_key(&self, key: SecretKey, rng: &mut CryptoRng) -> Result<(), Error> {
		let mut store = self.store.write().await;
		store.add_key(key);
		// Keeping the write lock here, this way we ensure that we can't corrupt the file
		store
			.encrypt(&self.master_key, rng, &self.keys_file_path)
			.await
	}

	pub async fn get_key(&self, hash: &KeyHash) -> Option<SecretKey> {
		self.store.read().await.get_key(hash)
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
