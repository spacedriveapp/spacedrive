use crate::Error;

use sd_cloud_schema::sync::KeyHash;
use sd_crypto::{
	cloud::{decrypt, encrypt, secret_key::SecretKey},
	primitives::{EncryptedBlock, OneShotNonce, StreamNonce},
	CryptoRng,
};
use sd_utils::error::FileIOError;

use std::{collections::HashMap, fs::Metadata, path::PathBuf, pin::pin};

use futures::StreamExt;
use iroh_base::key::{NodeId, SecretKey as IrohSecretKey};
use serde::{Deserialize, Serialize};
use tokio::{
	fs,
	io::{AsyncReadExt, AsyncWriteExt, BufWriter},
};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Serialize, Deserialize)]
pub struct KeyStore {
	iroh_secret_key: IrohSecretKey,
	keys_by_hash: HashMap<KeyHash, SecretKey>,
}

impl KeyStore {
	pub fn new(iroh_secret_key: IrohSecretKey) -> Self {
		Self {
			iroh_secret_key,
			keys_by_hash: HashMap::new(),
		}
	}

	pub fn add_key(&mut self, key: SecretKey) {
		let mut hasher = blake3::Hasher::new();
		hasher.update(key.as_ref());
		let hash = hasher.finalize();

		self.keys_by_hash
			.insert(KeyHash(hash.to_hex().to_string()), key);
	}

	pub fn iroh_secret_key(&self) -> IrohSecretKey {
		self.iroh_secret_key.clone()
	}

	pub fn node_id(&self) -> NodeId {
		self.iroh_secret_key.public()
	}

	pub fn get_key(&self, hash: &KeyHash) -> Option<SecretKey> {
		self.keys_by_hash.get(hash).cloned()
	}

	pub async fn encrypt(
		&self,
		key: &SecretKey,
		rng: &mut CryptoRng,
		keys_file_path: &PathBuf,
	) -> Result<(), Error> {
		let plain_text_bytes = postcard::to_stdvec(self)?;
		let mut file = BufWriter::with_capacity(
			EncryptedBlock::CIPHER_TEXT_SIZE,
			fs::OpenOptions::new()
				.create(true)
				.write(true)
				.truncate(true)
				.open(&keys_file_path)
				.await
				.map_err(|e| {
					FileIOError::from((
						&keys_file_path,
						e,
						"Failed to open space keys file to encrypt",
					))
				})?,
		);

		if plain_text_bytes.len() < EncryptedBlock::PLAIN_TEXT_SIZE {
			use encrypt::OneShotEncryption;

			let EncryptedBlock { nonce, cipher_text } = key
				.encrypt(&plain_text_bytes, rng)
				.map_err(|e| Error::KeyStoreCrypto {
					source: e,
					context: "Failed to oneshot encrypt key store",
				})?;

			file.write_all(nonce.as_slice()).await.map_err(|e| {
				FileIOError::from((
					&keys_file_path,
					e,
					"Failed to write space keys file oneshot nonce",
				))
			})?;

			file.write_all(cipher_text.as_slice()).await.map_err(|e| {
				FileIOError::from((
					&keys_file_path,
					e,
					"Failed to write space keys file oneshot cipher text",
				))
			})?;
		} else {
			use encrypt::StreamEncryption;

			let (nonce, stream) = key.encrypt(plain_text_bytes.as_slice(), rng);

			file.write_all(nonce.as_slice()).await.map_err(|e| {
				FileIOError::from((
					&keys_file_path,
					e,
					"Failed to write space keys file stream nonce",
				))
			})?;

			let mut stream = pin!(stream);
			while let Some(res) = stream.next().await {
				file.write_all(&res.map_err(|e| Error::KeyStoreCrypto {
					source: e,
					context: "Failed to stream encrypt key store",
				})?)
				.await
				.map_err(|e| {
					FileIOError::from((
						&keys_file_path,
						e,
						"Failed to write space keys file stream cipher text",
					))
				})?;
			}
		};

		file.flush().await.map_err(|e| {
			FileIOError::from((&keys_file_path, e, "Failed to flush space keys file")).into()
		})
	}

	pub async fn decrypt(
		key: &SecretKey,
		metadata: Metadata,
		keys_file_path: &PathBuf,
	) -> Result<Self, Error> {
		let mut file = fs::File::open(&keys_file_path).await.map_err(|e| {
			FileIOError::from((
				keys_file_path,
				e,
				"Failed to open space keys file to decrypt",
			))
		})?;

		let usize_file_len =
			usize::try_from(metadata.len()).expect("Failed to convert metadata length to usize");

		postcard::from_bytes(&if usize_file_len
			<= EncryptedBlock::CIPHER_TEXT_SIZE + size_of::<OneShotNonce>()
		{
			use decrypt::OneShotDecryption;

			let mut nonce = OneShotNonce::default();

			file.read_exact(&mut nonce).await.map_err(|e| {
				FileIOError::from((
					keys_file_path,
					e,
					"Failed to read space keys file oneshot nonce",
				))
			})?;

			let mut cipher_text = vec![0u8; usize_file_len - size_of::<OneShotNonce>()];

			file.read_exact(&mut cipher_text).await.map_err(|e| {
				FileIOError::from((
					keys_file_path,
					e,
					"Failed to read space keys file oneshot cipher text",
				))
			})?;

			key.decrypt(&EncryptedBlock { nonce, cipher_text })
				.map_err(|e| Error::KeyStoreCrypto {
					source: e,
					context: "Failed to oneshot decrypt space keys file",
				})?
		} else {
			use decrypt::StreamDecryption;

			let mut nonce = StreamNonce::default();

			let mut key_store_bytes = Vec::with_capacity(
				(usize_file_len - size_of::<StreamNonce>()) / EncryptedBlock::CIPHER_TEXT_SIZE
					* EncryptedBlock::PLAIN_TEXT_SIZE,
			);

			file.read_exact(&mut nonce).await.map_err(|e| {
				FileIOError::from((
					keys_file_path,
					e,
					"Failed to read space keys file stream nonce",
				))
			})?;

			key.decrypt(&nonce, &mut file, &mut key_store_bytes)
				.await
				.map_err(|e| Error::KeyStoreCrypto {
					source: e,
					context: "Failed to stream decrypt space keys file",
				})?;

			key_store_bytes
		})
		.map_err(Into::into)
	}
}

/// Zeroize our secret keys and scrambles up iroh's secret key that doesn't implement zeroize
impl Zeroize for KeyStore {
	fn zeroize(&mut self) {
		self.iroh_secret_key = IrohSecretKey::generate();
		self.keys_by_hash.values_mut().for_each(Zeroize::zeroize);
		self.keys_by_hash = HashMap::new();
	}
}

impl Drop for KeyStore {
	fn drop(&mut self) {
		self.zeroize();
	}
}

impl ZeroizeOnDrop for KeyStore {}
