// TODO: Document all types in this file

use std::{
	hash::{Hash, Hasher},
	str::FromStr,
};

use base64::{engine::general_purpose, Engine};
use ed25519_dalek::{VerifyingKey, SECRET_KEY_LENGTH};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use zeroize::ZeroizeOnDrop;

pub const REMOTE_IDENTITY_LEN: usize = 32;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum IdentityErr {
	#[error("{0}")]
	Dalek(#[from] ed25519_dalek::ed25519::Error),
	#[error("Invalid key length")]
	InvalidKeyLength,
}

/// TODO
#[derive(Debug, Clone, ZeroizeOnDrop)]
pub struct Identity(ed25519_dalek::SigningKey);

impl PartialEq for Identity {
	fn eq(&self, other: &Self) -> bool {
		self.0.verifying_key().eq(&other.0.verifying_key())
	}
}

impl Default for Identity {
	fn default() -> Self {
		Self(ed25519_dalek::SigningKey::generate(&mut OsRng))
	}
}

impl Identity {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityErr> {
		Ok(Self(ed25519_dalek::SigningKey::from_bytes(
			bytes[..SECRET_KEY_LENGTH]
				.try_into()
				.map_err(|_| IdentityErr::InvalidKeyLength)?,
		)))
	}

	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.to_bytes().to_vec()
	}

	#[must_use]
	pub fn to_remote_identity(&self) -> RemoteIdentity {
		RemoteIdentity(self.0.verifying_key())
	}
}

#[derive(Copy, Clone, PartialEq, Eq, Type)]
#[specta(transparent)]
pub struct RemoteIdentity(#[specta(type = String)] ed25519_dalek::VerifyingKey);

impl Hash for RemoteIdentity {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.as_bytes().hash(state);
	}
}

impl std::fmt::Debug for RemoteIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("RemoteIdentity")
			.field(&general_purpose::STANDARD_NO_PAD.encode(self.0.as_bytes()))
			.finish()
	}
}

impl std::fmt::Display for RemoteIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&general_purpose::STANDARD_NO_PAD.encode(self.0.as_bytes()))
	}
}

impl Serialize for RemoteIdentity {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&general_purpose::STANDARD_NO_PAD.encode(self.0.as_bytes()))
	}
}

impl<'de> Deserialize<'de> for RemoteIdentity {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		let bytes = general_purpose::STANDARD_NO_PAD
			.decode(s)
			.map_err(serde::de::Error::custom)?;
		Ok(Self(
			ed25519_dalek::VerifyingKey::from_bytes(
				bytes[..SECRET_KEY_LENGTH]
					.try_into()
					.map_err(|_| serde::de::Error::custom("Invalid key length"))?,
			)
			.map_err(serde::de::Error::custom)?,
		))
	}
}

impl TryFrom<String> for RemoteIdentity {
	type Error = IdentityErr;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		let bytes = general_purpose::STANDARD_NO_PAD
			.decode(value)
			.map_err(|_| IdentityErr::InvalidKeyLength)?;
		Ok(Self(ed25519_dalek::VerifyingKey::from_bytes(
			bytes[..SECRET_KEY_LENGTH]
				.try_into()
				.map_err(|_| IdentityErr::InvalidKeyLength)?,
		)?))
	}
}

impl FromStr for RemoteIdentity {
	type Err = IdentityErr;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let bytes = general_purpose::STANDARD_NO_PAD
			.decode(s)
			.map_err(|_| IdentityErr::InvalidKeyLength)?;
		Ok(Self(ed25519_dalek::VerifyingKey::from_bytes(
			bytes[..SECRET_KEY_LENGTH]
				.try_into()
				.map_err(|_| IdentityErr::InvalidKeyLength)?,
		)?))
	}
}

impl RemoteIdentity {
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityErr> {
		Ok(Self(ed25519_dalek::VerifyingKey::from_bytes(
			bytes[..SECRET_KEY_LENGTH]
				.try_into()
				.map_err(|_| IdentityErr::InvalidKeyLength)?,
		)?))
	}

	#[must_use]
	pub fn get_bytes(&self) -> [u8; REMOTE_IDENTITY_LEN] {
		self.0.to_bytes()
	}

	#[must_use]
	pub fn verifying_key(&self) -> VerifyingKey {
		self.0
	}
}

impl From<ed25519_dalek::SigningKey> for Identity {
	fn from(value: ed25519_dalek::SigningKey) -> Self {
		Self(value)
	}
}
