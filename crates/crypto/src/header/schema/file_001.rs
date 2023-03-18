use bincode::impl_borrow_decode;

use crate::{
	crypto::{Decryptor, Encryptor},
	encoding,
	header::file::{Header, HeaderObjectName},
	keys::hashing::PasswordHasher,
	primitives::{generate_byte_array, to_array},
	types::{
		Aad, Algorithm, DerivationContext, EncryptedKey, HashingAlgorithm, Key, Nonce, Params, Salt,
	},
	Error, Protected, Result,
};

const KEYSLOT_LIMIT: usize = 2;
const OBJECT_LIMIT: usize = 2;

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct FileHeader001 {
	pub aad: Aad,
	pub algorithm: Algorithm,
	pub nonce: Nonce,
	pub keyslots: KeyslotArea001,
	pub objects: Vec<FileHeaderObject001>,
}

/// A keyslot - 96 bytes (as of V1), and contains all the information for future-proofing while keeping the size reasonable
#[derive(bincode::Encode, bincode::Decode, Clone)]
pub struct Keyslot001 {
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub salt: Salt, // the salt used for deriving a KEK from a (key/content salt) hash
	pub content_salt: Salt,
	pub encrypted_key: EncryptedKey, // encrypted
	pub nonce: Nonce,
}

impl Keyslot001 {
	pub fn random() -> Self {
		Self {
			content_salt: Salt::generate(),
			hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
			encrypted_key: EncryptedKey(generate_byte_array()),
			salt: Salt::generate(),
			nonce: Nonce::default(),
		}
	}
}

/// We use this without encode/decode traits so we can map/wrap it to a `KeyslotAreaBundle`
#[derive(Clone)]
pub struct KeyslotArea001(Vec<Keyslot001>);

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct KeyslotBundle001 {
	pub enabled: bool,
	pub keyslot: Keyslot001,
}

#[derive(bincode::Encode, bincode::Decode)]
pub struct KeyslotAreaBundle001 {
	pub bundles: [KeyslotBundle001; KEYSLOT_LIMIT],
}

impl bincode::Decode for KeyslotArea001 {
	fn decode<D: bincode::de::Decoder>(
		decoder: &mut D,
	) -> std::result::Result<Self, bincode::error::DecodeError> {
		let bundle: KeyslotAreaBundle001 =
			bincode::decode_from_reader(decoder.reader(), bincode::config::standard())?;

		Ok(Self(bundle.into()))
	}
}

impl_borrow_decode!(KeyslotArea001);

impl bincode::Encode for KeyslotArea001 {
	fn encode<E: bincode::enc::Encoder>(
		&self,
		encoder: &mut E,
	) -> std::result::Result<(), bincode::error::EncodeError> {
		if self.0.len() > KEYSLOT_LIMIT {
			return Err(Error::TooManyKeyslots)?;
		}

		let mut bundle = vec![];

		// if it's an actual keyslot, mark it as enabled
		self.0.iter().for_each(|k| {
			bundle.push(KeyslotBundle001 {
				enabled: true,
				keyslot: k.clone(),
			});
		});

		// if it's an "empty space" keyslot, mark it as disabled
		(0..KEYSLOT_LIMIT - self.0.len()).for_each(|_| {
			bundle.push(KeyslotBundle001 {
				enabled: false,
				keyslot: Keyslot001::random(),
			});
		});

		KeyslotAreaBundle001::try_from(bundle)?.encode(encoder)?;

		Ok(())
	}
}

impl From<KeyslotAreaBundle001> for Vec<Keyslot001> {
	fn from(value: KeyslotAreaBundle001) -> Self {
		value
			.bundles
			.into_iter()
			.filter_map(|x| x.enabled.then_some(x.keyslot))
			.collect()
	}
}

impl TryFrom<Vec<KeyslotBundle001>> for KeyslotAreaBundle001 {
	type Error = Error;

	fn try_from(value: Vec<KeyslotBundle001>) -> std::result::Result<Self, Self::Error> {
		Ok(Self {
			bundles: value.try_into().map_err(|_| Error::LengthMismatch)?,
		})
	}
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct FileHeaderObject001 {
	pub identifier: HeaderObjectIdentifier,
	pub nonce: Nonce,
	pub data: Vec<u8>,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct HeaderObjectIdentifier {
	key: EncryptedKey,
	salt: Salt,
	nonce: Nonce,
}

impl HeaderObjectIdentifier {
	pub fn new(
		name: HeaderObjectName,
		master_key: Key,
		algorithm: Algorithm,
		context: DerivationContext,
		aad: Aad,
	) -> Result<Self> {
		let salt = Salt::generate();
		let nonce = Nonce::generate(algorithm);

		// it's encrypted so we probably don't need to hash it further/with anything else
		// this hash was only so we could 100% predict the length
		let name_hash = blake3::hash(name.inner());

		// encrypt the object name's hash with the master key
		let encrypted_key = Encryptor::encrypt_byte_array(
			Key::derive(master_key, salt, context),
			nonce,
			algorithm,
			name_hash.as_bytes(),
			aad.inner(),
		)
		.map(EncryptedKey::new)?;

		Ok(Self {
			key: encrypted_key,
			salt,
			nonce,
		})
	}

	pub fn eq(
		&self,
		other: HeaderObjectName,
		master_key: Key,
		algorithm: Algorithm,
		context: DerivationContext,
		aad: Aad,
	) -> Result<bool> {
		let source = self.decrypt_hash(master_key, algorithm, context, aad)?;
		let rhs = blake3::hash(other.inner());
		Ok(source.eq(&rhs))
	}

	fn decrypt_hash(
		&self,
		master_key: Key,
		algorithm: Algorithm,
		context: DerivationContext,
		aad: Aad,
	) -> Result<blake3::Hash> {
		to_array(
			Decryptor::decrypt_bytes(
				Key::derive(master_key, self.salt, context),
				self.nonce,
				algorithm,
				self.key.inner(),
				aad.inner(),
			)?
			.expose(),
		)
		.map_err(|_| Error::Decrypt)
		.map(blake3::Hash::from)
	}
}

impl Keyslot001 {
	#[allow(clippy::needless_pass_by_value)]
	pub fn new(
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_key: Key,
		master_key: Key,
		aad: Aad,
		context: DerivationContext,
	) -> Result<Self> {
		let nonce = Nonce::generate(algorithm);
		let salt = Salt::generate();

		let encrypted_key = Encryptor::encrypt_byte_array(
			Key::derive(hashed_key, salt, context),
			nonce,
			algorithm,
			master_key.expose(),
			aad.inner(),
		)
		.map(EncryptedKey::new)?;

		Ok(Self {
			hashing_algorithm,
			salt,
			content_salt,
			encrypted_key,
			nonce,
		})
	}

	fn decrypt(
		&self,
		algorithm: Algorithm,
		key: Key,
		aad: Aad,
		context: DerivationContext,
	) -> Result<Key> {
		Key::try_from(Decryptor::decrypt_bytes(
			Key::derive(key, self.salt, context),
			self.nonce,
			algorithm,
			self.encrypted_key.inner(),
			aad.inner(),
		)?)
	}
}

impl FileHeader001 {
	// TODO(brxken128): make the AAD not static
	// should be brought in from the raw file bytes but bincode makes that harder
	// as the first 32~ bytes of the file *may* change
	pub fn new(algorithm: Algorithm) -> Self {
		Self {
			aad: Aad::generate(),
			algorithm,
			nonce: Nonce::generate(algorithm),
			keyslots: KeyslotArea001(vec![]),
			objects: vec![],
		}
	}
}

impl FileHeaderObject001 {
	pub fn new(
		name: HeaderObjectName,
		algorithm: Algorithm,
		master_key: Key,
		context: DerivationContext,
		aad: Aad,
		data: &[u8],
	) -> Result<Self> {
		let identifier =
			HeaderObjectIdentifier::new(name, master_key.clone(), algorithm, context, aad)?;

		let nonce = Nonce::generate(algorithm);
		let encrypted_data =
			Encryptor::encrypt_bytes(master_key, nonce, algorithm, data, aad.inner())?;

		let object = Self {
			identifier,
			nonce,
			data: encrypted_data,
		};

		Ok(object)
	}

	fn decrypt(
		&self,
		algorithm: Algorithm,
		aad: Aad,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		Decryptor::decrypt_bytes(master_key, self.nonce, algorithm, &self.data, aad.inner())
	}
}

impl Header for FileHeader001 {
	fn serialize(&self) -> Result<Vec<u8>> {
		encoding::encode(self)
	}

	fn decrypt_object(
		&self,
		name: HeaderObjectName,
		context: DerivationContext,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		self.objects
			.iter()
			.filter_map(|o| {
				o.identifier
					.eq(
						name.clone(),
						master_key.clone(),
						self.algorithm,
						context,
						self.aad,
					)
					.ok()
					.and_then(|z| z.then_some(o))
			})
			.cloned()
			.collect::<Vec<_>>()
			.first()
			.ok_or(Error::NoObjects)?
			.decrypt(self.algorithm, self.aad, master_key)
	}

	fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_key: Key,
		master_key: Key,
		context: DerivationContext,
	) -> Result<()> {
		if self.keyslots.0.len() + 1 > KEYSLOT_LIMIT {
			return Err(Error::TooManyKeyslots);
		}

		self.keyslots.0.push(Keyslot001::new(
			self.algorithm,
			hashing_algorithm,
			content_salt,
			hashed_key,
			master_key,
			self.aad,
			context,
		)?);

		Ok(())
	}

	fn add_object(
		&mut self,
		name: HeaderObjectName,
		context: DerivationContext,
		master_key: Key,
		data: &[u8],
	) -> Result<()> {
		if self.objects.len() + 1 > OBJECT_LIMIT {
			return Err(Error::TooManyObjects);
		}

		if self
			.objects
			.iter()
			.flat_map(|o| {
				o.identifier.eq(
					name.clone(),
					master_key.clone(),
					self.algorithm,
					context,
					self.aad,
				)
			})
			.any(|x| x)
		{
			return Err(Error::DuplicateObjects);
		}

		self.objects.push(FileHeaderObject001::new(
			name,
			self.algorithm,
			master_key,
			context,
			self.aad,
			data,
		)?);
		Ok(())
	}

	#[allow(clippy::needless_pass_by_value)]
	fn decrypt_master_key(&self, keys: Vec<Key>, context: DerivationContext) -> Result<Key> {
		if self.keyslots.0.is_empty() {
			return Err(Error::NoKeyslots);
		}

		keys.iter()
			.find_map(|k| {
				self.keyslots
					.0
					.iter()
					.find_map(|z| z.decrypt(self.algorithm, k.clone(), self.aad, context).ok())
			})
			.ok_or(Error::IncorrectPassword)
	}

	#[allow(clippy::needless_pass_by_value)]
	fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
		context: DerivationContext,
	) -> Result<Key> {
		if self.keyslots.0.is_empty() {
			return Err(Error::NoKeyslots);
		}

		self.keyslots
			.0
			.iter()
			.find_map(|z| {
				let k = PasswordHasher::hash(
					z.hashing_algorithm,
					password.clone(),
					z.content_salt,
					None,
				)
				.ok()?;
				z.decrypt(self.algorithm, k, self.aad, context).ok()
			})
			.ok_or(Error::IncorrectPassword)
	}

	fn get_aad(&self) -> Aad {
		self.aad
	}

	fn get_nonce(&self) -> Nonce {
		self.nonce
	}

	fn get_algorithm(&self) -> Algorithm {
		self.algorithm
	}

	fn count_objects(&self) -> usize {
		self.objects.len()
	}

	fn count_keyslots(&self) -> usize {
		self.keyslots.0.len()
	}
}
