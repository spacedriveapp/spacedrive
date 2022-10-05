use crate::{
	error::Error,
	keys::hashing::Params,
	primitives::{Algorithm, HashingAlgorithm, Mode},
};

use super::file::{FileHeaderVersion, FileKeyslotVersion};

impl FileHeaderVersion {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0A, 0x01],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0A, 0x01] => Ok(Self::V1),
			_ => Err(Error::FileHeader),
		}
	}
}

impl FileKeyslotVersion {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0D, 0x01],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0D, 0x01] => Ok(Self::V1),
			_ => Err(Error::FileHeader),
		}
	}
}

impl HashingAlgorithm {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::Argon2id(p) => match p {
				Params::Standard => [0x0F, 0x01],
				Params::Hardened => [0x0F, 0x02],
				Params::Paranoid => [0x0F, 0x03],
			},
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0F, 0x01] => Ok(Self::Argon2id(Params::Standard)),
			[0x0F, 0x02] => Ok(Self::Argon2id(Params::Hardened)),
			[0x0F, 0x03] => Ok(Self::Argon2id(Params::Paranoid)),
			_ => Err(Error::FileHeader),
		}
	}
}

impl Algorithm {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::XChaCha20Poly1305 => [0x0B, 0x01],
			Self::Aes256Gcm => [0x0B, 0x02],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0B, 0x01] => Ok(Self::XChaCha20Poly1305),
			[0x0B, 0x02] => Ok(Self::Aes256Gcm),
			_ => Err(Error::FileHeader),
		}
	}
}

impl Mode {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::Stream => [0x0C, 0x01],
			Self::Memory => [0x0C, 0x02],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0C, 0x01] => Ok(Self::Stream),
			[0x0C, 0x02] => Ok(Self::Memory),
			_ => Err(Error::FileHeader),
		}
	}
}
