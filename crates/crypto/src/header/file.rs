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
use std::io::{Read, Seek, SeekFrom, Write};

use crate::{
	crypto::stream::Algorithm,
	primitives::{generate_nonce, to_array, KEY_LEN},
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

impl FileHeader {
	/// This function is used for creating a file header.
	#[must_use]
	pub fn new(
		version: FileHeaderVersion,
		algorithm: Algorithm,
		keyslots: Vec<Keyslot>,
		//metadata: Option<Metadata>,
		//preview_media: Option<PreviewMedia>,
	) -> Self {
		let nonce = generate_nonce(algorithm);

		Self {
			version,
			algorithm,
			nonce,
			keyslots,
			metadata: None,
			preview_media: None,
		}
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
	pub fn decrypt_master_key(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<[u8; KEY_LEN]>> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		self.keyslots
			.iter()
			.find_map(|v| v.decrypt_master_key(password.clone()).ok())
			.map(|v| Protected::new(to_array::<KEY_LEN>(v.expose().clone()).unwrap()))
			.ok_or(Error::IncorrectPassword)
	}

	/// This is a helper function to find which keyslot a key belongs to.
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub fn find_key_index(&self, password: Protected<Vec<u8>>) -> Result<usize> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		self.keyslots
			.iter()
			.enumerate()
			.find_map(|(i, v)| v.decrypt_master_key(password.clone()).ok().map(|_| i))
			.ok_or(Error::IncorrectPassword)
	}

	/// This is a helper function to serialize and write a header to a file.
	pub fn write<W>(&self, writer: &mut W) -> Result<()>
	where
		W: Write,
	{
		writer.write_all(&self.to_bytes()?)?;
		Ok(())
	}

	/// This is a helper function to decrypt a master key from keyslots that are attached to a header.
	///
	/// It takes in a Vec of pre-hashed keys, which is what the key manager returns
	///
	/// You receive an error if the password doesn't match or if there are no keyslots.
	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key_from_prehashed(
		&self,
		hashed_keys: Vec<Protected<[u8; KEY_LEN]>>,
	) -> Result<Protected<[u8; KEY_LEN]>> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		hashed_keys
			.iter()
			.find_map(|v| {
				self.keyslots.iter().find_map(|z| {
					z.decrypt_master_key_from_prehashed(v.clone())
						.ok()
						.map(|x| Protected::new(to_array::<KEY_LEN>(x.expose().clone()).unwrap()))
				})
			})
			.ok_or(Error::IncorrectPassword)
	}

	/// This function should be used for generating AAD before encryption
	///
	/// Use the return value from `FileHeader::deserialize()` for decryption
	#[must_use]
	pub fn generate_aad(&self) -> Vec<u8> {
		match self.version {
			FileHeaderVersion::V1 => vec![
				MAGIC_BYTES.as_ref(),
				self.version.to_bytes().as_ref(),
				self.algorithm.to_bytes().as_ref(),
				self.nonce.as_ref(),
				&vec![0u8; 25 - self.nonce.len()],
			]
			.iter()
			.flat_map(|&v| v)
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

				let metadata = self.metadata.clone().map_or(Vec::new(), |v| v.to_bytes());

				let preview_media = self
					.preview_media
					.clone()
					.map_or(Vec::new(), |v| v.to_bytes());

				let header = vec![
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
				.iter()
				.flat_map(|&v| v)
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
	pub fn from_reader<R>(reader: &mut R) -> Result<(Self, Vec<u8>)>
	where
		R: Read + Seek,
	{
		let mut magic_bytes = [0u8; MAGIC_BYTES.len()];
		reader.read_exact(&mut magic_bytes)?;

		if magic_bytes != MAGIC_BYTES {
			return Err(Error::FileHeader);
		}

		let mut version = [0u8; 2];

		reader.read_exact(&mut version)?;
		let version = FileHeaderVersion::from_bytes(version)?;

		// Rewind so we can get the AAD
		reader.rewind()?;

		// read the aad according to the size
		let mut aad = vec![0u8; Self::size(version)];
		reader.read_exact(&mut aad)?;

		// seek back to the start (plus magic bytes and the two version bytes)
		reader.seek(SeekFrom::Start(MAGIC_BYTES.len() as u64 + 2))?;

		// read the header
		let header = match version {
			FileHeaderVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read_exact(&mut algorithm)?;
				let algorithm = Algorithm::from_bytes(algorithm)?;

				let mut nonce = vec![0u8; algorithm.nonce_len()];
				reader.read_exact(&mut nonce)?;

				// read and discard the padding
				reader.read_exact(&mut vec![0u8; 25 - nonce.len()])?;

				let mut keyslot_bytes = [0u8; (KEYSLOT_SIZE * 2)]; // length of 2x keyslots
				let mut keyslots: Vec<Keyslot> = Vec::new();

				reader.read_exact(&mut keyslot_bytes)?;

				for _ in 0..2 {
					Keyslot::from_reader(&mut keyslot_bytes.as_ref())
						.map(|k| keyslots.push(k))
						.ok();
				}

				let metadata = Metadata::from_reader(reader).map_or_else(
					|_| {
						reader.seek(SeekFrom::Start(
							Self::size(version) as u64 + (KEYSLOT_SIZE * 2) as u64,
						))?;
						Ok::<Option<Metadata>, Error>(None)
					},
					|metadata| Ok(Some(metadata)),
				)?;

				let preview_media = PreviewMedia::from_reader(reader).map_or_else(
					|_| {
						let seek_len = metadata.clone().map_or_else(
							|| Self::size(version) as u64 + (KEYSLOT_SIZE * 2) as u64,
							|metadata| {
								Self::size(version) as u64
									+ (KEYSLOT_SIZE * 2) as u64 + metadata.size() as u64
							},
						);

						reader.seek(SeekFrom::Start(seek_len))?;

						Ok::<Option<PreviewMedia>, Error>(None)
					},
					|preview_media| Ok(Some(preview_media)),
				)?;

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

// #[cfg(test)]
// mod test {
// 	use crate::{
// 		crypto::stream::Algorithm,
// 		header::keyslot::{Keyslot, KeyslotVersion},
// 		keys::hashing::{HashingAlgorithm, Params},
// 	};
// 	use std::io::Cursor;

// 	use super::{FileHeader, FileHeaderVersion};

// 	const HEADER_BYTES_NO_ADDITIONAL_OBJECTS: [u8; 228] = [
// 		98, 97, 108, 108, 97, 112, 112, 10, 1, 11, 1, 230, 47, 48, 63, 225, 227, 15, 211, 115, 69,
// 		169, 184, 184, 18, 110, 189, 167, 0, 144, 26, 0, 0, 0, 0, 0, 13, 1, 11, 1, 15, 1, 104, 176,
// 		135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235, 117, 160, 55, 36, 93, 100,
// 		83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74, 205, 239, 253, 48, 239, 249, 203, 121,
// 		126, 231, 52, 38, 49, 154, 254, 234, 41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198,
// 		109, 33, 216, 163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
// 		184, 216, 175, 202, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
// 		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
// 		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
// 		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
// 	];

// 	#[test]
// 	fn deserialize_header() {
// 		let mut reader = Cursor::new(HEADER_BYTES_NO_ADDITIONAL_OBJECTS);
// 		FileHeader::deserialize(&mut reader).unwrap();
// 	}

// 	#[test]
// 	fn serialize_header() {
// 		let header: FileHeader = FileHeader {
// 			version: FileHeaderVersion::V1,
// 			algorithm: Algorithm::XChaCha20Poly1305,
// 			nonce: [
// 				230, 47, 48, 63, 225, 227, 15, 211, 115, 69, 169, 184, 184, 18, 110, 189, 167, 0,
// 				144, 26,
// 			]
// 			.to_vec(),
// 			keyslots: [Keyslot {
// 				version: KeyslotVersion::V1,
// 				algorithm: Algorithm::XChaCha20Poly1305,
// 				hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
// 				content_salt: [
// 					104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235, 117,
// 				],
// 				master_key: [
// 					160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74, 205,
// 					239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234, 41, 113,
// 					169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
// 				],
// 				nonce: [
// 					163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131, 184,
// 					216, 175, 202,
// 				]
// 				.to_vec(),
// 			}]
// 			.to_vec(),
// 			metadata: None,
// 			preview_media: None,
// 		};

// 		let header_bytes = header.serialize().unwrap();

// 		assert_eq!(HEADER_BYTES_NO_ADDITIONAL_OBJECTS.to_vec(), header_bytes)
// 	}

// 	#[test]
// 	#[should_panic]
// 	fn serialize_header_with_too_many_keyslots() {
// 		let header: FileHeader = FileHeader {
// 			version: FileHeaderVersion::V1,
// 			algorithm: Algorithm::XChaCha20Poly1305,
// 			nonce: [
// 				230, 47, 48, 63, 225, 227, 15, 211, 115, 69, 169, 184, 184, 18, 110, 189, 167, 0,
// 				144, 26,
// 			]
// 			.to_vec(),
// 			keyslots: [
// 				Keyslot {
// 					version: KeyslotVersion::V1,
// 					algorithm: Algorithm::XChaCha20Poly1305,
// 					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
// 					content_salt: [
// 						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
// 						117,
// 					],
// 					master_key: [
// 						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
// 						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
// 						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
// 					],
// 					nonce: [
// 						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
// 						184, 216, 175, 202,
// 					]
// 					.to_vec(),
// 				},
// 				Keyslot {
// 					version: KeyslotVersion::V1,
// 					algorithm: Algorithm::XChaCha20Poly1305,
// 					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
// 					content_salt: [
// 						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
// 						117,
// 					],
// 					master_key: [
// 						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
// 						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
// 						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
// 					],
// 					nonce: [
// 						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
// 						184, 216, 175, 202,
// 					]
// 					.to_vec(),
// 				},
// 				Keyslot {
// 					version: KeyslotVersion::V1,
// 					algorithm: Algorithm::XChaCha20Poly1305,
// 					hashing_algorithm: HashingAlgorithm::Argon2id(Params::Standard),
// 					content_salt: [
// 						104, 176, 135, 146, 133, 75, 34, 155, 165, 148, 179, 133, 114, 245, 235,
// 						117,
// 					],
// 					master_key: [
// 						160, 55, 36, 93, 100, 83, 164, 171, 19, 57, 66, 65, 253, 42, 160, 239, 74,
// 						205, 239, 253, 48, 239, 249, 203, 121, 126, 231, 52, 38, 49, 154, 254, 234,
// 						41, 113, 169, 25, 195, 84, 78, 180, 212, 54, 4, 198, 109, 33, 216,
// 					],
// 					nonce: [
// 						163, 148, 79, 207, 121, 142, 102, 39, 169, 31, 55, 41, 231, 248, 65, 131,
// 						184, 216, 175, 202,
// 					]
// 					.to_vec(),
// 				},
// 			]
// 			.to_vec(),
// 			metadata: None,
// 			preview_media: None,
// 		};

// 		header.serialize().unwrap();
// 	}
// }
