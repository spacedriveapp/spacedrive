use std::hash::{Hash, Hasher};

use ed25519_dalek::PublicKey;
use rand_core::OsRng;
use serde::Serialize;
use specta::Type;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct IdentityErr(#[from] ed25519_dalek::ed25519::Error);

/// TODO
#[derive(Debug)]
pub struct Identity(ed25519_dalek::Keypair);

impl PartialEq for Identity {
	fn eq(&self, other: &Self) -> bool {
		self.0.public.eq(&other.0.public)
	}
}

impl Default for Identity {
	fn default() -> Self {
		Self(ed25519_dalek::Keypair::generate(&mut OsRng))
	}
}

impl Identity {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityErr> {
		Ok(Self(ed25519_dalek::Keypair::from_bytes(bytes)?))
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.to_bytes().to_vec()
	}

	pub fn to_remote_identity(&self) -> RemoteIdentity {
		RemoteIdentity(self.0.public)
	}
}
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteIdentity(ed25519_dalek::PublicKey);

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

impl Type for RemoteIdentity {
	fn inline(
		_: specta::DefOpts,
		_: &[specta::DataType],
	) -> Result<specta::DataType, specta::ExportError> {
		Ok(specta::DataType::Primitive(specta::PrimitiveType::String))
	}
}

impl RemoteIdentity {
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityErr> {
		Ok(Self(ed25519_dalek::PublicKey::from_bytes(bytes)?))
	}

	pub fn to_bytes(&self) -> [u8; 32] {
		self.0.to_bytes()
	}

	pub fn public_key(&self) -> PublicKey {
		self.0
	}
}
