//! Network identity management - node ID and key generation

use crate::service::network::{NetworkingError, Result};
use iroh::{NodeId, SecretKey};
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
		let secret_key = SecretKey::generate(&mut rand::thread_rng());
		let node_id = secret_key.public();

		// Generate Ed25519 seed for backward compatibility
		let ed25519_seed = rand::random();

		Ok(Self {
			secret_key,
			node_id,
			ed25519_seed,
		})
	}

	/// Create a deterministic network identity from device UUID
	///
	/// This ensures network identity is tied to the canonical device identity
	/// and remains stable as long as `device_id` doesn't change.
	pub async fn from_device_id(device_id: Uuid) -> Result<Self> {
		use hkdf::Hkdf;
		use sha2::Sha256;

		// Use device_id bytes (16 bytes) as IKM for HKDF
		let device_id_bytes = device_id.as_bytes();

		// Derive a 32-byte Ed25519 seed from the 16-byte device_id
		let hk = Hkdf::<Sha256>::new(None, device_id_bytes);
		let mut ed25519_seed = [0u8; 32];
		hk.expand(b"spacedrive-network-identity-v1", &mut ed25519_seed)
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to derive network key: {}", e))
			})?;

		// Create Iroh secret key from the derived seed
		let secret_key = SecretKey::from_bytes(&ed25519_seed);
		let node_id = secret_key.public();

		Ok(Self {
			secret_key,
			node_id,
			ed25519_seed,
		})
	}

	// OLD: Only kept for reference, not used
	// Can be deleted in future cleanup
	#[allow(dead_code)]
	async fn from_device_key_old(device_key: &[u8; 32]) -> Result<Self> {
		use hkdf::Hkdf;
		use sha2::Sha256;

		let hk = Hkdf::<Sha256>::new(None, device_key);
		let mut ed25519_seed = [0u8; 32];
		hk.expand(b"spacedrive-network-identity", &mut ed25519_seed)
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to derive network key: {}", e))
			})?;

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
		use ed25519_dalek::{Signature, SigningKey, Verifier, VerifyingKey};

		let signing_key = SigningKey::from_bytes(&self.ed25519_seed);
		let verifying_key = signing_key.verifying_key();

		if let Ok(sig) = Signature::from_slice(signature) {
			verifying_key.verify(data, &sig).is_ok()
		} else {
			false
		}
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

#[cfg(test)]
mod tests {
	use super::*;
	use uuid::Uuid;

	#[tokio::test]
	async fn test_device_id_derivation_is_deterministic() {
		let device_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

		let identity1 = NetworkIdentity::from_device_id(device_id).await.unwrap();
		let identity2 = NetworkIdentity::from_device_id(device_id).await.unwrap();

		assert_eq!(identity1.node_id, identity2.node_id);
	}

	#[tokio::test]
	async fn test_different_device_ids_produce_different_node_ids() {
		let device_id1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
		let device_id2 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();

		let identity1 = NetworkIdentity::from_device_id(device_id1).await.unwrap();
		let identity2 = NetworkIdentity::from_device_id(device_id2).await.unwrap();

		assert_ne!(identity1.node_id, identity2.node_id);
	}

	#[tokio::test]
	async fn test_node_id_is_valid_ed25519_key() {
		let device_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
		let identity = NetworkIdentity::from_device_id(device_id).await.unwrap();

		assert_eq!(identity.node_id.as_bytes().len(), 32);
	}
}
