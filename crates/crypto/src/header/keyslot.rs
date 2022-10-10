use std::io::{Read, Seek};

use crate::{
	error::Error,
	primitives::{Algorithm, HashingAlgorithm, Mode, ENCRYPTED_MASTER_KEY_LEN, SALT_LEN},
};

/// A keyslot. 96 bytes, and contains all the information for future-proofing while keeping the size reasonable
///
/// The mode was added so others can see that master keys are encrypted differently from data
///
/// The algorithm (should) be inherited from the parent header, but that's not a guarantee
pub struct Keyslot {
	pub version: KeyslotVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub mode: Mode,
	pub salt: [u8; SALT_LEN],
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is encrypted so we can store it
	pub nonce: Vec<u8>,
}

/// This defines the keyslot version
pub enum KeyslotVersion {
	V1,
}

impl Keyslot {
	#[must_use]
	pub fn new(
		version: KeyslotVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		salt: [u8; SALT_LEN],
		encrypted_master_key: [u8; ENCRYPTED_MASTER_KEY_LEN],
		nonce: Vec<u8>,
	) -> Self {
		Self {
			version,
			algorithm,
			hashing_algorithm,
			mode: Mode::Memory,
			salt,
			master_key: encrypted_master_key,
			nonce,
		}
	}
	/// This function is used to serialize a keyslot into bytes
	#[must_use]
	pub fn serialize(&self) -> Vec<u8> {
		match self.version {
			KeyslotVersion::V1 => {
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

	/// This function reads a keyslot from a reader
	pub fn deserialize<R>(reader: &mut R) -> Result<Self, Error>
	where
		R: Read + Seek,
	{
		let mut version = [0u8; 2];
		reader.read(&mut version).map_err(Error::Io)?;
		let version = KeyslotVersion::deserialize(version)?;

		match version {
			KeyslotVersion::V1 => {
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

				let keyslot = Self {
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
