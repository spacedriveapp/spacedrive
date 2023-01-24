//! This module contains the metadata header item.
//!
//! This is an optional item, and anything that may be serialized with `serde` can be used here.
//!
//! # Examples
//!
//! ```rust,ignore
//! #[derive(Serialize, Deserialize)]
//! pub struct FileInformation {
//!     pub file_name: String,
//! }
//!
//! let embedded_metadata = FileInformation {
//!     file_name: "filename.txt".to_string(),
//! };
//!
//! // Ideally this will be generated via a key management system
//! let md_salt = generate_salt();
//!
//! let md = Metadata::new(
//!     MetadataVersion::V1,
//!     ALGORITHM,
//!     HASHING_ALGORITHM,
//!     password,
//!     &md_salt,
//!     &embedded_metadata,
//! )
//! .unwrap();
//! ```
use std::io::Read;

#[cfg(feature = "serde")]
use crate::{
	crypto::stream::{StreamDecryption, StreamEncryption},
	primitives::{generate_nonce, KEY_LEN},
	Protected,
};

use crate::{crypto::stream::Algorithm, Error, Result};

use super::file::FileHeader;

/// This is a metadata header item. You may add it to a header, and this will be stored with the file.
///
/// The `Metadata::new()` function handles master key and metadata encryption.
///
/// The salt should be generated elsewhere (e.g. a key management system).
#[derive(Clone)]
pub struct Metadata {
	pub version: MetadataVersion,
	pub algorithm: Algorithm, // encryption algorithm
	pub metadata_nonce: Vec<u8>,
	pub metadata: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum MetadataVersion {
	V1,
}

impl FileHeader {
	/// This should be used for creating a header metadata item.
	///
	/// It handles encrypting the master key and metadata.
	///
	/// You will need to provide the user's password, and a semi-universal salt for hashing the user's password. This allows for extremely fast decryption.
	///
	/// Metadata needs to be accessed switfly, so a key management system should handle the salt generation.
	#[cfg(feature = "serde")]
	#[allow(clippy::needless_pass_by_value)]
	pub async fn add_metadata<T>(
		&mut self,
		version: MetadataVersion,
		algorithm: Algorithm,
		master_key: Protected<[u8; KEY_LEN]>,
		metadata: &T,
	) -> Result<()>
	where
		T: ?Sized + serde::Serialize,
	{
		let metadata_nonce = generate_nonce(algorithm);

		let encrypted_metadata = StreamEncryption::encrypt_bytes(
			master_key,
			&metadata_nonce,
			algorithm,
			&serde_json::to_vec(metadata).map_err(|_| Error::Serialization)?,
			&[],
		)
		.await?;

		let metadata = Metadata {
			version,
			algorithm,
			metadata_nonce,
			metadata: encrypted_metadata,
		};

		self.metadata = Some(metadata);

		Ok(())
	}

	/// This function should be used to retrieve the metadata for a file
	///
	/// All it requires is pre-hashed keys returned from the key manager
	///
	/// A deserialized data type will be returned from this function
	#[cfg(feature = "serde")]
	pub async fn decrypt_metadata_from_prehashed<T>(
		&self,
		hashed_keys: Vec<Protected<[u8; KEY_LEN]>>,
	) -> Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let master_key = self.decrypt_master_key_from_prehashed(hashed_keys)?;

		match self.metadata.as_ref() {
			Some(metadata) => {
				let metadata = StreamDecryption::decrypt_bytes(
					master_key,
					&metadata.metadata_nonce,
					metadata.algorithm,
					&metadata.metadata,
					&[],
				)
				.await?;

				serde_json::from_slice::<T>(&metadata).map_err(|_| Error::Serialization)
			}
			None => Err(Error::NoMetadata),
		}
	}

	/// This function should be used to retrieve the metadata for a file
	///
	/// All it requires is a password. Hashing is handled for you.
	///
	/// A deserialized data type will be returned from this function
	#[cfg(feature = "serde")]
	pub async fn decrypt_metadata<T>(&self, password: Protected<Vec<u8>>) -> Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let master_key = self.decrypt_master_key(password)?;

		match self.metadata.as_ref() {
			Some(metadata) => {
				let metadata = StreamDecryption::decrypt_bytes(
					master_key,
					&metadata.metadata_nonce,
					metadata.algorithm,
					&metadata.metadata,
					&[],
				)
				.await?;

				serde_json::from_slice::<T>(&metadata).map_err(|_| Error::Serialization)
			}
			None => Err(Error::NoMetadata),
		}
	}
}

impl Metadata {
	#[must_use]
	pub fn size(&self) -> usize {
		self.to_bytes().len()
	}

	/// This function is used to serialize a metadata item into bytes
	///
	/// This also includes the encrypted metadata itself, so this may be sizeable
	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		match self.version {
			MetadataVersion::V1 => [
				self.version.to_bytes().as_ref(),
				self.algorithm.to_bytes().as_ref(),
				&self.metadata_nonce,
				&vec![0u8; 24 - self.metadata_nonce.len()],
				&(self.metadata.len() as u64).to_le_bytes(),
				&self.metadata,
			]
			.into_iter()
			.flatten()
			.copied()
			.collect(),
		}
	}

	/// This function reads a metadata header item from a reader
	///
	/// The cursor will be left at the end of the metadata item on success
	///
	/// The cursor will not be rewound on error.
	pub fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: Read,
	{
		let mut version = [0u8; 2];
		reader.read_exact(&mut version)?;
		let version = MetadataVersion::from_bytes(version).map_err(|_| Error::NoMetadata)?;

		match version {
			MetadataVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read_exact(&mut algorithm)?;
				let algorithm = Algorithm::from_bytes(algorithm)?;

				let mut metadata_nonce = vec![0u8; algorithm.nonce_len()];
				reader.read_exact(&mut metadata_nonce)?;

				reader.read_exact(&mut vec![0u8; 24 - metadata_nonce.len()])?;

				let mut metadata_length = [0u8; 8];
				reader.read_exact(&mut metadata_length)?;

				let metadata_length = u64::from_le_bytes(metadata_length);

				#[allow(clippy::cast_possible_truncation)]
				let mut metadata = vec![0u8; metadata_length as usize];
				reader.read_exact(&mut metadata)?;

				let metadata = Self {
					version,
					algorithm,
					metadata_nonce,
					metadata,
				};

				Ok(metadata)
			}
		}
	}
}
