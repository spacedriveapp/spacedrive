//! This module defines all of the serialization and deserialization rules for the header items
//!
//! It contains `byte -> enum` and `enum -> byte` conversions for everything that could be written to a header (except headers, keyslots, and other header items)
use std::fmt::Display;

use crate::{
	crypto::stream::Algorithm,
	keys::hashing::{HashingAlgorithm, Params},
	Error, Result,
};

use super::{
	file::FileHeaderVersion, keyslot::KeyslotVersion, metadata::MetadataVersion,
	preview_media::PreviewMediaVersion,
};

impl FileHeaderVersion {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0A, 0x01],
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0x0A, 0x01] => Ok(Self::V1),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for FileHeaderVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::V1 => write!(f, "V1"),
		}
	}
}

impl KeyslotVersion {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0D, 0x01],
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0x0D, 0x01] => Ok(Self::V1),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for KeyslotVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::V1 => write!(f, "V1"),
		}
	}
}

impl PreviewMediaVersion {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x0E, 0x01],
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0x0E, 0x01] => Ok(Self::V1),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for PreviewMediaVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::V1 => write!(f, "V1"),
		}
	}
}

impl MetadataVersion {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::V1 => [0x1F, 0x01],
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0x1F, 0x01] => Ok(Self::V1),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for MetadataVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::V1 => write!(f, "V1"),
		}
	}
}

impl HashingAlgorithm {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::Argon2id(p) => match p {
				Params::Standard => [0xA2, 0x01],
				Params::Hardened => [0xA2, 0x02],
				Params::Paranoid => [0xA2, 0x03],
			},
			Self::BalloonBlake3(p) => match p {
				Params::Standard => [0xB3, 0x01],
				Params::Hardened => [0xB3, 0x02],
				Params::Paranoid => [0xB3, 0x03],
			},
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0xA2, 0x01] => Ok(Self::Argon2id(Params::Standard)),
			[0xA2, 0x02] => Ok(Self::Argon2id(Params::Hardened)),
			[0xA2, 0x03] => Ok(Self::Argon2id(Params::Paranoid)),
			[0xB3, 0x01] => Ok(Self::BalloonBlake3(Params::Standard)),
			[0xB3, 0x02] => Ok(Self::BalloonBlake3(Params::Hardened)),
			[0xB3, 0x03] => Ok(Self::BalloonBlake3(Params::Paranoid)),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for HashingAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Argon2id(p) => write!(f, "Argon2id ({p})"),
			Self::BalloonBlake3(p) => write!(f, "BLAKE3-Balloon ({p})"),
		}
	}
}

impl Display for Params {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Standard => write!(f, "Standard"),
			Self::Hardened => write!(f, "Hardened"),
			Self::Paranoid => write!(f, "Paranoid"),
		}
	}
}

impl Algorithm {
	#[must_use]
	pub const fn to_bytes(&self) -> [u8; 2] {
		match self {
			Self::XChaCha20Poly1305 => [0x0B, 0x01],
			Self::Aes256Gcm => [0x0B, 0x02],
		}
	}

	pub const fn from_bytes(bytes: [u8; 2]) -> Result<Self> {
		match bytes {
			[0x0B, 0x01] => Ok(Self::XChaCha20Poly1305),
			[0x0B, 0x02] => Ok(Self::Aes256Gcm),
			_ => Err(Error::Serialization),
		}
	}
}

impl Display for Algorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::XChaCha20Poly1305 => write!(f, "XChaCha20-Poly1305"),
			Self::Aes256Gcm => write!(f, "AES-256-GCM"),
		}
	}
}
