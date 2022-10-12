//! This module defines all of the serialization and deserialization rules for the header items
//!
//! It contains `byte -> enum` and `enum -> byte` conversions for everything that could be written to a header (except headers, keyslots, and other header items)

use crate::{
	crypto::stream::Algorithm,
	error::Error,
	keys::hashing::{HashingAlgorithm, Params},
};

use super::{
	file::FileHeaderVersion, keyslot::KeyslotVersion, metadata::MetadataVersion,
	preview_media::PreviewMediaVersion,
};

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

impl KeyslotVersion {
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

impl PreviewMediaVersion {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0E, 0x01],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x0E, 0x01] => Ok(Self::V1),
			_ => Err(Error::FileHeader),
		}
	}
}

impl MetadataVersion {
	#[must_use]
	pub const fn serialize(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x1F, 0x01],
		}
	}

	pub const fn deserialize(bytes: [u8; 2]) -> Result<Self, Error> {
		match bytes {
			[0x1F, 0x01] => Ok(Self::V1),
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
