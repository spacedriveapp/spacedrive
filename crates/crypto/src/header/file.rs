use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::{
	error::Error,
	objects::stream::StreamDecryption,
	primitives::{Algorithm, MASTER_KEY_LEN},
	protected::Protected,
};

use super::{keyslot::Keyslot, metadata::Metadata, preview_media::PreviewMedia};

/// These are used to quickly and easily identify Spacedrive-encrypted files
/// These currently are set as "ballapp"
pub const MAGIC_BYTES: [u8; 7] = [0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70];

// Everything contained within this header can be flaunted around with minimal security risk
// The only way this could compromise any data is if a weak password/key was used
// Even then, `argon2id` helps alleviate this somewhat (brute-forcing it is incredibly tough)
// We also use high memory parameters in order to hinder attacks with ASICs
// There should be no more than two keyslots in this header type
pub struct FileHeader {
	pub version: FileHeaderVersion,
	pub algorithm: Algorithm,
	pub nonce: Vec<u8>,
	pub keyslots: Vec<Keyslot>,
	pub metadata: Option<Metadata>,
	pub preview_media: Option<PreviewMedia>,
}

/// This defines the main file header version
#[derive(Clone, Copy)]
pub enum FileHeaderVersion {
	V1,
}

// This includes the magic bytes at the start of the file
#[must_use]
pub const fn aad_length(version: FileHeaderVersion) -> usize {
	match version {
		FileHeaderVersion::V1 => 36,
	}
}

impl FileHeader {
	#[must_use]
	pub fn new(
		version: FileHeaderVersion,
		algorithm: Algorithm,
		nonce: Vec<u8>,
		keyslots: Vec<Keyslot>,
		metadata: Option<Metadata>,
		preview_media: Option<PreviewMedia>,
	) -> Self {
		Self {
			version,
			algorithm,
			nonce,
			keyslots,
			metadata,
			preview_media,
		}
	}

	/// This is a helper function to decrypt a master key from a set of keyslots
	/// It's easier to call this on the header for now - but this may be changed in the future
	/// You receive an error if the password doesn't match
	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<[u8; MASTER_KEY_LEN]>, Error> {
		let mut master_key = [0u8; MASTER_KEY_LEN];

		for keyslot in &self.keyslots {
			let key = keyslot
				.hashing_algorithm
				.hash(password.clone(), keyslot.salt)
				.map_err(|_| Error::PasswordHash)?;

			if let Ok(decrypted_master_key) = StreamDecryption::decrypt_bytes(
				key,
				&keyslot.nonce,
				keyslot.algorithm,
				&keyslot.master_key,
				&[],
			) {
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
		writer.write(&self.serialize()).map_err(Error::Io)?;
		Ok(())
	}

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

	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			FileHeaderVersion::V1 => {
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

				// TODO(brxken128): metadata serialization

				if let Some(preview_media) = self.preview_media.clone() {
					header.extend_from_slice(&preview_media.serialize());
				}

				header
			}
		}
	}

	// This includes the magic bytes at the start of the file
	#[must_use]
	pub const fn length(&self) -> usize {
		match self.version {
			FileHeaderVersion::V1 => 222 + MAGIC_BYTES.len(),
		}
	}

	// The AAD retrieval here could be optimised - we do rewind a couple of times
	/// This deserializes a header directly from a reader, and leaves the reader at the start of the encrypted data
	/// It returns both the header, and the AAD that should be used for decryption
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

				let preview_media = if let Ok(preview_media) = PreviewMedia::deserialize(reader) {
					Some(preview_media)
				} else {
					// TODO(brxken128): this will need changing once we add metadata
					// header/aad area, keyslot area
					reader.seek(SeekFrom::Start(36 + 192)).map_err(Error::Io)?;
					None
				};

				Self {
					version,
					algorithm,
					nonce,
					keyslots,
					metadata: None, // set these to none temporarily
					preview_media,
				}
			}
		};

		Ok((header, aad))
	}
}
