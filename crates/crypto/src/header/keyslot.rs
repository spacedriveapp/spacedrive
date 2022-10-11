use std::io::{Read, Seek};

use crate::{
	error::Error,
	objects::stream::StreamEncryption,
	primitives::{
		generate_nonce, generate_salt, to_array, Algorithm, HashingAlgorithm,
		ENCRYPTED_MASTER_KEY_LEN, MASTER_KEY_LEN, SALT_LEN,
	},
	protected::Protected,
};

/// A keyslot - 96 bytes (as of V1), and contains all the information for future-proofing while keeping the size reasonable
///
/// The algorithm (should) be inherited from the parent header, but that's not a guarantee so we include it here too
///
/// The master key needs to be encrypted before being added to the keyslot (the master key have no AAD).
pub struct Keyslot {
	pub version: KeyslotVersion,
	pub algorithm: Algorithm,                // encryption algorithm
	pub hashing_algorithm: HashingAlgorithm, // password hashing algorithm
	pub salt: [u8; SALT_LEN],
	pub master_key: [u8; ENCRYPTED_MASTER_KEY_LEN], // this is encrypted so we can store it
	pub nonce: Vec<u8>,
}

/// This defines the keyslot version
///
/// The goal is to not increment this much, but it's here in case we need to make breaking changes
pub enum KeyslotVersion {
	V1,
}

impl Keyslot {
	/// This handles encrypting the master key.
	///
	/// You will need to provide the user's password/key, and a generated master key (this can't generate it, otherwise it can't be used elsewhere)
	pub fn new(
		version: KeyslotVersion,
		algorithm: Algorithm,
		hashing_algorithm: HashingAlgorithm,
		password: Protected<Vec<u8>>,
		master_key: &Protected<[u8; MASTER_KEY_LEN]>,
	) -> Result<Self, Error> {
		let salt = generate_salt();
		let nonce = generate_nonce(algorithm.nonce_len());

		let hashed_password = hashing_algorithm.hash(password, salt).unwrap();

		let encrypted_master_key: [u8; 48] = to_array(StreamEncryption::encrypt_bytes(
			hashed_password,
			&nonce,
			algorithm,
			master_key.expose(),
			&[],
		)?)?;

		Ok(Self {
			version,
			algorithm,
			hashing_algorithm,
			salt,
			master_key: encrypted_master_key,
			nonce,
		})
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
				keyslot.extend_from_slice(&self.salt); // 22
				keyslot.extend_from_slice(&self.master_key); // 70
				keyslot.extend_from_slice(&self.nonce); // 78 or 90
				keyslot.extend_from_slice(&vec![0u8; 26 - self.nonce.len()]); // 96 total bytes
				keyslot
			}
		}
	}

	/// This function reads a keyslot from a reader
	///
	/// It will leave the cursor at the end of the keyslot on success
	///
	/// The cursor will not be rewound on error.
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

				let mut salt = [0u8; SALT_LEN];
				reader.read(&mut salt).map_err(Error::Io)?;

				let mut master_key = [0u8; ENCRYPTED_MASTER_KEY_LEN];
				reader.read(&mut master_key).map_err(Error::Io)?;

				let mut nonce = vec![0u8; algorithm.nonce_len()];
				reader.read(&mut nonce).map_err(Error::Io)?;

				reader
					.read(&mut vec![0u8; 26 - nonce.len()])
					.map_err(Error::Io)?;

				let keyslot = Self {
					version,
					algorithm,
					hashing_algorithm,
					salt,
					master_key,
					nonce,
				};

				Ok(keyslot)
			}
		}
	}
}
