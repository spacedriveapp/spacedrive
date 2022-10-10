use std::io::{Cursor, Read, Seek};

use zeroize::Zeroize;

use crate::{
	error::Error,
	objects::{memory::MemoryDecryption, stream::StreamDecryption},
	primitives::{
		Algorithm, HashingAlgorithm, Mode, ENCRYPTED_MASTER_KEY_LEN, MASTER_KEY_LEN, SALT_LEN,
	},
	protected::Protected,
};

/// `mode` here refers to how the data within is encrypted - this should be streaming mode.
///
/// The master key is always encrypted with memory mode.
///
/// Here we have two nonces - one is for the encrypted master key, and the other is for the data itself.
///
/// We need the `media_length` so we're able to determine how long this header object is
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
	pub media: Vec<u8>,
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
		media: Vec<u8>,
	) -> Self {
		Self {
			version,
			algorithm,
			hashing_algorithm,
			mode: Mode::Stream,
			salt,
			master_key: encrypted_master_key,
			master_key_nonce,
			media_nonce,
			media_length: media.len(),
			media,
		}
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
				preview_media.extend_from_slice(&self.media_length.to_le_bytes()); // 128 total bytes
				preview_media.extend_from_slice(&self.media); // this can vary in length
				preview_media
			}
		}
	}

    // This function assumes preview media will be encrypted in streaming mode
	pub fn decrypt_preview_media(&self, hashed_key: Protected<[u8; 32]>) -> Result<Vec<u8>, Error> {
		let mut master_key = [0u8; MASTER_KEY_LEN];

		let decryptor =
			MemoryDecryption::new(hashed_key, self.algorithm).map_err(|_| Error::MemoryModeInit)?;
		let master_key = if let Ok(mut decrypted_master_key) =
			decryptor.decrypt(self.master_key.as_ref(), &self.master_key_nonce)
		{
			master_key.copy_from_slice(&decrypted_master_key);
			decrypted_master_key.zeroize();
			Ok(Protected::new(master_key))
		} else {
			Err(Error::IncorrectPassword)
		}?;

		let decryptor = StreamDecryption::new(master_key, &self.media_nonce, self.algorithm)
			.map_err(|_| Error::StreamModeInit)?;

		// Maybe this isn't the most efficient way to read this data - cloning may be costly depending on the size of the media
		let mut reader = Cursor::new(self.media.clone());
		let mut writer = Cursor::new(Vec::<u8>::new());

		decryptor
			.decrypt_streams(&mut reader, &mut writer, &[])
			.map_err(|_| Error::Decrypt)?;

		Ok(writer.into_inner())
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

				let mut master_key_nonce = vec![0u8; algorithm.nonce_len(Mode::Memory)];
				reader.read(&mut master_key_nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 24 - master_key_nonce.len()])
					.map_err(Error::Io)?;

				let mut media_nonce = vec![0u8; algorithm.nonce_len(mode)];
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
					mode,
					salt,
					master_key,
					master_key_nonce,
					media_nonce,
					media_length,
					media,
				};

				Ok(preview_media)
			}
		}
	}
}
