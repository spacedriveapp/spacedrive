use crate::primitives::{Algorithm, HashingAlgorithm, Mode, ENCRYPTED_MASTER_KEY_LEN, SALT_LEN};

// Everything contained within this header can be flaunted around with minimal security risk
// The only way this could compromise any data is if a weak password/key was used
// Even then, `argon2id` helps alleiviate this somewhat (brute-forcing it is incredibly tough)
// We also use high memory parameters in order to hinder attacks with ASICs
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

pub enum FileKeyslotVersion {
	V1,
}

impl FileKeyslot {
	fn serialize_keyslot_version(&self) -> [u8; 2] {
		match self.version {
			FileKeyslotVersion::V1 => [0x0D, 0x01],
		}
	}

	fn serialize(&self) -> Vec<u8> {
		let mut keyslot: Vec<u8> = Vec::new();
		keyslot.extend_from_slice(&self.serialize_keyslot_version()); // 2
		keyslot.extend_from_slice(&serialize_algorithm(self.algorithm)); // 4
		keyslot.extend_from_slice(&serialize_mode(self.mode)); // 6
		keyslot.extend_from_slice(&self.salt); // 22
		keyslot.extend_from_slice(&self.master_key); // 70
		keyslot.extend_from_slice(&self.nonce); // 82 OR 94
		keyslot.extend_from_slice(&vec![0u8; 26 - self.nonce.len()]); // 96 total bytes
		keyslot
	}
}

fn serialize_algorithm(algorithm: Algorithm) -> [u8; 2] {
	match algorithm {
		Algorithm::XChaCha20Poly1305 => [0x0B, 0x01],
		Algorithm::Aes256Gcm => [0x0B, 0x02],
	}
}

fn serialize_mode(mode: Mode) -> [u8; 2] {
	match mode {
		Mode::Stream => [0x0C, 0x01],
		Mode::Memory => [0x0C, 0x02],
	}
}

// random values, can be changed
pub const MAGIC_BYTES: [u8; 6] = [0x08, 0xFF, 0x55, 0x32, 0x58, 0x1A];

impl FileHeader {
	fn serialize_header_version(&self) -> [u8; 2] {
		match self.version {
			FileHeaderVersion::V1 => [0x0A, 0x01],
		}
	}

	pub fn serialize(&self) -> Vec<u8> {
		let mut header: Vec<u8> = Vec::new();
		header.extend_from_slice(&MAGIC_BYTES); // 6
		header.extend_from_slice(&self.serialize_header_version()); // 8
		header.extend_from_slice(&serialize_algorithm(self.algorithm)); // 10
		header.extend_from_slice(&serialize_mode(self.mode)); // 12
		header.extend_from_slice(&self.nonce); // 20 OR 32
		header.extend_from_slice(&vec![0u8; 24 - self.nonce.len()]); // padded until 36 bytes

		todo!()
	}
}
