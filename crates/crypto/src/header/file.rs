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

// There's no need to include `Mode` here, as master keys/keyslots should always use "memory" mode
// Stream encryption isn't viable for 32 bytes of data
// I opted to include a hashing algorithm - it's 2 additional bytes but it may save a version iteration in the future
// This also may become the universal keyslot standard, so maybe `FileKeyslot` isn't the best name
pub struct FileKeyslot {
	pub version: FileKeyslotVersion,
	pub hashing_algorithm: HashingAlgorithm,
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

impl FileHeader {
	pub fn serialize(&self) -> Vec<u8> {
		todo!()
	}
}
