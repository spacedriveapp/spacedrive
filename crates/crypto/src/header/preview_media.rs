//! This module contains the preview media header item.
//!
//! It is an optional extension to a header, and is intended for video/image thumbnails.
//!
//! # Examples
//!
//! ```rust,ignore
//! // Ideally this will be generated via a key management system
//! let pvm_salt = generate_salt();
//!
//! let pvm_media = b"a nice mountain".to_vec();
//!
//! let pvm = PreviewMedia::new(
//!     PreviewMediaVersion::V1,
//!     ALGORITHM,
//!     HASHING_ALGORITHM,
//!     password,
//!     &pvm_salt,
//!     &pvm_media,
//! )
//! .unwrap();
//! ```
use std::io::{Read, Seek};

use crate::{
	crypto::stream::{Algorithm, StreamDecryption, StreamEncryption},
	error::Error,
	keys::hashing::HashingAlgorithm,
	primitives::{
		generate_master_key, generate_nonce, to_array, ENCRYPTED_MASTER_KEY_LEN, MASTER_KEY_LEN,
		SALT_LEN,
	},
	Protected,
};

/// This is a preview media header item. You may add it to a header, and this will be stored with the file.
///
/// The `Metadata::new()` function handles master key and metadata encryption.
///
/// The salt should be generated elsewhere (e.g. a key management system).
#[derive(Clone)]
pub struct PreviewMedia {
	pub version: PreviewMediaVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub media_nonce: Vec<u8>,
	pub media: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum PreviewMediaVersion {
	V1,
}

impl PreviewMedia {
	/// This should be used for creating a header preview media item.
	///
	/// This handles encrypting the master key and preview media.
	///
	/// You will need to provide the user's password, and a semi-universal salt for hashing the user's password. This allows for extremely fast decryption.
	///
	/// Preview media needs to be accessed switfly, so a key management system should handle the salt generation.
	pub fn new(
		version: PreviewMediaVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		content_salt: &[u8; SALT_LEN],
		hashed_key: &Protected<[u8; 32]>,
		media: &[u8],
	) -> Result<Self, Error> {
		let media_nonce = generate_nonce(algorithm);
		let master_key_nonce = generate_nonce(algorithm);
		let master_key = generate_master_key();

		let encrypted_master_key: [u8; 48] = to_array(StreamEncryption::encrypt_bytes(
			hashed_key.clone(),
			&master_key_nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		let encrypted_media =
			StreamEncryption::encrypt_bytes(master_key, &media_nonce, algorithm, media, &[])?;

		Ok(Self {
			version,
			algorithm,
			hashing_algorithm,
			salt: *content_salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			media_nonce,
			media: encrypted_media,
		})
	}

	#[must_use]
	pub fn get_length(&self) -> usize {
		match self.version {
			PreviewMediaVersion::V1 => 128 + self.media.len(),
		}
	}

	/// This function is used to serialize a preview media header item into bytes
	///
	/// This also includes the encrypted preview media itself, so this may be sizeable
	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			PreviewMediaVersion::V1 => {
				let mut preview_media: Vec<u8> = Vec::new();
				preview_media.extend_from_slice(&self.version.serialize()); // 2
				preview_media.extend_from_slice(&self.algorithm.serialize()); // 4
				preview_media.extend_from_slice(&self.hashing_algorithm.serialize()); // 6
				preview_media.extend_from_slice(&self.salt); // 22
				preview_media.extend_from_slice(&self.master_key); // 70
				preview_media.extend_from_slice(&self.master_key_nonce); // 82 or 94
				preview_media.extend_from_slice(&vec![0u8; 26 - self.master_key_nonce.len()]); // 96
				preview_media.extend_from_slice(&self.media_nonce); // 108 or 120
				preview_media.extend_from_slice(&vec![0u8; 24 - self.media_nonce.len()]); // 120
				preview_media.extend_from_slice(&self.media.len().to_le_bytes()); // 128 total bytes
				preview_media.extend_from_slice(&self.media); // this can vary in length
				preview_media
			}
		}
	}

	/// This function is what you'll want to use to get the preview media for a file
	///
	/// All it requires is a pre-hashed key (hashed with the salt provided on creation)
	///
	/// Once provided, a `Vec<u8>` is returned that contains the preview media
	pub fn decrypt_preview_media(
		&self,
		hashed_key: Protected<[u8; 32]>,
	) -> Result<Protected<Vec<u8>>, Error> {
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

		let media = StreamDecryption::decrypt_bytes(
			master_key,
			&self.media_nonce,
			self.algorithm,
			&self.media,
			&[],
		)?;

		Ok(media)
	}

	/// This function reads a preview media header item from a reader
	///
	/// The cursor will be left at the end of the preview media item on success
	///
	/// The cursor will not be rewound on error.
	pub fn deserialize<R>(reader: &mut R) -> Result<Self, Error>
	where
		R: Read + Seek,
	{
		let mut version = [0u8; 2];
		reader.read(&mut version).map_err(Error::Io)?;
		let version =
			PreviewMediaVersion::deserialize(version).map_err(|_| Error::NoPreviewMedia)?;

		match version {
			PreviewMediaVersion::V1 => {
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

				let mut media_nonce = vec![0u8; algorithm.nonce_len()];
				reader.read(&mut media_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 24 - media_nonce.len()])
					.map_err(Error::Io)?;

				let mut media_length = [0u8; 8];
				reader.read(&mut media_length).map_err(Error::Io)?;

				let media_length: usize = usize::from_le_bytes(media_length);

				let mut media = vec![0u8; media_length];
				reader.read(&mut media).map_err(Error::Io)?;

				let preview_media = Self {
					version,
					algorithm,
					hashing_algorithm,
					salt,
					master_key,
					master_key_nonce,
					media_nonce,
					media,
				};

				Ok(preview_media)
			}
		}
	}
}
