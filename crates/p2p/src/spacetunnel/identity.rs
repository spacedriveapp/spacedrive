use ed25519_dalek::PublicKey;
use rand_core::OsRng;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct IdentityErr(#[from] ed25519_dalek::ed25519::Error);

/// TODO
pub struct Identity(ed25519_dalek::Keypair);

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

	pub fn public_key(&self) -> PublicKey {
		self.0.public
	}

	pub fn to_remote_identity(&self) -> RemoteIdentity {
		RemoteIdentity(self.0.public)
	}
}
#[derive(Debug, PartialEq, Eq)]
pub struct RemoteIdentity(ed25519_dalek::PublicKey);

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
