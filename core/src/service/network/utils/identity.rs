//! Network identity management - node ID and key generation

use crate::service::networking::{NetworkingError, Result};
use iroh::net::key::{NodeId, SecretKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Network identity containing keypair and node ID
#[derive(Clone)]
pub struct NetworkIdentity {
	secret_key: SecretKey,
	node_id: NodeId,
	// Keep Ed25519 keypair for backward compatibility
	ed25519_seed: [u8; 32],
}

impl NetworkIdentity {
	/// Create a new random network identity
	pub async fn new() -> Result<Self> {
		let secret_key = SecretKey::generate();
		let node_id = secret_key.public();

		// Generate Ed25519 seed for backward compatibility
		let ed25519_seed = rand::random();

		Ok(Self {
			secret_key,
			node_id,
			ed25519_seed,
		})
	}

	/// Create a deterministic network identity from device key
	pub async fn from_device_key(device_key: &[u8; 32]) -> Result<Self> {
		// Derive Ed25519 seed from master key using HKDF
		use hkdf::Hkdf;
		use sha2::Sha256;

		let hk = Hkdf::<Sha256>::new(None, device_key);
		let mut ed25519_seed = [0u8; 32];
		hk.expand(b"spacedrive-network-identity", &mut ed25519_seed)
			.map_err(|e| NetworkingError::Protocol(format!("Failed to derive network key: {}", e)))?;

		// Create Iroh secret key from the same seed
		let secret_key = SecretKey::from_bytes(&ed25519_seed);
		let node_id = secret_key.public();

		Ok(Self {
			secret_key,
			node_id,
			ed25519_seed,
		})
	}

	/// Convert to Iroh SecretKey
	pub fn to_iroh_secret_key(&self) -> Result<SecretKey> {
		Ok(self.secret_key.clone())
	}

	/// Get the node ID
	pub fn node_id(&self) -> NodeId {
		self.node_id
	}

	/// Get the public key bytes (for backward compatibility)
	pub fn public_key_bytes(&self) -> Vec<u8> {
		self.node_id.as_bytes().to_vec()
	}

	/// Get the keypair bytes (for backward compatibility)
	pub fn keypair_bytes(&self) -> &[u8; 32] {
		&self.ed25519_seed
	}

	/// Sign data with this identity
	pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
		// Use Ed25519 signing for backward compatibility
		use ed25519_dalek::{Signer, SigningKey};

		let signing_key = SigningKey::from_bytes(&self.ed25519_seed);
		let signature = signing_key.sign(data);
		Ok(signature.to_bytes().to_vec())
	}

	/// Verify signature with this identity's public key
	pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
		// Use Ed25519 verification for backward compatibility
		use ed25519_dalek::{Signature, Verifier, VerifyingKey, SigningKey};

		let signing_key = SigningKey::from_bytes(&self.ed25519_seed);
		let verifying_key = signing_key.verifying_key();

		if let Ok(sig) = Signature::from_slice(signature) {
			verifying_key.verify(data, &sig).is_ok()
		} else {
			false
		}
	}

	/// Get a deterministic device ID from the network identity
	pub fn device_id(&self) -> Uuid {
		// Create a deterministic UUID from the node ID
		let node_id_bytes = self.node_id.as_bytes();

		// Use the first 16 bytes of the node ID hash to create a UUID
		let mut uuid_bytes = [0u8; 16];
		let hash = blake3::hash(node_id_bytes);
		uuid_bytes.copy_from_slice(&hash.as_bytes()[..16]);

		Uuid::from_bytes(uuid_bytes)
	}

	/// Get network fingerprint for device identification
	pub fn network_fingerprint(&self) -> NetworkFingerprint {
		let public_key_bytes = self.public_key_bytes();
		let public_key_hash = blake3::hash(&public_key_bytes);

		NetworkFingerprint {
			node_id: self.node_id.to_string(),
			public_key_hash: hex::encode(&public_key_hash.as_bytes()[..16]),
		}
	}
}

impl std::fmt::Debug for NetworkIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NetworkIdentity")
			.field("node_id", &self.node_id)
			.finish()
	}
}

/// Serializable network fingerprint for device identification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NetworkFingerprint {
	pub node_id: String,
	pub public_key_hash: String,
}

impl std::fmt::Display for NetworkFingerprint {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}:{}", &self.node_id[..8], &self.public_key_hash[..8])
	}
}

impl NetworkFingerprint {
	/// Create fingerprint from network identity
	pub fn from_identity(identity: &NetworkIdentity) -> Self {
		let public_key_bytes = identity.public_key_bytes();
		let public_key_hash = blake3::hash(&public_key_bytes).to_hex().to_string();

		Self {
			node_id: identity.node_id().to_string(),
			public_key_hash,
		}
	}

	/// Verify that this fingerprint matches the given identity
	pub fn verify(&self, identity: &NetworkIdentity) -> bool {
		let expected = Self::from_identity(identity);
		*self == expected
	}
}