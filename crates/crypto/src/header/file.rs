use std::io::{Read, Seek, Write};

use zeroize::Zeroize;

use crate::{
	error::Error,
	objects::memory::MemoryDecryption,
	primitives::{Algorithm, Mode, MASTER_KEY_LEN},
	protected::Protected,
};

use super::keyslot::Keyslot;

/// These are used to quickly and easily identify Spacedrive-encrypted files
/// Random values - can be changed (up until 0.1.0)
pub const MAGIC_BYTES: [u8; 6] = [0x08, 0xFF, 0x55, 0x32, 0x58, 0x1A];

// Everything contained within this header can be flaunted around with minimal security risk
// The only way this could compromise any data is if a weak password/key was used
// Even then, `argon2id` helps alleiviate this somewhat (brute-forcing it is incredibly tough)
// We also use high memory parameters in order to hinder attacks with ASICs
// There should be no more than two keyslots in this header type
pub struct FileHeader {
	pub version: FileHeaderVersion,
	pub algorithm: Algorithm,
	pub mode: Mode,
	pub nonce: Vec<u8>,
	pub keyslots: Vec<Keyslot>,
}

/// This defines the main file header version
pub enum FileHeaderVersion {
	V1,
}

impl FileHeader {
	#[must_use]
	pub fn new(
		version: FileHeaderVersion,
		algorithm: Algorithm,
		nonce: Vec<u8>,
		keyslots: Vec<Keyslot>,
	) -> Self {
		Self {
			version,
			algorithm,
			mode: Mode::Stream,
			nonce,
			keyslots,
		}
	}

	/// This is a helper function to decrypt a master key from a set of keyslots
	/// It's easier to call this on the header for now - but this may be changed in the future
	/// You receive an error if the password doesn't match
	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Protected<[u8; 32]>, Error> {
		let mut master_key = [0u8; MASTER_KEY_LEN];

		for keyslot in &self.keyslots {
			let key = keyslot
				.hashing_algorithm
				.hash(password.clone(), keyslot.salt)
				.map_err(|_| Error::PasswordHash)?;

			let decryptor =
				MemoryDecryption::new(key, keyslot.algorithm).map_err(|_| Error::MemoryModeInit)?;
			if let Ok(mut decrypted_master_key) =
				decryptor.decrypt(keyslot.master_key.as_ref(), &keyslot.nonce)
			{
				master_key.copy_from_slice(&decrypted_master_key);
				decrypted_master_key.zeroize();
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
				aad.extend_from_slice(&MAGIC_BYTES); // 6
				aad.extend_from_slice(&self.version.serialize()); // 8
				aad.extend_from_slice(&self.algorithm.serialize()); // 10
				aad.extend_from_slice(&self.mode.serialize()); // 12
				aad.extend_from_slice(&self.nonce); // 20 OR 32
				aad.extend_from_slice(&vec![0u8; 24 - self.nonce.len()]); // padded until 36 bytes
				aad
			}
		}
	}

	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			FileHeaderVersion::V1 => {
				let mut header: Vec<u8> = Vec::new();
				header.extend_from_slice(&MAGIC_BYTES); // 6
				header.extend_from_slice(&self.version.serialize()); // 8
				header.extend_from_slice(&self.algorithm.serialize()); // 10
				header.extend_from_slice(&self.mode.serialize()); // 12
				header.extend_from_slice(&self.nonce); // 20 OR 32
				header.extend_from_slice(&vec![0u8; 24 - self.nonce.len()]); // padded until 36 bytes

				for keyslot in &self.keyslots {
					header.extend_from_slice(&keyslot.serialize());
				}

				for _ in 0..(2 - self.keyslots.len()) {
					header.extend_from_slice(&[0u8; 96]);
				}

				header
			}
		}
	}

	#[must_use]
	pub const fn length(&self) -> usize {
		match self.version {
			FileHeaderVersion::V1 => 228,
		}
	}

	#[must_use]
	pub const fn aad_length(&self) -> usize {
		match self.version {
			FileHeaderVersion::V1 => 36,
		}
	}

	// The AAD retrieval here could be optimised - we do rewind a couple of times
	/// This deserializes a header directly from a reader, and leaves the reader at the start of the encrypted data
	/// It returns both the header, and the AAD that should be used for decryption
	pub fn deserialize<R>(reader: &mut R) -> Result<(Self, Vec<u8>), Error>
	where
		R: Read + Seek,
	{
		let mut magic_bytes = [0u8; 6];
		reader.read(&mut magic_bytes).map_err(Error::Io)?;

		if magic_bytes != MAGIC_BYTES {
			return Err(Error::FileHeader);
		}

		let mut version = [0u8; 2];

		reader.read(&mut version).map_err(Error::Io)?;
		let version = FileHeaderVersion::deserialize(version)?;

		let header = match version {
			FileHeaderVersion::V1 => {
				let mut algorithm = [0u8; 2];
				reader.read(&mut algorithm).map_err(Error::Io)?;
				let algorithm = Algorithm::deserialize(algorithm)?;

				let mut mode = [0u8; 2];
				reader.read(&mut mode).map_err(Error::Io)?;
				let mode = Mode::deserialize(mode)?;

				let mut nonce = vec![0u8; algorithm.nonce_len(mode)];
				reader.read(&mut nonce).map_err(Error::Io)?;

				// read and discard the padding
				reader
					.read(&mut vec![0u8; 24 - nonce.len()])
					.map_err(Error::Io)?;

				let mut keyslots: Vec<Keyslot> = Vec::new();

				for _ in 0..2 {
					if let Ok(keyslot) = Keyslot::deserialize(reader) {
						keyslots.push(keyslot);
					}
				}

				Self {
					version,
					algorithm,
					mode,
					nonce,
					keyslots,
				}
			}
		};

		// Rewind so we can get the AAD
		reader.rewind().map_err(Error::Io)?;

		let mut aad = vec![0u8; header.aad_length()];
		reader.read(&mut aad).map_err(Error::Io)?;

		// We return the cursor position to the end of the header,
		// So that the encrypted data can be read directly afterwards
		reader
			.seek(std::io::SeekFrom::Start(header.length() as u64))
			.map_err(Error::Io)?;

		Ok((header, aad))
	}
}
