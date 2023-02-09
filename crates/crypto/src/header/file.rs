//! This module contains a standard file header, and the functions needed to serialize/deserialize it.
//!
//! # Examples
//!
//! ```rust,ignore
//! let password = Protected::new(b"password".to_vec());
//!
//! let mut writer = File::create("test.encrypted").unwrap();
//!
//! // This needs to be generated here, otherwise we won't have access to it for encryption
//! let master_key = generate_master_key();
//!
//! // Create a keyslot to be added to the header
//! let mut keyslots: Vec<Keyslot> = Vec::new();
//! keyslots.push(
//!     Keyslot::new(
//!         KeyslotVersion::V1,
//!         ALGORITHM,
//!         HASHING_ALGORITHM,
//!         password,
//!         &master_key,
//!     )
//!     .unwrap(),
//! );
//!
//! // Create the header for the encrypted file
//! let header = FileHeader::new(FileHeaderVersion::V1, ALGORITHM, keyslots, None, None);
//!
//! // Write the header to the file
//! header.write(&mut writer).unwrap();
//! ```
use std::io::SeekFrom;

use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::{
	crypto::stream::Algorithm,
	primitives::types::{Key, Nonce},
	Error, Protected, Result,
};

use super::{
	keyslot::{Keyslot, KEYSLOT_SIZE},
	metadata::Metadata,
	preview_media::PreviewMedia,
};

/// These are used to quickly and easily identify Spacedrive-encrypted files
/// These currently are set as "ballapp"
pub const MAGIC_BYTES: [u8; 7] = [0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70];

/// This header is primarily used for encrypting/decrypting single files.
///
/// It has support for 2 keyslots (maximum).
///
/// You may optionally attach `Metadata` and `PreviewMedia` structs to this header, and they will be accessible on deserialization.
///
/// This contains everything necessary for decryption, and the entire header can be flaunted with no worries (provided a suitable password was selected by the user).
#[derive(Clone)]
pub struct FileHeader {
	pub version: FileHeaderVersion,
	pub algorithm: Algorithm,
	pub nonce: Nonce,
	pub keyslots: Vec<Keyslot>,
	pub metadata: Option<Metadata>,
	pub preview_media: Option<PreviewMedia>,
}

/// This defines the main file header version.
#[derive(Clone, Copy)]
pub enum FileHeaderVersion {
	V1,
}

impl FileHeader {
	/// This function is used for creating a file header.
	pub fn new(
		version: FileHeaderVersion,
		algorithm: Algorithm,
		keyslots: Vec<Keyslot>,
	) -> Result<Self> {
		if keyslots.len() > 2 {
			return Err(Error::TooManyKeyslots);
		}

		let f = Self {
			version,
			algorithm,
			nonce: Nonce::generate(algorithm)?,
			keyslots,
			metadata: None,
			preview_media: None,
		};

		Ok(f)
	}

	/// This includes the magic bytes at the start of the file, and remainder of the header itself (excluding keyslots, metadata, and preview media as these can all change)
	///
	/// This can be used for getting the length of the AAD
	#[must_use]
	pub const fn size(version: FileHeaderVersion) -> usize {
		match version {
			FileHeaderVersion::V1 => 36,
		}
	}

	/// This is a helper function to decrypt a master key from keyslots that are attached to a header, from a user-supplied password.
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn decrypt_master_key(&self, password: Protected<Vec<u8>>) -> Result<Key> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		for v in &self.keyslots {
			if let Ok(key) = v.decrypt_master_key(password.clone()).await {
				return Ok(key);
			}
		}

		Err(Error::IncorrectPassword)
	}

	/// This is a helper function to decrypt a master key from keyslots that are attached to a header.
	///
	/// It takes in a Vec of pre-hashed keys, which is what the key manager returns
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn decrypt_master_key_from_prehashed(&self, hashed_keys: Vec<Key>) -> Result<Key> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		for hashed_key in hashed_keys {
			for v in &self.keyslots {
				if let Ok(key) = v
					.decrypt_master_key_from_prehashed(hashed_key.clone())
					.await
				{
					return Ok(key);
				}
			}
		}

		Err(Error::IncorrectPassword)
	}

	/// This is a helper function to serialize and write a header to a file.
	pub async fn write<W>(&self, writer: &mut W) -> Result<()>
	where
		W: AsyncWriteExt + Unpin + Send,
	{
		writer.write_all(&self.to_bytes()?).await?;
		Ok(())
	}

	/// This is a helper function to find which keyslot a key belongs to.
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub async fn find_key_index(&self, password: Protected<Vec<u8>>) -> Result<usize> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		for (i, v) in self.keyslots.iter().enumerate() {
			if let Some(i) = v.decrypt_master_key(password.clone()).await.ok().map(|_| i) {
				return Ok(i);
			}
		}

		Err(Error::IncorrectPassword)
	}

	/// This function should be used for generating AAD before encryption
	///
	/// Use the return value from `FileHeader::deserialize()` for decryption
	#[must_use]
	pub fn generate_aad(&self) -> Vec<u8> {
		match self.version {
			FileHeaderVersion::V1 => [
				MAGIC_BYTES.as_ref(),
				&self.version.to_bytes(),
				&self.algorithm.to_bytes(),
				&self.nonce,
				&vec![0u8; 25 - self.nonce.len()],
			]
			.into_iter()
			.flatten()
			.copied()
			.collect(),
		}
	}

	/// This function serializes a full header.
	///
	/// This will include keyslots, metadata and preview media (if provided)
	///
	/// An error will be returned if there are no keyslots/more than two keyslots attached.
	pub fn to_bytes(&self) -> Result<Vec<u8>> {
		match self.version {
			FileHeaderVersion::V1 => {
				if self.keyslots.len() > 2 {
					return Err(Error::TooManyKeyslots);
				} else if self.keyslots.is_empty() {
					return Err(Error::NoKeyslots);
				}

				let mut keyslots: Vec<Vec<u8>> =
					self.keyslots.iter().map(Keyslot::to_bytes).collect();

				if keyslots.len() == 1 {
					keyslots.push(vec![0u8; KEYSLOT_SIZE]);
				}

				let metadata = self
					.metadata
					.as_ref()
					.map_or(Vec::new(), Metadata::to_bytes);

				let preview_media = self
					.preview_media
					.as_ref()
					.map_or(Vec::new(), PreviewMedia::to_bytes);

				let header = [
					MAGIC_BYTES.as_ref(),
					&self.version.to_bytes(),
					&self.algorithm.to_bytes(),
					&self.nonce,
					&vec![0u8; 25 - self.nonce.len()],
					&keyslots[0],
					&keyslots[1],
					&metadata,
					&preview_media,
				]
				.into_iter()
				.flatten()
				.copied()
				.collect();

				Ok(header)
			}
		}
	}

	/// This deserializes a header directly from a reader, and leaves the reader at the start of the encrypted data.
	///
	/// On error, the cursor will not be rewound.
	///
	/// It returns both the header, and the AAD that should be used for decryption.
	pub async fn from_reader<R>(reader: &mut R) -> Result<(Self, Vec<u8>)>
	where
		R: AsyncReadExt + AsyncSeekExt + Unpin + Send,
	{
		let mut magic_bytes = [0u8; MAGIC_BYTES.len()];
		reader.read_exact(&mut magic_bytes).await?;

		if magic_bytes != MAGIC_BYTES {
			return Err(Error::Serialization);
		}

		let mut version = [0u8; 2];

		reader.read_exact(&mut version).await?;
		let version = FileHeaderVersion::from_bytes(version)?;

		// Rewind so we can get the AAD
		reader.rewind().await?;

		// read the aad according to the size
		let mut aad = vec![0u8; Self::size(version)];
		reader.read_exact(&mut aad).await?;

		// seek back to the start (plus magic bytes and the two version bytes)
		reader
			.seek(SeekFrom::Start(MAGIC_BYTES.len() as u64 + 2))
			.await?;

		// read the header
		let header = match version {
			FileHeaderVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read_exact(&mut algorithm).await?;
				let algorithm = Algorithm::from_bytes(algorithm)?;

				let mut nonce = vec![0u8; algorithm.nonce_len()];
				reader.read_exact(&mut nonce).await?;
				let nonce = Nonce::try_from(nonce)?;

				// read and discard the padding
				reader.read_exact(&mut vec![0u8; 25 - nonce.len()]).await?;

				let mut keyslot_bytes = [0u8; (KEYSLOT_SIZE * 2)]; // length of 2x keyslots
				let mut keyslots: Vec<Keyslot> = Vec::new();

				reader.read_exact(&mut keyslot_bytes).await?;

				for _ in 0..2 {
					Keyslot::from_reader(&mut keyslot_bytes.as_ref())
						.map(|k| keyslots.push(k))
						.ok();
				}

				let metadata = if let Ok(metadata) = Metadata::from_reader(reader).await {
					reader
						.seek(SeekFrom::Start(
							Self::size(version) as u64 + (KEYSLOT_SIZE * 2) as u64,
						))
						.await?;
					Ok::<Option<Metadata>, Error>(Some(metadata))
				} else {
					Ok(None)
				}?;

				let preview_media =
					if let Ok(preview_media) = PreviewMedia::from_reader(reader).await {
						Ok::<Option<PreviewMedia>, Error>(Some(preview_media))
					} else {
						let seek_len = metadata.as_ref().map_or_else(
							|| Self::size(version) as u64 + (KEYSLOT_SIZE * 2) as u64,
							|metadata| {
								Self::size(version) as u64
									+ (KEYSLOT_SIZE * 2) as u64 + metadata.size() as u64
							},
						);

						reader.seek(SeekFrom::Start(seek_len)).await?;

						Ok(None)
					}?;

				Self {
					version,
					algorithm,
					nonce,
					keyslots,
					metadata,
					preview_media,
				}
			}
		};

		Ok((header, aad))
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use crate::{
		keys::hashing::{HashingAlgorithm, Params},
		primitives::{types::Salt, LATEST_FILE_HEADER, LATEST_KEYSLOT, LATEST_PREVIEW_MEDIA},
	};

	use super::*;

	const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
	const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);
	const PVM_BYTES: [u8; 4] = [0x01, 0x02, 0x03, 0x04];

	#[tokio::test]
	async fn serialize_and_deserialize_header() {
		let mk = Key::generate();
		let content_salt = Salt::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive

		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![Keyslot::new(
				LATEST_KEYSLOT,
				ALGORITHM,
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw,
				mk,
			)
			.await
			.unwrap()],
		)
		.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(writer.position() == 260)
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_preview_media() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![Keyslot::new(
				LATEST_KEYSLOT,
				ALGORITHM,
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap()],
		)
		.unwrap();

		header
			.add_preview_media(LATEST_PREVIEW_MEDIA, ALGORITHM, mk, &PVM_BYTES)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let (header, _) = FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.preview_media.is_some());
		assert!(header.metadata.is_none());
		assert!(header.keyslots.len() == 1);
	}

	#[cfg(feature = "serde")]
	#[tokio::test]
	async fn serialize_and_deserialize_header_with_metadata() {
		use crate::primitives::LATEST_METADATA;

		#[derive(serde::Serialize)]
		struct Metadata {
			pub name: String,
			pub favorite: bool,
		}

		let mk = Key::generate();
		let md = Metadata {
			name: "file.txt".to_string(),
			favorite: true,
		};

		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![Keyslot::new(
				LATEST_KEYSLOT,
				ALGORITHM,
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap()],
		)
		.unwrap();

		header
			.add_metadata(LATEST_METADATA, ALGORITHM, mk, &md)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let (header, _) = FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.metadata.is_some());
		assert!(header.preview_media.is_none());
		assert!(header.keyslots.len() == 1);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_two_keyslots() {
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);
		let mk = Key::generate();

		let header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk.clone(),
				)
				.await
				.unwrap(),
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk,
				)
				.await
				.unwrap(),
			],
		)
		.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let (header, _) = FileHeader::from_reader(&mut writer).await.unwrap();
		assert!(header.keyslots.len() == 2);
		assert!(header.metadata.is_none());
		assert!(header.preview_media.is_none());
	}

	#[tokio::test]
	#[should_panic]
	async fn serialize_and_deserialize_header_with_too_many_keyslots() {
		let mk = Key::generate();

		FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk.clone(),
				)
				.await
				.unwrap(),
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk.clone(),
				)
				.await
				.unwrap(),
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk,
				)
				.await
				.unwrap(),
			],
		)
		.unwrap();
	}

	#[cfg(feature = "serde")]
	#[tokio::test]
	async fn serialize_and_deserialize_header_with_all() {
		use crate::primitives::LATEST_METADATA;

		#[derive(serde::Serialize)]
		struct Metadata {
			pub name: String,
			pub favorite: bool,
		}

		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);
		let mk = Key::generate();

		let md = Metadata {
			name: "file.txt".to_string(),
			favorite: true,
		};

		let mut header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk.clone(),
				)
				.await
				.unwrap(),
				Keyslot::new(
					LATEST_KEYSLOT,
					ALGORITHM,
					HASHING_ALGORITHM,
					Salt::generate(),
					Key::generate(),
					mk.clone(),
				)
				.await
				.unwrap(),
			],
		)
		.unwrap();

		header
			.add_metadata(LATEST_METADATA, ALGORITHM, mk.clone(), &md)
			.await
			.unwrap();

		header
			.add_preview_media(LATEST_PREVIEW_MEDIA, ALGORITHM, mk, &PVM_BYTES)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let (header, _) = FileHeader::from_reader(&mut writer).await.unwrap();
		assert!(header.metadata.is_some());
		assert!(header.preview_media.is_some());
		assert!(header.keyslots.len() == 2);
	}

	#[tokio::test]
	async fn aad_validity() {
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let header = FileHeader::new(
			LATEST_FILE_HEADER,
			ALGORITHM,
			vec![Keyslot::new(
				LATEST_KEYSLOT,
				ALGORITHM,
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				Key::generate(),
			)
			.await
			.unwrap()],
		)
		.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let (header, aad) = FileHeader::from_reader(&mut writer).await.unwrap();

		assert_eq!(header.generate_aad(), aad);
		assert_eq!(&header.to_bytes().unwrap()[..36], aad);
	}
}
