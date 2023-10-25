use std::hash::{Hash, Hasher};

use ed25519_dalek::{VerifyingKey, SECRET_KEY_LENGTH};
use rand_core::OsRng;
use serde::Serialize;
use specta::Type;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum IdentityErr {
	#[error("{0}")]
	Darlek(#[from] ed25519_dalek::ed25519::Error),
	#[error("Invalid key length")]
	InvalidKeyLength,
}
/// TODO
#[derive(Debug)]
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

	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.to_bytes().to_vec()
	}

	pub fn to_remote_identity(&self) -> RemoteIdentity {
		RemoteIdentity(self.0.verifying_key())
	}
}
#[derive(Clone, PartialEq, Eq, Type)]
pub struct RemoteIdentity(#[specta(type = String)] ed25519_dalek::VerifyingKey);

impl Hash for RemoteIdentity {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.as_bytes().hash(state);
	}
}

impl std::fmt::Debug for RemoteIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("RemoteIdentity")
			.field(&hex::encode(self.0.as_bytes()))
			.finish()
	}
}

impl Serialize for RemoteIdentity {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&hex::encode(self.0.as_bytes()))
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

	pub fn to_bytes(&self) -> [u8; 32] {
		self.0.to_bytes()
	}

	pub fn verifying_key(&self) -> VerifyingKey {
		self.0
	}
}
