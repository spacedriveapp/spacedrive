use std::io::{Read, Seek};

use crate::{
	error::Error,
	keys::hashing::Params,
	primitives::{Algorithm, HashingAlgorithm, Mode, ENCRYPTED_MASTER_KEY_LEN, SALT_LEN},
};

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
	pub keyslots: Vec<FileKeyslot>,
}

// I chose to add the mode for uniformity, that way it's clear that master keys are encrypted differently
// I opted to include a hashing algorithm - it's 2 additional bytes but it may save a version iteration in the future
// This also may become the universal keyslot standard, so maybe `FileKeyslot` isn't the best name
// Keyslots should inherit the parent's encryption algorithm, but I chose to add it anyway just in case
pub struct FileKeyslot {
	pub version: FileKeyslotVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub mode: Mode,
	pub salt: [u8; SALT_LEN],
	pub nonce: Vec<u8>,
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is encrypted so we can store it
}

// The goal is to try and keep these in sync as much as possible,
// but the option to increment one is always there.
// I designed with a lot of future-proofing, even if it doesn't fit our current plans
pub enum FileHeaderVersion {
	V1,
}

// TODO(brxken128): maybe use a deserialization error
// TODO(brxken128): move all serialization/deserialization rules
impl FileHeaderVersion {
	pub fn serialize(&self) -> [u8; 2] {
		match self {
			FileHeaderVersion::V1 => [0x0A, 0x01],
		}
	}

	pub fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0A, 0x01] => Ok(FileHeaderVersion::V1),
			_ => Err(Error::FileHeader),
		}
	}
}

pub enum FileKeyslotVersion {
	V1,
}

impl FileKeyslotVersion {
	pub fn serialize(&self) -> [u8; 2] {
		match self {
			FileKeyslotVersion::V1 => [0x0D, 0x01],
		}
	}

	pub fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0D, 0x01] => Ok(FileKeyslotVersion::V1),
			_ => Err(Error::FileHeader),
		}
	}
}

impl HashingAlgorithm {
	pub fn serialize(&self) -> [u8; 2] {
		match self {
			HashingAlgorithm::Argon2id(p) => match p {
				Params::Standard => [0x0F, 0x01],
				Params::Hardened => [0x0F, 0x02],
				Params::Paranoid => [0x0F, 0x03],
			},
		}
	}

	pub fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0F, 0x01] => Ok(HashingAlgorithm::Argon2id(Params::Standard)),
			[0x0F, 0x02] => Ok(HashingAlgorithm::Argon2id(Params::Hardened)),
			[0x0F, 0x03] => Ok(HashingAlgorithm::Argon2id(Params::Paranoid)),
			_ => Err(Error::FileHeader),
		}
	}
}

impl FileKeyslot {
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			FileKeyslotVersion::V1 => {
				let mut keyslot: Vec<u8> = Vec::new();
				keyslot.extend_from_slice(&self.version.serialize()); // 2
				keyslot.extend_from_slice(&self.algorithm.serialize()); // 4
				keyslot.extend_from_slice(&self.hashing_algorithm.serialize()); // 6
				keyslot.extend_from_slice(&self.mode.serialize()); // 8
				keyslot.extend_from_slice(&self.salt); // 24
				keyslot.extend_from_slice(&self.master_key); // 72
				keyslot.extend_from_slice(&self.nonce); // 82 OR 94
				keyslot.extend_from_slice(&vec![0u8; 24 - self.nonce.len()]); // 96 total bytes
				keyslot
			}
		}
	}

	pub fn deserialize<R>(reader: &mut R) -> Result<FileKeyslot, Error>
	where
		R: Read + Seek,
	{
		let mut version = [0u8; 2];
		reader.read(&mut version).map_err(Error::Io)?;
		let version = FileKeyslotVersion::deserialize(version)?;

		match version {
			FileKeyslotVersion::V1 => {
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

				let mut nonce = vec![0u8; algorithm.nonce_len(mode)];
				reader.read(&mut nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 26 - nonce.len()])
					.map_err(Error::Io)?;

				let keyslot = FileKeyslot {
					version,
					algorithm,
					hashing_algorithm,
					mode,
					salt,
					master_key,
					nonce,
				};

				Ok(keyslot)
			}
		}
	}
}

impl Algorithm {
	pub fn serialize(&self) -> [u8; 2] {
		match self {
			Algorithm::XChaCha20Poly1305 => [0x0B, 0x01],
			Algorithm::Aes256Gcm => [0x0B, 0x02],
		}
	}

	pub fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0B, 0x01] => Ok(Algorithm::XChaCha20Poly1305),
			[0x0B, 0x02] => Ok(Algorithm::Aes256Gcm),
			_ => Err(Error::FileHeader),
		}
	}
}

impl Mode {
	pub fn serialize(&self) -> [u8; 2] {
		match self {
			Mode::Stream => [0x0C, 0x01],
			Mode::Memory => [0x0C, 0x02],
		}
	}

	pub fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0C, 0x01] => Ok(Mode::Stream),
			[0x0C, 0x02] => Ok(Mode::Memory),
			_ => Err(Error::FileHeader),
		}
	}
}

// random values, can be changed
pub const MAGIC_BYTES: [u8; 6] = [0x08, 0xFF, 0x55, 0x32, 0x58, 0x1A];

impl FileHeader {
	pub fn create_aad(&self) -> Vec<u8> {
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

	pub fn length(&self) -> usize {
		match self.version {
			FileHeaderVersion::V1 => 228,
		}
	}

	pub fn aad_length(&self) -> usize {
		match self.version {
			FileHeaderVersion::V1 => 36,
		}
	}

	// This returns both the Header and the AAD
	// The AAD retrieval here could be optimised - we do rewind a couple of times
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

				let mut keyslots: Vec<FileKeyslot> = Vec::new();

				for _ in 0..2 {
					if let Ok(keyslot) = FileKeyslot::deserialize(reader) {
						keyslots.push(keyslot);
					}
				}

				let file_header = FileHeader {
					version,
					algorithm,
					mode,
					nonce,
					keyslots,
				};

				file_header
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
