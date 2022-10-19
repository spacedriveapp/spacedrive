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
use std::io::{Read, Seek};

use crate::{
	crypto::stream::{Algorithm},
	error::Error,
};

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

impl Metadata {
	#[must_use]
	pub fn get_length(&self) -> usize {
		match self.version {
			MetadataVersion::V1 => 36 + self.metadata.len(),
		}
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
				metadata.extend_from_slice(&self.metadata_nonce); // 24 max
				metadata.extend_from_slice(&vec![0u8; 24 - self.metadata_nonce.len()]); // 28
				metadata.extend_from_slice(&self.metadata.len().to_le_bytes()); // 36 total bytes
				metadata.extend_from_slice(&self.metadata); // this can vary in length
				metadata
			}
		}
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
		let version = MetadataVersion::deserialize(version).map_err(|_| Error::NoMetadata)?;

		match version {
			MetadataVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read(&mut algorithm).map_err(Error::Io)?;
				let algorithm = Algorithm::deserialize(algorithm)?;

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
					metadata_nonce,
					metadata,
				};

				Ok(metadata)
			}
		}
	}
}
