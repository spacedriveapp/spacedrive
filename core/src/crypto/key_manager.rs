use std::io::BufReader;
use std::{path::PathBuf, sync::Arc};

use bincode::{Decode, Encode};
use dashmap::{DashMap, DashSet};
use sd_crypto::encoding;
use sd_crypto::types::{Algorithm, EncryptedKey, HashingAlgorithm, Key, Nonce, Salt};
use uuid::Uuid;

use super::Result;
use crate::prisma::PrismaClient;

pub struct KeyManager {
	key: Option<Key>,
	inner: DashMap<Uuid, Key>,
	queue: DashSet<Uuid>,
	db: Arc<PrismaClient>,
	config_path: PathBuf,
	config: Option<KeyManagerConfig>,
}

#[derive(Encode, Decode)]
pub struct RootKey {
	#[bincode(with_serde)]
	pub id: Uuid,
	pub hashing_algorithm: HashingAlgorithm,
	pub salt: Salt,
	pub key: EncryptedKey,
}

// Encrypted with the key manager's root key
// this means keys can't even begin to be mounted until the key manager is correctly unlocked
pub struct TestVector(
	// salt + root key = encrypted key decryption key
	Salt,
	// key for the actual test vector expected bytes
	EncryptedKey,
	Nonce,
	Vec<u8>,
);

#[derive(Encode, Decode)]
pub struct KeyManagerConfig {
	algorithm: Algorithm,
	root_keys: Vec<RootKey>,
}

pub struct EncryptedWord(Salt, Nonce, Vec<u8>);

pub struct KMKey {
	pub id: Uuid,
	pub hashing_algorithm: HashingAlgorithm,
	pub word: EncryptedWord, // word (once hashed with b3) acts like a salt
	pub tv: TestVector,
}

impl KeyManager {
	pub fn new(config_path: PathBuf, db: Arc<PrismaClient>) -> Self {
		let config = if std::fs::metadata(&config_path).is_ok() {
			let file = std::fs::File::open(config_path).unwrap();
			let mut reader = BufReader::new(file);
			Some(encoding::decode_from_reader(&mut reader).unwrap())
		} else {
			None
		};

		Self {
			key: None,
			inner: DashMap::new(),
			queue: DashSet::new(),
			db,
			config_path,
			config,
		}
	}

	pub fn unlocked(&self) -> bool {
		self.key.is_some()
	}

	pub fn unlock(&self, password: String, secret_key: String) -> Result<()> {
		todo!()
	}

	pub fn initial_setup(&self, algorithm: Algorithm, password: String) -> Result<String> {
		todo!()
	}
}
