use std::io::{Read, Seek};

use crate::{
	error::Error,
	objects::stream::{StreamDecryption, StreamEncryption},
	primitives::{Algorithm, HashingAlgorithm, ENCRYPTED_MASTER_KEY_LEN, MASTER_KEY_LEN, SALT_LEN, generate_nonce, generate_master_key, to_array},
	protected::Protected,
};

/// This is a metadata header item. You may add it to a header, and this will be stored with the file.
///
/// The `Metadata::new()` function handles master key and metadata encryption.
///
/// The salt should be generated elsewhere (e.g. a key management system).
#[derive(Clone)]
pub struct Metadata {
	pub version: MetadataVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub salt: [u8; SALT_LEN],
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN],
	pub master_key_nonce: Vec<u8>,
	pub metadata_nonce: Vec<u8>,
	pub metadata_length: usize,
	pub metadata: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum MetadataVersion {
	V1,
}

impl Metadata {
	#[must_use]
	/// This handles encrypting the master key and encrypting the metadata.
	/// 
	/// You will need to provide the user's password/key, and a semi-universal salt for hashing the user's password. This allows for extremely fast decryption.
	pub fn new<T>(
		version: MetadataVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		password: Protected<Vec<u8>>,
		salt: [u8; SALT_LEN],
		media: &T,
	) -> Result<Self, Error> where T: ?Sized + serde::Serialize {
		let metadata_nonce = generate_nonce(algorithm.nonce_len());
		let master_key_nonce = generate_nonce(algorithm.nonce_len());
		let master_key = generate_master_key();

		let hashed_password = hashing_algorithm.hash(password, salt)?;

		let encrypted_master_key: [u8; 48] = to_array(
			StreamEncryption::encrypt_bytes(
				hashed_password,
				&master_key_nonce,
				algorithm,
				master_key.expose(),
				&[],
			)?,
		)?;

		let encrypted_metadata = StreamEncryption::encrypt_bytes(master_key, &metadata_nonce, algorithm, &serde_json::to_vec(media).map_err(|_| Error::MetadataDeSerialization)?, &[])?;

		Ok(Self {
			version,
			algorithm,
			hashing_algorithm,
			salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			metadata_nonce,
			metadata_length: encrypted_metadata.len(),
			metadata: encrypted_metadata,
		})
	}

	/// This function is used to serialize a metadata item into bytes
	///
	/// This also includes the encrypted metadata itself, so this may be sizeable
	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			MetadataVersion::V1 => {
				let mut metadata: Vec<u8> = Vec::new();
				metadata.extend_from_slice(&self.version.serialize()); // 2
				metadata.extend_from_slice(&self.algorithm.serialize()); // 4
				metadata.extend_from_slice(&self.hashing_algorithm.serialize()); // 6
				metadata.extend_from_slice(&self.salt); // 22
				metadata.extend_from_slice(&self.master_key); // 70
				metadata.extend_from_slice(&self.master_key_nonce); // 82 or 94
				metadata.extend_from_slice(&vec![0u8; 26 - self.master_key_nonce.len()]); // 96
				metadata.extend_from_slice(&self.metadata_nonce); // 108 or 120
				metadata.extend_from_slice(&vec![0u8; 24 - self.metadata_nonce.len()]); // 120
				metadata.extend_from_slice(&self.metadata_length.to_le_bytes()); // 128 total bytes
				metadata.extend_from_slice(&self.metadata); // this can vary in length
				metadata
			}
		}
	}

	/// This function is what you'll want to use to get the metadata for a file
	///
	/// All it requires is a hashed key, encrypted with the metadata "master salt"
	///
	/// A deserialized data type will be returned from this function
	pub fn decrypt_metadata<T>(&self, hashed_key: Protected<[u8; 32]>) -> Result<T, Error> where T: serde::de::DeserializeOwned {
		let mut master_key = [0u8; MASTER_KEY_LEN];

		let master_key = if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
			hashed_key,
			&self.master_key_nonce,
			self.algorithm,
			&self.master_key,
			&[],
		) {
			master_key.copy_from_slice(&decrypted_master_key);
			Ok(Protected::new(master_key))
		} else {
			Err(Error::IncorrectPassword)
		}?;

		let metadata = StreamDecryption::decrypt_bytes(master_key, &self.metadata_nonce, self.algorithm, &self.metadata, &[])?;

		serde_json::from_slice::<T>(&metadata).map_err(|_| Error::MetadataDeSerialization)
	}

	/// This function reads a metadata header item from a reader
	///
	/// The cursor will be left at the end of the metadata item on success
	///
	/// The cursor will not be rewound on error.
	pub fn deserialize<R>(reader: &mut R) -> Result<Self, Error>
	where
		R: Read + Seek,
	{
		let mut version = [0u8; 2];
		reader.read(&mut version).map_err(Error::Io)?;
		let version =
			MetadataVersion::deserialize(version).map_err(|_| Error::NoMetadata)?;

		match version {
			MetadataVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read(&mut algorithm).map_err(Error::Io)?;
				let algorithm = Algorithm::deserialize(algorithm)?;

				let mut hashing_algorithm = [0u8; 2];
				reader.read(&mut hashing_algorithm).map_err(Error::Io)?;
				let hashing_algorithm = HashingAlgorithm::deserialize(hashing_algorithm)?;

				let mut salt = [0u8; SALT_LEN];
				reader.read(&mut salt).map_err(Error::Io)?;

				let mut master_key = [0u8; ENCRYPTED_MASTER_KEY_LEN];
				reader.read(&mut master_key).map_err(Error::Io)?;

				let mut master_key_nonce = vec![0u8; algorithm.nonce_len()];
				reader.read(&mut master_key_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 26 - master_key_nonce.len()])
					.map_err(Error::Io)?;

				let mut metadata_nonce = vec![0u8; algorithm.nonce_len()];
				reader.read(&mut metadata_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 24 - metadata_nonce.len()])
					.map_err(Error::Io)?;

				let mut metadata_length = [0u8; 8];
				reader.read(&mut metadata_length).map_err(Error::Io)?;

				let metadata_length: usize = usize::from_le_bytes(metadata_length);

				let mut metadata = vec![0u8; metadata_length];
				reader.read(&mut metadata).map_err(Error::Io)?;

				let metadata = Self {
					version,
					algorithm,
					hashing_algorithm,
					salt,
					master_key,
					master_key_nonce,
					metadata_nonce,
					metadata_length,
					metadata,
				};

				Ok(metadata)
			}
		}
	}
}
