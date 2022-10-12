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
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::{
	crypto::stream::Algorithm,
	error::Error,
	primitives::{generate_nonce, MASTER_KEY_LEN},
	Protected,
};

use super::{keyslot::Keyslot, metadata::Metadata, preview_media::PreviewMedia};

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
	pub nonce: Vec<u8>,
	pub keyslots: Vec<Keyslot>,
	pub metadata: Option<Metadata>,
	pub preview_media: Option<PreviewMedia>,
}

/// This defines the main file header version.
#[derive(Clone, Copy)]
pub enum FileHeaderVersion {
	V1,
}

/// This includes the magic bytes at the start of the file, and remainder of the header itself (excluding keyslots, metadata, and preview media as these can all change)
#[must_use]
pub const fn aad_length(version: FileHeaderVersion) -> usize {
	match version {
		FileHeaderVersion::V1 => 36,
	}
}

impl FileHeader {
	/// This function is used for creating a file header.
	#[must_use]
	pub fn new(
		version: FileHeaderVersion,
		algorithm: Algorithm,
		keyslots: Vec<Keyslot>,
		metadata: Option<Metadata>,
		preview_media: Option<PreviewMedia>,
	) -> Self {
		let nonce = generate_nonce(algorithm);

		Self {
			version,
			algorithm,
			nonce,
			keyslots,
			metadata,
			preview_media,
		}
	}

	/// This is a helper function to decrypt a master key from keyslots that are attached to a header.
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<[u8; MASTER_KEY_LEN]>, Error> {
		let mut master_key = [0u8; MASTER_KEY_LEN];

		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		for keyslot in &self.keyslots {
			if let Ok(decrypted_master_key) = keyslot.decrypt_master_key(&password) {
				master_key.copy_from_slice(&decrypted_master_key);
			}
		}

		if master_key == [0u8; MASTER_KEY_LEN] {
			Err(Error::IncorrectPassword)
		} else {
			Ok(Protected::new(master_key))
		}
	}

	pub fn write<W>(&self, writer: &mut W) -> Result<(), Error>
	where
		W: Write + Seek,
	{
		writer.write(&self.serialize()?).map_err(Error::Io)?;
		Ok(())
	}

	/// This function should be used for generating AAD before encryption
	///
	/// Use the return value from `FileHeader::deserialize()` for decryption
	#[must_use]
	pub fn generate_aad(&self) -> Vec<u8> {
		match self.version {
			FileHeaderVersion::V1 => {
				let mut aad: Vec<u8> = Vec::new();
				aad.extend_from_slice(&MAGIC_BYTES); // 7
				aad.extend_from_slice(&self.version.serialize()); // 9
				aad.extend_from_slice(&self.algorithm.serialize()); // 11
				aad.extend_from_slice(&self.nonce); // 19 OR 31
				aad.extend_from_slice(&vec![0u8; 25 - self.nonce.len()]); // padded until 36 bytes
				aad
			}
		}
	}

	/// This function serializes a full header.
	///
	/// This will include keyslots, metadata and preview media (if provided)
	///
	/// An error will be returned if there are no keyslots/more than two keyslots attached.
	pub fn serialize(&self) -> Result<Vec<u8>, Error> {
		match self.version {
			FileHeaderVersion::V1 => {
				if self.keyslots.len() > 2 {
					return Err(Error::TooManyKeyslots);
				} else if self.keyslots.is_empty() {
					return Err(Error::NoKeyslots);
				}

				let mut header: Vec<u8> = Vec::new();
				header.extend_from_slice(&MAGIC_BYTES); // 7
				header.extend_from_slice(&self.version.serialize()); // 9
				header.extend_from_slice(&self.algorithm.serialize()); // 11
				header.extend_from_slice(&self.nonce); // 19 OR 31
				header.extend_from_slice(&vec![0u8; 25 - self.nonce.len()]); // padded until 36 bytes

				for keyslot in &self.keyslots {
					header.extend_from_slice(&keyslot.serialize());
				}

				for _ in 0..(2 - self.keyslots.len()) {
					header.extend_from_slice(&[0u8; 96]);
				}

				if let Some(metadata) = self.metadata.clone() {
					header.extend_from_slice(&metadata.serialize());
				}

				if let Some(preview_media) = self.preview_media.clone() {
					header.extend_from_slice(&preview_media.serialize());
				}

				Ok(header)
			}
		}
	}

	/// This deserializes a header directly from a reader, and leaves the reader at the start of the encrypted data.
	///
	/// On error, the cursor will not be rewound.
	///
	/// It returns both the header, and the AAD that should be used for decryption.
	pub fn deserialize<R>(reader: &mut R) -> Result<(Self, Vec<u8>), Error>
	where
		R: Read + Seek,
	{
		let mut magic_bytes = [0u8; MAGIC_BYTES.len()];
		reader.read(&mut magic_bytes).map_err(Error::Io)?;

		if magic_bytes != MAGIC_BYTES {
			return Err(Error::FileHeader);
		}

		let mut version = [0u8; 2];

		reader.read(&mut version).map_err(Error::Io)?;
		let version = FileHeaderVersion::deserialize(version)?;

		// Rewind so we can get the AAD
		reader.rewind().map_err(Error::Io)?;

		let mut aad = vec![0u8; aad_length(version)];
		reader.read(&mut aad).map_err(Error::Io)?;

		reader
			.seek(SeekFrom::Start(MAGIC_BYTES.len() as u64 + 2))
			.map_err(Error::Io)?;

		let header = match version {
			FileHeaderVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read(&mut algorithm).map_err(Error::Io)?;
				let algorithm = Algorithm::deserialize(algorithm)?;

				let mut nonce = vec![0u8; algorithm.nonce_len()];
				reader.read(&mut nonce).map_err(Error::Io)?;

				// read and discard the padding
				reader
					.read(&mut vec![0u8; 25 - nonce.len()])
					.map_err(Error::Io)?;

				let mut keyslot_bytes = [0u8; 192]; // length of 2x keyslots
				let mut keyslots: Vec<Keyslot> = Vec::new();

				reader.read(&mut keyslot_bytes).map_err(Error::Io)?;
				let mut keyslot_reader = Cursor::new(keyslot_bytes);

				for _ in 0..2 {
					if let Ok(keyslot) = Keyslot::deserialize(&mut keyslot_reader) {
						keyslots.push(keyslot);
					}
				}

				let metadata = if let Ok(metadata) = Metadata::deserialize(reader) {
					Some(metadata)
				} else {
					// header/aad area, keyslot area
					reader.seek(SeekFrom::Start(36 + 192)).map_err(Error::Io)?;
					None
				};

				let preview_media = if let Ok(preview_media) = PreviewMedia::deserialize(reader) {
					Some(preview_media)
				} else {
					// header/aad area, keyslot area, full metadata length
					if metadata.is_some() {
						reader
							.seek(SeekFrom::Start(
								36 + 192 + metadata.clone().unwrap().get_length() as u64,
							))
							.map_err(Error::Io)?;
					} else {
						// header/aad area, keyslot area
						reader.seek(SeekFrom::Start(36 + 192)).map_err(Error::Io)?;
					}
					None
				};

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
mod test {
	use crate::{
		crypto::stream::Algorithm,
		header::keyslot::{Keyslot, KeyslotVersion},
		keys::hashing::{HashingAlgorithm, Params},
	};
	use std::io::Cursor;

	use super::{FileHeader, FileHeaderVersion};

	const HEADER_BYTES_NO_ADDITIONAL_OBJECTS: [u8; 228] = [
		98, 97, 108, 108, 97, 112, 112, 10, 1, 11, 1, 230, 47, 48, 63, 225, 227, 15, 211, 115, 69,
		169, 184, 184, 18, 110, 189, 167, 0, 144, 26, 0, 0, 0, 0, 0, 13, 1, 11, 1, 15, 1, 104, 176,
		135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235, 117, 160, 55, 36, 93, 100,
		83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74, 205, 239, 253, 48, 239, 249, 203, 121,
		126, 231, 52, 38, 49, 154, 254, 234, 41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198,
		109, 33, 216, 163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
		184, 216, 175, 202, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
	];

	#[test]
	fn deserialize_header() {
		let mut reader = Cursor::new(HEADER_BYTES_NO_ADDITIONAL_OBJECTS);
		FileHeader::deserialize(&mut reader).unwrap();
	}

	#[test]
	fn serialize_header() {
		let header: FileHeader = FileHeader {
			version: FileHeaderVersion::V1,
			algorithm: Algorithm::XChaCha20Poly1305,
			nonce: [
				230, 47, 48, 63, 225, 227, 15, 211, 115, 69, 169, 184, 184, 18, 110, 189, 167, 0,
				144, 26,
			]
			.to_vec(),
			keyslots: [Keyslot {
				version: KeyslotVersion::V1,
				algorithm: Algorithm::XChaCha20Poly1305,
				hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
				salt: [
					104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235, 117,
				],
				master_key: [
					160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74, 205,
					239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234, 41, 113,
					169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
				],
				nonce: [
					163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131, 184,
					216, 175, 202,
				]
				.to_vec(),
			}]
			.to_vec(),
			metadata: None,
			preview_media: None,
		};

		let header_bytes = header.serialize().unwrap();

		assert_eq!(HEADER_BYTES_NO_ADDITIONAL_OBJECTS.to_vec(), header_bytes)
	}

	#[test]
	#[should_panic]
	fn serialize_header_with_too_many_keyslots() {
		let header: FileHeader = FileHeader {
			version: FileHeaderVersion::V1,
			algorithm: Algorithm::XChaCha20Poly1305,
			nonce: [
				230, 47, 48, 63, 225, 227, 15, 211, 115, 69, 169, 184, 184, 18, 110, 189, 167, 0,
				144, 26,
			]
			.to_vec(),
			keyslots: [
				Keyslot {
					version: KeyslotVersion::V1,
					algorithm: Algorithm::XChaCha20Poly1305,
					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
					salt: [
						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
						117,
					],
					master_key: [
						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
					],
					nonce: [
						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
						184, 216, 175, 202,
					]
					.to_vec(),
				},
				Keyslot {
					version: KeyslotVersion::V1,
					algorithm: Algorithm::XChaCha20Poly1305,
					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
					salt: [
						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
						117,
					],
					master_key: [
						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
					],
					nonce: [
						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
						184, 216, 175, 202,
					]
					.to_vec(),
				},
				Keyslot {
					version: KeyslotVersion::V1,
					algorithm: Algorithm::XChaCha20Poly1305,
					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
					salt: [
						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
						117,
					],
					master_key: [
						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
					],
					nonce: [
						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
						184, 216, 175, 202,
					]
					.to_vec(),
				},
			]
			.to_vec(),
			metadata: None,
			preview_media: None,
		};

		header.serialize().unwrap();
	}
}
