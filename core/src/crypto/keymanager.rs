use std::path::PathBuf;
use std::sync::Arc;

use bincode::{Decode, Encode};
use dashmap::DashSet;
use sd_crypto::crypto::{Decryptor, Encryptor};
use sd_crypto::hashing::Hasher;
use sd_crypto::primitives::{BLOCK_LEN, SALT_LEN};
use sd_crypto::types::{
	Aad, Algorithm, EncryptedKey, HashingAlgorithm, Key, Nonce, Salt, SecretKey,
};
use sd_crypto::utils::generate_passphrase;
use sd_crypto::{encoding, Protected};
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::error::CryptoError;
use super::{Result, KEY_MOUNTING_CONTEXT, TEST_VECTOR_CONTEXT};
use crate::crypto::ENCRYPTED_WORD_CONTEXT;
use crate::prisma::{key, mounted_key, PrismaClient};

pub struct KeyManager {
	key: Mutex<Option<Key>>, // the root key
	_queue: DashSet<Uuid>,
	db: Arc<PrismaClient>,
}

#[derive(Clone, Encode, Decode)]
pub struct MountedKey {
	version: KeyVersion,
	#[bincode(with_serde)]
	uuid: Uuid,
	algorithm: Algorithm,
	salt: Salt,
	key: EncryptedKey,
}

impl MountedKey {
	pub fn encrypt(
		root_key: &Key,
		key: &Key,
		algorithm: Algorithm,
		word: &Protected<Vec<u8>>,
	) -> Result<Self> {
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);

		// TODO(brxken128): maybe give these separate contexts, or even remove the second derivation
		let ek = Encryptor::encrypt_key(
			&Hasher::derive_key(root_key, salt, KEY_MOUNTING_CONTEXT),
			&nonce,
			algorithm,
			&Hasher::derive_key(key, word_to_salt(word)?, KEY_MOUNTING_CONTEXT),
			Aad::Null,
		)?;

		Ok(Self {
			version: KeyVersion::V1,
			uuid: Uuid::new_v4(),
			algorithm,
			salt,
			key: ek,
		})
	}

	pub fn decrypt(&self, root_key: &Key) -> Result<Key> {
		Ok(Decryptor::decrypt_key(
			&Hasher::derive_key(root_key, self.salt, KEY_MOUNTING_CONTEXT),
			self.algorithm,
			&self.key,
			Aad::Null,
		)?)
	}
}

impl TryFrom<&MountedKey> for mounted_key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: &MountedKey) -> std::result::Result<Self, Self::Error> {
		#[allow(clippy::as_conversions)]
		let s = Self {
			version: value.version as i32,
			uuid: Uuid::new_v4().as_bytes().to_vec(), // random uuid to prevent conflicts
			algorithm: encoding::encode(&value.algorithm)?,
			key: encoding::encode(&value.key)?,
			salt: encoding::encode(&value.salt)?,
			_params: vec![],
		};

		Ok(s)
	}
}

impl TryFrom<MountedKey> for mounted_key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: MountedKey) -> std::result::Result<Self, Self::Error> {
		(&value).try_into()
	}
}

impl TryFrom<mounted_key::Data> for MountedKey {
	type Error = CryptoError;

	fn try_from(value: mounted_key::Data) -> std::result::Result<Self, Self::Error> {
		let mk = Self {
			version: KeyVersion::try_from(value.version)?,
			uuid: Uuid::from_slice(&value.uuid)?,
			algorithm: encoding::decode(&value.algorithm)?,
			key: encoding::decode(&value.key)?,
			salt: encoding::decode(&value.salt)?,
		};

		Ok(mk)
	}
}

#[derive(Clone, Encode, Decode)]
pub struct OnDiskBackup {
	root_keys: Vec<RootKey>,
	user_keys: Vec<UserKey>,
}

#[derive(Clone, Encode, Decode)]
pub struct TestVector(Salt, EncryptedKey);

#[derive(Encode, Decode, PartialEq, Eq)]
#[repr(i32)]
pub enum KeyType {
	Root = 0,
	User = 1,
}

impl TryFrom<i32> for KeyType {
	type Error = CryptoError;

	fn try_from(value: i32) -> std::result::Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::Root),
			1 => Ok(Self::User),
			_ => Err(CryptoError::Conversion),
		}
	}
}

#[derive(Clone, Copy, Encode, Decode)]
#[repr(i32)]
pub enum KeyVersion {
	V1 = 0,
}

impl TryFrom<i32> for KeyVersion {
	type Error = CryptoError;

	fn try_from(value: i32) -> std::result::Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::V1),
			_ => Err(CryptoError::Conversion),
		}
	}
}

#[derive(Clone, Encode, Decode)]
pub struct EncryptedWord(Salt, Nonce, Vec<u8>);

impl EncryptedWord {
	pub fn decrypt(&self, root_key: &Key, algorithm: Algorithm) -> Result<Protected<Vec<u8>>> {
		Decryptor::decrypt_tiny(
			&Hasher::derive_key(root_key, self.0, ENCRYPTED_WORD_CONTEXT),
			&self.1,
			algorithm,
			&self.2,
			Aad::Null,
		)
		.map_err(CryptoError::Crypto)
	}

	pub fn encrypt(
		root_key: &Key,
		word: &Protected<Vec<u8>>,
		algorithm: Algorithm,
	) -> Result<Self> {
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);
		let bytes = Encryptor::encrypt_tiny(
			&Hasher::derive_key(root_key, salt, ENCRYPTED_WORD_CONTEXT),
			&nonce,
			algorithm,
			word.expose(),
			Aad::Null,
		)?;

		Ok(Self(salt, nonce, bytes))
	}
}

key::select!(key_with_user_info { uuid name });

#[derive(Clone, Encode, Decode)]
pub struct UserKey {
	pub version: KeyVersion,
	#[bincode(with_serde)]
	pub uuid: Uuid,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
	pub word: EncryptedWord, // word (once hashed with b3) acts like a salt
	pub tv: TestVector,
}

fn word_to_salt(word: &Protected<Vec<u8>>) -> Result<Salt> {
	Salt::try_from(Hasher::blake3(word.expose()).expose()[..SALT_LEN].to_vec())
		.map_err(CryptoError::Crypto)
}

impl TryFrom<&UserKey> for key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: &UserKey) -> std::result::Result<Self, Self::Error> {
		#[allow(clippy::as_conversions)]
		let s = Self {
			uuid: value.uuid.as_bytes().to_vec(),
			version: value.version as i32,
			key_type: KeyType::User as i32,
			algorithm: encoding::encode(&value.algorithm)?,
			hashing_algorithm: encoding::encode(&value.hashing_algorithm)?,
			key: encoding::encode(&value.tv)?,
			salt: encoding::encode(&value.word)?,
			_params: vec![],
		};

		Ok(s)
	}
}

impl TryFrom<UserKey> for key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: UserKey) -> std::result::Result<Self, Self::Error> {
		(&value).try_into()
	}
}

impl TryFrom<key::Data> for UserKey {
	type Error = CryptoError;

	fn try_from(value: key::Data) -> std::result::Result<Self, Self::Error> {
		if KeyType::try_from(value.key_type)? != KeyType::User {
			return Err(CryptoError::Conversion);
		}

		let uk = Self {
			version: KeyVersion::try_from(value.version)?,
			uuid: Uuid::from_slice(&value.uuid)?,
			algorithm: encoding::decode(&value.algorithm)?,
			hashing_algorithm: encoding::decode(&value.hashing_algorithm)?,
			word: encoding::decode(&value.salt)?,
			tv: encoding::decode(&value.key)?,
		};

		Ok(uk)
	}
}

impl TestVector {
	pub fn validate(&self, algorithm: Algorithm, hashed_password: &Key) -> Result<()> {
		Decryptor::decrypt_key(
			&Hasher::derive_key(hashed_password, self.0, TEST_VECTOR_CONTEXT),
			algorithm,
			&self.1,
			Aad::Null,
		)
		.map_or(Err(CryptoError::IncorrectPassword), |_| Ok(()))
	}
}

impl KeyManager {
	pub fn new(db: Arc<PrismaClient>) -> Self {
		Self {
			key: Mutex::new(None),
			_queue: DashSet::new(),
			db,
		}
	}

	pub async fn is_unlocked(&self) -> bool {
		self.key.lock().await.is_some()
	}

	async fn get_root_key(&self) -> Result<Key> {
		self.key.lock().await.clone().ok_or(CryptoError::Locked)
	}

	async fn ensure_unlocked(&self) -> Result<()> {
		self.key
			.lock()
			.await
			.as_ref()
			.map_or(Err(CryptoError::Locked), |_| Ok(()))
	}

	pub async fn unlock(
		&self,
		password: Protected<String>,
		secret_key: Option<Protected<String>>,
	) -> Result<()> {
		let password: Protected<Vec<u8>> = password.into_inner().into_bytes().into();

		let secret_key: SecretKey = if let Some(secret_key) = secret_key {
			secret_key.try_into()?
		} else {
			// TODO(brxken128): source from keyring here, or return error if that fails
			SecretKey::generate()
		};

		#[allow(clippy::as_conversions)]
		let root_keys = self
			.db
			.key()
			.find_many(vec![key::key_type::equals(KeyType::Root as i32)])
			.exec()
			.await?;

		let root_keys = root_keys
			.into_iter()
			.map(RootKey::try_from)
			.collect::<Result<Vec<_>>>()?;

		let rk = root_keys
			.into_iter()
			.find_map(|k| {
				let pw = Hasher::hash_password(k.hashing_algorithm, &password, k.salt, &secret_key)
					.ok()?;

				Decryptor::decrypt_key(&pw, k.algorithm, &k.key, Aad::Null).ok()
			})
			.ok_or(CryptoError::Unlock)?;

		*self.key.lock().await = Some(rk);
		Ok(())
	}

	pub async fn initial_setup(
		&self,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		password: Protected<String>,
	) -> Result<Protected<String>> {
		let secret_key = SecretKey::generate();
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);
		let password = password.into_inner().into_bytes().into();

		let hashed_password =
			Hasher::hash_password(hashing_algorithm, &password, salt, &secret_key)?;

		let root_key = Key::generate();
		let root_key_e =
			Encryptor::encrypt_key(&hashed_password, &nonce, algorithm, &root_key, Aad::Null)?;

		let rk: key::CreateUnchecked = RootKey {
			version: KeyVersion::V1,
			uuid: Uuid::new_v4(),
			algorithm,
			hashing_algorithm,
			salt,
			key: root_key_e,
		}
		.try_into()?;

		rk.to_query(&self.db).exec().await?;

		*self.key.lock().await = Some(root_key);

		Ok(secret_key.to_string().into())
	}

	pub async fn delete(&self, uuid: Uuid) -> Result<()> {
		self.db
			.key()
			.delete(key::uuid::equals(uuid.as_bytes().to_vec()))
			.exec()
			.await
			.map_err(|_| CryptoError::KeyNotFound)?;

		Ok(())
	}

	pub async fn update_key_name(&self, uuid: Uuid, name: String) -> Result<()> {
		self.db
			.key()
			.update(
				key::uuid::equals(uuid.as_bytes().to_vec()),
				vec![key::name::set(Some(name))],
			)
			.exec()
			.await
			.map_or(Err(CryptoError::KeyNotFound), |_| Ok(()))
	}

	pub async fn insert_new(
		&self,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		password: Protected<String>,
		word: Option<Protected<String>>,
	) -> Result<Uuid> {
		self.ensure_unlocked().await?;

		word.as_ref().map(|w| {
			if w.expose().len() < 3 {
				Err(CryptoError::WordTooShort)
			} else {
				Ok(())
			}
		});

		let word: Protected<Vec<u8>> = word
			.map_or(
				generate_passphrase(1, '_').into_inner(),
				Protected::into_inner,
			)
			.into_bytes()
			.into();

		let tv_key = Key::generate();
		let tv_nonce = Nonce::generate(algorithm);
		let tv_salt = Salt::generate();

		let hashed_password = Hasher::hash_password(
			hashing_algorithm,
			&password.into_inner().into_bytes().into(),
			word_to_salt(&word)?,
			&SecretKey::Null,
		)?;

		let tv_key = Encryptor::encrypt_key(
			&Hasher::derive_key(&hashed_password, tv_salt, TEST_VECTOR_CONTEXT),
			&tv_nonce,
			algorithm,
			&tv_key,
			Aad::Null,
		)?;

		let ew = EncryptedWord::encrypt(&self.get_root_key().await?, &word, algorithm)?;

		let uuid = Uuid::new_v4();

		let key: key::CreateUnchecked = UserKey {
			version: KeyVersion::V1,
			uuid,
			algorithm,
			hashing_algorithm,
			tv: TestVector(tv_salt, tv_key),
			word: ew,
		}
		.try_into()?;

		key.to_query(&self.db).exec().await?;

		let mk = MountedKey::encrypt(
			&self.get_root_key().await?,
			&hashed_password,
			algorithm,
			&word,
		)?;

		let mkc: mounted_key::CreateUnchecked = mk.try_into()?;
		let mk_uuid = mkc.uuid.clone();

		mkc.to_query(&self.db).exec().await?;

		self.db
			.mounted_key()
			.update(
				mounted_key::uuid::equals(mk_uuid),
				vec![mounted_key::SetParam::ConnectAssociatedKey(
					key::uuid::equals(uuid.as_bytes().to_vec()),
				)],
			)
			.exec()
			.await?;

		Ok(uuid)
	}

	pub async fn list(&self) -> Result<Vec<UserKey>> {
		#[allow(clippy::as_conversions)]
		self.db
			.key()
			.find_many(vec![key::key_type::equals(KeyType::User as i32)])
			.exec()
			.await?
			.into_iter()
			.map(UserKey::try_from)
			.collect()
	}

	pub async fn list_mounted(&self) -> Result<Vec<Uuid>> {
		self.ensure_unlocked().await?;

		self.db
			.mounted_key()
			.find_many(vec![])
			.exec()
			.await?
			.iter()
			.map(|x| Uuid::from_slice(&x.uuid))
			.collect::<std::result::Result<_, _>>()
			.map_err(|_| CryptoError::Conversion)
	}

	pub async fn mount(&self, uuid: Uuid, password: Protected<String>) -> Result<()> {
		self.ensure_unlocked().await?;
		// handle the queue

		self.db
			.key()
			.find_unique(key::uuid::equals(uuid.as_bytes().to_vec()))
			.select(key::select!({ mounted_key }))
			.exec()
			.await?
			.ok_or(CryptoError::KeyNotFound)?
			.mounted_key
			.map_or(Ok(()), |_| Err(CryptoError::AlreadyMounted))?;

		let key = self
			.db
			.key()
			.find_unique(key::uuid::equals(uuid.as_bytes().to_vec()))
			.exec()
			.await?
			.ok_or(CryptoError::KeyNotFound)?;

		let key = UserKey::try_from(key)?;

		let word = key
			.word
			.decrypt(&self.get_root_key().await?, key.algorithm)?;

		let hashed_password = Hasher::hash_password(
			key.hashing_algorithm,
			&password.into_inner().into_bytes().into(),
			word_to_salt(&word)?,
			&SecretKey::Null,
		)?;

		key.tv.validate(key.algorithm, &hashed_password)?;

		let mk = MountedKey::encrypt(
			&self.get_root_key().await?,
			&hashed_password,
			key.algorithm,
			&word,
		)?;

		let mkc: mounted_key::CreateUnchecked = mk.try_into()?;
		let mk_uuid = mkc.uuid.clone();

		mkc.to_query(&self.db)
			// .with()
			.exec()
			.await?;

		self.db
			.mounted_key()
			.update(
				mounted_key::uuid::equals(mk_uuid),
				vec![mounted_key::SetParam::ConnectAssociatedKey(
					key::uuid::equals(uuid.as_bytes().to_vec()),
				)],
			)
			.exec()
			.await?;

		Ok(())
	}

	pub async fn enumerate_user_keys(&self) -> Result<Vec<key_with_user_info::Data>> {
		self.ensure_unlocked().await?;

		#[allow(clippy::as_conversions)]
		Ok(self
			.db
			.key()
			.find_many(vec![key::key_type::equals(KeyType::User as i32)])
			.select(key_with_user_info::select())
			.exec()
			.await?)
	}

	pub async fn unmount(&self, uuid: Uuid) -> Result<()> {
		if self
			.db
			.mounted_key()
			.delete_many(vec![mounted_key::associated_key::is(vec![
				key::uuid::equals(uuid.as_bytes().to_vec()),
			])])
			.exec()
			.await? == 1
		{
			Ok(())
		} else {
			Err(CryptoError::KeyNotFound)
		}
	}

	pub async fn unmount_all(&self) -> Result<i64> {
		Ok(self.db.mounted_key().delete_many(vec![]).exec().await?)
	}

	pub async fn lock(&self) -> Result<()> {
		self.ensure_unlocked().await?;
		*self.key.lock().await = None;

		Ok(())
	}

	pub async fn get_key(&self, uuid: Uuid) -> Result<Key> {
		self.ensure_unlocked().await?;

		let key = self
			.db
			.key()
			.find_unique(key::uuid::equals(uuid.as_bytes().to_vec()))
			.select(key::select!({ mounted_key }))
			.exec()
			.await?
			.ok_or(CryptoError::KeyNotFound)?
			.mounted_key
			.map_or(Err(CryptoError::NotMounted), MountedKey::try_from)?;

		key.decrypt(&self.get_root_key().await?)
	}

	pub async fn enumerate_hashed_keys(&self) -> Result<Vec<Key>> {
		self.ensure_unlocked().await?;

		let rk = self.get_root_key().await?;

		self.db
			.mounted_key()
			.find_many(vec![])
			.exec()
			.await?
			.into_iter()
			.flat_map(MountedKey::try_from)
			.map(|x| x.decrypt(&rk))
			.collect()
	}

	pub async fn backup_to_file(&self, path: PathBuf) -> Result<()> {
		if fs::metadata(&path).await.is_ok() {
			return Err(CryptoError::FileAlreadyExists);
		}

		#[allow(clippy::as_conversions)]
		let user_keys = self
			.db
			.key()
			.find_many(vec![key::key_type::equals(KeyType::User as i32)])
			.exec()
			.await?
			.into_iter()
			.map(UserKey::try_from)
			.collect::<Result<Vec<_>>>()?;

		#[allow(clippy::as_conversions)]
		let root_keys = self
			.db
			.key()
			.find_many(vec![key::key_type::equals(KeyType::Root as i32)])
			.exec()
			.await?
			.into_iter()
			.map(RootKey::try_from)
			.collect::<Result<Vec<_>>>()?;

		let backup = OnDiskBackup {
			root_keys,
			user_keys,
		};

		let mut file = File::create(&path).await?;
		file.write_all(&encoding::encode(&backup)?).await?;

		Ok(())
	}

	pub async fn restore_from_file(
		&self,
		path: PathBuf,
		password: Protected<String>,
		secret_key: Protected<String>,
	) -> Result<i64> {
		let file_len: usize = fs::metadata(&path)
			.await
			.map_or(Err(CryptoError::FileDoesntExist), |x| {
				x.len().try_into().map_err(|_| CryptoError::Conversion)
			})?;

		if file_len > (BLOCK_LEN * 16) {
			return Err(CryptoError::FileTooLarge);
		}

		let mut bytes = vec![0u8; file_len];
		let mut file = File::open(&path).await?;
		file.read_to_end(&mut bytes).await?;

		let backup: OnDiskBackup = encoding::decode(&bytes)?;

		let password: Protected<Vec<u8>> = password.into_inner().into_bytes().into();
		let secret_key = secret_key.try_into()?;

		let backup_rk = backup
			.root_keys
			.into_iter()
			.find_map(|k| {
				let pw = Hasher::hash_password(k.hashing_algorithm, &password, k.salt, &secret_key)
					.ok()?;

				Decryptor::decrypt_key(&pw, k.algorithm, &k.key, Aad::Null).ok()
			})
			.ok_or(CryptoError::IncorrectPassword)?;

		let rk = self.get_root_key().await?;

		let user_keys = backup
			.user_keys
			.into_iter()
			.map(|mut key| {
				let word = key.word.decrypt(&backup_rk, key.algorithm)?;
				key.word = EncryptedWord::encrypt(&rk, &word, key.algorithm)?;

				key.try_into()
			})
			.collect::<Result<Vec<key::CreateUnchecked>>>()?;

		Ok(self
			.db
			.key()
			.create_many(user_keys)
			.skip_duplicates()
			.exec()
			.await?)
	}
}

#[derive(Clone, Encode, Decode)]
pub struct RootKey {
	pub version: KeyVersion,
	#[bincode(with_serde)]
	pub uuid: Uuid,
	pub algorithm: Algorithm,
	pub hashing_algorithm: HashingAlgorithm,
	pub salt: Salt,
	pub key: EncryptedKey,
}

impl TryFrom<key::Data> for RootKey {
	type Error = CryptoError;

	fn try_from(value: key::Data) -> std::result::Result<Self, Self::Error> {
		if KeyType::try_from(value.key_type)? != KeyType::User {
			return Err(CryptoError::Conversion);
		}

		let rk = Self {
			version: KeyVersion::try_from(value.version)?,
			uuid: Uuid::from_slice(&value.uuid)?,
			algorithm: encoding::decode(&value.algorithm)?,
			hashing_algorithm: encoding::decode(&value.hashing_algorithm)?,
			key: encoding::decode(&value.key)?,
			salt: encoding::decode(&value.salt)?,
		};

		Ok(rk)
	}
}

impl TryFrom<&RootKey> for key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: &RootKey) -> std::result::Result<Self, Self::Error> {
		#[allow(clippy::as_conversions)]
		let s = Self {
			uuid: value.uuid.as_bytes().to_vec(),
			version: value.version as i32,
			key_type: KeyType::Root as i32,
			algorithm: encoding::encode(&value.algorithm)?,
			hashing_algorithm: encoding::encode(&value.hashing_algorithm)?,
			key: encoding::encode(&value.key)?,
			salt: encoding::encode(&value.salt)?,
			_params: vec![],
		};

		Ok(s)
	}
}

impl TryFrom<RootKey> for key::CreateUnchecked {
	type Error = CryptoError;

	fn try_from(value: RootKey) -> std::result::Result<Self, Self::Error> {
		(&value).try_into()
	}
}
