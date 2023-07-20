use std::str::FromStr;

use ed25519_dalek::PublicKey;
use rand_core::OsRng;
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

	pub fn public_key(&self) -> PublicKey {
		self.0.public
	}

	pub fn to_remote_identity(&self) -> RemoteIdentity {
		RemoteIdentity(self.0.public)
	}
}
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl FromStr for RemoteIdentity {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let bytes = hex::decode(s).map_err(|e| e.to_string())?;
		Ok(Self(
			ed25519_dalek::PublicKey::from_bytes(&bytes).map_err(|e| e.to_string())?,
		))
	}
}

impl ToString for RemoteIdentity {
	fn to_string(&self) -> String {
		hex::encode(self.to_bytes())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_identity() {
		let pair = Identity::new();

		let pair2 = Identity::from_bytes(&pair.to_bytes()).unwrap();
		assert_eq!(pair, pair2);

		let pk = pair.to_remote_identity();
		let pk2 = RemoteIdentity::from_bytes(&pk.to_bytes()).unwrap();
		assert_eq!(pk, pk2);

		let pk3 = pk.to_string().parse::<RemoteIdentity>().unwrap();
		assert_eq!(pk, pk3);
	}
}
