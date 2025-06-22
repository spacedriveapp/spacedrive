//! Network identity management - peer ID and key generation

use crate::infrastructure::networking::{NetworkingError, Result};
use libp2p::{identity::Keypair, PeerId};
use serde::{Deserialize, Serialize};

/// Network identity containing keypair and peer ID
#[derive(Clone)]
pub struct NetworkIdentity {
	keypair: Keypair,
	peer_id: PeerId,
}

impl NetworkIdentity {
	/// Create a new random network identity
	pub async fn new() -> Result<Self> {
		let keypair = Keypair::generate_ed25519();
		let peer_id = PeerId::from(keypair.public());

		Ok(Self { keypair, peer_id })
	}

	/// Create network identity from existing keypair
	pub fn from_keypair(keypair: Keypair) -> Self {
		let peer_id = PeerId::from(keypair.public());

		Self { keypair, peer_id }
	}

	/// Get the keypair
	pub fn keypair(&self) -> &Keypair {
		&self.keypair
	}

	/// Get the peer ID
	pub fn peer_id(&self) -> PeerId {
		self.peer_id
	}

	/// Get the public key bytes
	pub fn public_key_bytes(&self) -> Vec<u8> {
		self.keypair.public().encode_protobuf()
	}

	/// Sign data with this identity
	pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
		self.keypair
			.sign(data)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to sign data: {}", e)))
	}

	/// Verify signature with this identity's public key
	pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
		self.keypair.public().verify(data, signature)
	}
}

impl std::fmt::Debug for NetworkIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NetworkIdentity")
			.field("peer_id", &self.peer_id)
			.finish()
	}
}

/// Serializable network fingerprint for device identification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NetworkFingerprint {
	pub peer_id: String,
	pub public_key_hash: String,
}

impl std::fmt::Display for NetworkFingerprint {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}:{}", &self.peer_id[..8], &self.public_key_hash[..8])
	}
}

impl NetworkFingerprint {
	/// Create fingerprint from network identity
	pub fn from_identity(identity: &NetworkIdentity) -> Self {
		let public_key_bytes = identity.public_key_bytes();
		let public_key_hash = blake3::hash(&public_key_bytes).to_hex().to_string();

		Self {
			peer_id: identity.peer_id().to_string(),
			public_key_hash,
		}
	}

	/// Verify that this fingerprint matches the given identity
	pub fn verify(&self, identity: &NetworkIdentity) -> bool {
		let expected = Self::from_identity(identity);
		*self == expected
	}
}
