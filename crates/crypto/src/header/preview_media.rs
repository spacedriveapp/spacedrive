use std::io::{Read, Seek};

use crate::{
	error::Error,
	primitives::{Algorithm, HashingAlgorithm, Mode, ENCRYPTED_MASTER_KEY_LEN, SALT_LEN},
};

#[derive(Clone)]
pub struct PreviewMedia {
	pub version: PreviewMediaVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub mode: Mode,
	pub salt: [u8; SALT_LEN],
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN],
	pub master_key_nonce: Vec<u8>,
	pub media_nonce: Vec<u8>,
	pub media_length: usize,
	pub preview_media: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum PreviewMediaVersion {
	V1,
}

impl PreviewMedia {
    #[allow(clippy::too_many_arguments)]
	#[must_use]
	pub fn new(
		version: PreviewMediaVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		salt: [u8; SALT_LEN],
		encrypted_master_key: [u8; ENCRYPTED_MASTER_KEY_LEN],
		master_key_nonce: Vec<u8>,
		media_nonce: Vec<u8>,
		preview_media: Vec<u8>,
	) -> Self {
		Self {
			version,
			algorithm,
			hashing_algorithm,
			mode: Mode::Memory,
			salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			media_nonce,
			media_length: preview_media.len(),
			preview_media,
		}
	}

	/// This returns the full length of this header item, including the encrypted preview media itself
	#[must_use]
	pub const fn get_length(&self) -> usize {
		117 + self.media_length
	}

	fn serialize_media_length(&self) -> Vec<u8> {
		// length_bytes needs to be 21 digits, with zeroes prepending it
		// I'm unsure as to whether or not this is the best way to go about it
		// We add a lot of additional data (13 bytes), but we skip differences between little and big endian platforms
		// We also avoid x64 and x86 differences (4 byte usize vs 8 byte usize)
		// This function will likely not be final
		let mut length_bytes: Vec<u8> = Vec::new();
		length_bytes.extend_from_slice(self.media_length.to_string().as_bytes());
		for _ in 0..(21 - self.media_length.to_string().len()) {
			length_bytes.insert(0, 0x30);
		}
		length_bytes
	}

	/// This function is used to serialize a preview media header item into bytes
	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			PreviewMediaVersion::V1 => {
				let mut preview_media: Vec<u8> = Vec::new();
				preview_media.extend_from_slice(&self.version.serialize()); // 2
				preview_media.extend_from_slice(&self.algorithm.serialize()); // 4
				preview_media.extend_from_slice(&self.hashing_algorithm.serialize()); // 6
				preview_media.extend_from_slice(&self.mode.serialize()); // 8
				preview_media.extend_from_slice(&self.salt); // 24
				preview_media.extend_from_slice(&self.master_key); // 72
				preview_media.extend_from_slice(&self.master_key_nonce); // 84 or 96
				preview_media.extend_from_slice(&vec![0u8; 24 - self.master_key_nonce.len()]); // 96
				preview_media.extend_from_slice(&self.media_nonce); // 108 or 120
				preview_media.extend_from_slice(&vec![0u8; 24 - self.media_nonce.len()]); // 120
				preview_media.extend_from_slice(&self.serialize_media_length()); // 141 total bytes
				preview_media.extend_from_slice(&self.preview_media); // this can vary in length
				preview_media
			}
		}
	}

	/// This function reads a preview media header item from a reader
	pub fn deserialize<R>(reader: &mut R) -> Result<Self, Error>
	where
		R: Read + Seek,
	{
		let mut version = [0u8; 2];
		reader.read(&mut version).map_err(Error::Io)?;
		let version = PreviewMediaVersion::deserialize(version)?;

		match version {
			PreviewMediaVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read(&mut algorithm).map_err(Error::Io)?;
				let algorithm = Algorithm::deserialize(algorithm)?;

				let mut hashing_algorithm = [0u8; 2];
				reader.read(&mut hashing_algorithm).map_err(Error::Io)?;
				let hashing_algorithm = HashingAlgorithm::deserialize(hashing_algorithm)?;

				let mut mode = [0u8; 2];
				reader.read(&mut mode).map_err(Error::Io)?;
				let mode = Mode::deserialize(mode)?;

				let mut salt = [0u8; SALT_LEN];
				reader.read(&mut salt).map_err(Error::Io)?;

				let mut master_key = [0u8; ENCRYPTED_MASTER_KEY_LEN];
				reader.read(&mut master_key).map_err(Error::Io)?;

				let mut master_key_nonce = vec![0u8; algorithm.nonce_len(mode)];
				reader.read(&mut master_key_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 24 - master_key_nonce.len()])
					.map_err(Error::Io)?;

				let mut media_nonce = vec![0u8; algorithm.nonce_len(mode)];
				reader.read(&mut media_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 24 - media_nonce.len()])
					.map_err(Error::Io)?;

				let mut media_length = vec![0u8; 21];
				reader.read(&mut media_length).map_err(Error::Io)?;

				let media_length: usize = String::from_utf8(media_length)
					.map_err(|_| Error::MediaLengthParse)?
					.parse::<usize>()
					.map_err(|_| Error::MediaLengthParse)?;

				let mut preview_media = vec![0u8; media_length];
				reader.read(&mut preview_media).map_err(Error::Io)?;

				let preview_media = Self {
					version,
					algorithm,
					hashing_algorithm,
					mode,
					salt,
					master_key,
					master_key_nonce,
					media_nonce,
					media_length,
					preview_media,
				};

				Ok(preview_media)
			}
		}
	}
}
