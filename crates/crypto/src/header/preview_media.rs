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
use tokio::io::AsyncReadExt;

use crate::{
	crypto::stream::{Algorithm, StreamDecryption, StreamEncryption},
	primitives::types::{Key, Nonce},
	Error, ProtectedVec, Result,
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
	pub media_nonce: Nonce,
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
	#[allow(clippy::needless_pass_by_value)]
	pub async fn add_preview_media(
		&mut self,
		version: PreviewMediaVersion,
		algorithm: Algorithm,
		master_key: Key,
		media: &[u8],
	) -> Result<()> {
		let media_nonce = Nonce::generate(algorithm)?;

		let encrypted_media =
			StreamEncryption::encrypt_bytes(master_key, media_nonce, algorithm, media, &[]).await?;

		self.preview_media = Some(PreviewMedia {
			version,
			algorithm,
			media_nonce,
			media: encrypted_media,
		});

		Ok(())
	}

	/// This function is what you'll want to use to get the preview media for a file
	///
	/// All it requires is the user's password. Hashing is handled for you.
	///
	/// Once provided, a `Vec<u8>` is returned that contains the preview media
	pub async fn decrypt_preview_media(
		&self,
		password: ProtectedVec<u8>,
	) -> Result<ProtectedVec<u8>> {
		let master_key = self.decrypt_master_key(password).await?;

		if let Some(pvm) = self.preview_media.as_ref() {
			let pvm = StreamDecryption::decrypt_bytes(
				master_key,
				pvm.media_nonce,
				pvm.algorithm,
				&pvm.media,
				&[],
			)
			.await?;

			Ok(pvm)
		} else {
			Err(Error::NoPreviewMedia)
		}
	}

	/// This function is what you'll want to use to get the preview media for a file
	///
	/// All it requires is pre-hashed keys returned from the key manager
	///
	/// Once provided, a `Vec<u8>` is returned that contains the preview media
	pub async fn decrypt_preview_media_from_prehashed(
		&self,
		hashed_keys: Vec<Key>,
	) -> Result<ProtectedVec<u8>> {
		let master_key = self.decrypt_master_key_from_prehashed(hashed_keys).await?;

		if let Some(pvm) = self.preview_media.as_ref() {
			let pvm = StreamDecryption::decrypt_bytes(
				master_key,
				pvm.media_nonce,
				pvm.algorithm,
				&pvm.media,
				&[],
			)
			.await?;

			Ok(pvm)
		} else {
			Err(Error::NoPreviewMedia)
		}
	}
}

impl PreviewMedia {
	#[must_use]
	pub fn size(&self) -> usize {
		self.to_bytes().len()
	}

	/// This function is used to serialize a preview media header item into bytes
	///
	/// This also includes the encrypted preview media itself, so this may be sizeable
	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		match self.version {
			PreviewMediaVersion::V1 => [
				self.version.to_bytes().as_ref(),
				self.algorithm.to_bytes().as_ref(),
				&self.media_nonce,
				&vec![0u8; 24 - self.media_nonce.len()],
				&(self.media.len() as u64).to_le_bytes(),
				&self.media,
			]
			.into_iter()
			.flatten()
			.copied()
			.collect(),
		}
	}

	/// This function reads a preview media header item from a reader
	///
	/// The cursor will be left at the end of the preview media item on success
	///
	/// The cursor will not be rewound on error.
	pub async fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: AsyncReadExt + Unpin + Send,
	{
		let mut version = [0u8; 2];
		reader.read_exact(&mut version).await?;
		let version =
			PreviewMediaVersion::from_bytes(version).map_err(|_| Error::NoPreviewMedia)?;

		match version {
			PreviewMediaVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read_exact(&mut algorithm).await?;
				let algorithm = Algorithm::from_bytes(algorithm)?;

				let mut media_nonce = vec![0u8; algorithm.nonce_len()];
				reader.read_exact(&mut media_nonce).await?;
				let media_nonce = Nonce::try_from(media_nonce)?;

				reader
					.read_exact(&mut vec![0u8; 24 - media_nonce.len()])
					.await?;

				let mut media_length = [0u8; 8];
				reader.read_exact(&mut media_length).await?;

				let media_length = u64::from_le_bytes(media_length);

				#[allow(clippy::cast_possible_truncation)]
				let mut media = vec![0u8; media_length as usize];
				reader.read_exact(&mut media).await?;

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
