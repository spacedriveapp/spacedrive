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
	Error,
	Result,
	primitives::{generate_nonce, MASTER_KEY_LEN},
	Protected,
};

use super::file::FileHeader;

/// This is a preview media header item. You may add it to a header, and this will be stored with the file.
///
/// The `Metadata::new()` function handles master key and metadata encryption.
///
/// The salt should be generated elsewhere (e.g. a key management system).
#[derive(Clone)]
pub struct PreviewMedia {
	pub version: PreviewMediaVersion,
	pub algorithm: Algorithm, // encryption algorithm
	pub media_nonce: Vec<u8>,
	pub media: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum PreviewMediaVersion {
	V1,
}

impl FileHeader {
	/// This should be used for creating a header preview media item.
	///
	/// This handles encrypting the master key and preview media.
	///
	/// You will need to provide the user's password, and a semi-universal salt for hashing the user's password. This allows for extremely fast decryption.
	///
	/// Preview media needs to be accessed switfly, so a key management system should handle the salt generation.
	pub fn add_preview_media(
		&mut self,
		version: PreviewMediaVersion,
		algorithm: Algorithm,
		master_key: &Protected<[u8; MASTER_KEY_LEN]>,
		media: &[u8],
	) -> Result<()> {
		let media_nonce = generate_nonce(algorithm);

		let encrypted_media = StreamEncryption::encrypt_bytes(
			master_key.clone(),
			&media_nonce,
			algorithm,
			media,
			&[],
		)?;

		let pvm = PreviewMedia {
			version,
			algorithm,
			media_nonce,
			media: encrypted_media,
		};

		self.preview_media = Some(pvm);

		Ok(())
	}

	/// This function is what you'll want to use to get the preview media for a file
	///
	/// All it requires is pre-hashed keys returned from the key manager
	///
	/// Once provided, a `Vec<u8>` is returned that contains the preview media
	pub fn decrypt_preview_media_from_prehashed(
		&self,
		hashed_keys: Vec<Protected<[u8; 32]>>,
	) -> Result<Protected<Vec<u8>>> {
		let master_key = self.decrypt_master_key_from_prehashed(hashed_keys)?;

		// could be an expensive clone (a few MiB at most)
		if let Some(pvm) = self.preview_media.clone() {
			let media = StreamDecryption::decrypt_bytes(
				master_key,
				&pvm.media_nonce,
				pvm.algorithm,
				&pvm.media,
				&[],
			)?;

			Ok(media)
		} else {
			Err(Error::NoPreviewMedia)
		}
	}

	/// This function is what you'll want to use to get the preview media for a file
	///
	/// All it requires is the user's password. Hashing is handled for you.
	///
	/// Once provided, a `Vec<u8>` is returned that contains the preview media
	pub fn decrypt_preview_media(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<Vec<u8>>> {
		let master_key = self.decrypt_master_key(password)?;

		// could be an expensive clone (a few MiB at most)
		if let Some(pvm) = self.preview_media.clone() {
			let media = StreamDecryption::decrypt_bytes(
				master_key,
				&pvm.media_nonce,
				pvm.algorithm,
				&pvm.media,
				&[],
			)?;

			Ok(media)
		} else {
			Err(Error::NoPreviewMedia)
		}
	}
}

impl PreviewMedia {
	#[must_use]
	pub fn get_length(&self) -> usize {
		match self.version {
			PreviewMediaVersion::V1 => 36 + self.media.len(),
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
				preview_media.extend_from_slice(&self.media_nonce); // 24 max
				preview_media.extend_from_slice(&vec![0u8; 24 - self.media_nonce.len()]); // 28 total bytes
				preview_media.extend_from_slice(&self.media.len().to_le_bytes()); // 36 total bytes
				preview_media.extend_from_slice(&self.media); // this can vary in length
				preview_media
			}
		}
	}

	/// This function reads a preview media header item from a reader
	///
	/// The cursor will be left at the end of the preview media item on success
	///
	/// The cursor will not be rewound on error.
	pub fn deserialize<R>(reader: &mut R) -> Result<Self>
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
					media_nonce,
					media,
				};

				Ok(preview_media)
			}
		}
	}
}
