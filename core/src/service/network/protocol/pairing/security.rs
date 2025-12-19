//! Security utilities for pairing protocol

use crate::service::network::{NetworkingError, Result};
// We'll use our own signature verification

/// Security operations for pairing protocol
pub struct PairingSecurity;

impl PairingSecurity {
	/// Verify a challenge response signature using ed25519 public key
	pub fn verify_challenge_response(
		public_key_bytes: &[u8],
		challenge: &[u8],
		signature: &[u8],
	) -> Result<bool> {
		// Validate inputs first
		Self::validate_public_key(public_key_bytes)?;
		Self::validate_challenge(challenge)?;
		Self::validate_signature(signature)?;

		// Use ed25519-dalek for verification
		use ed25519_dalek::{Signature, Verifier, VerifyingKey};

		// Public key should be 32 bytes for Ed25519
		if public_key_bytes.len() != 32 {
			return Err(NetworkingError::Protocol(
				"Invalid public key length for Ed25519".to_string(),
			));
		}

		let verifying_key = VerifyingKey::from_bytes(public_key_bytes.try_into().unwrap())
			.map_err(|e| NetworkingError::Protocol(format!("Invalid public key: {}", e)))?;

		let sig = Signature::from_slice(signature)
			.map_err(|e| NetworkingError::Protocol(format!("Invalid signature: {}", e)))?;

		Ok(verifying_key.verify(challenge, &sig).is_ok())
	}

	/// Validate device public key format (Ed25519 raw bytes)
	pub fn validate_public_key(public_key_bytes: &[u8]) -> Result<()> {
		// Ed25519 public keys are exactly 32 bytes
		if public_key_bytes.len() != 32 {
			return Err(NetworkingError::Protocol(format!(
				"Invalid public key length: expected 32 bytes, got {}",
				public_key_bytes.len()
			)));
		}

		// Basic validation - ensure it's not all zeros
		if public_key_bytes.iter().all(|&b| b == 0) {
			return Err(NetworkingError::Protocol(
				"Invalid public key: all zeros".to_string(),
			));
		}

		Ok(())
	}

	/// Validate challenge format and size
	pub fn validate_challenge(challenge: &[u8]) -> Result<()> {
		if challenge.len() != 32 {
			return Err(NetworkingError::Protocol(format!(
				"Invalid challenge length: expected 32 bytes, got {}",
				challenge.len()
			)));
		}

		// Ensure challenge isn't all zeros (weak challenge)
		if challenge.iter().all(|&b| b == 0) {
			return Err(NetworkingError::Protocol(
				"Invalid challenge: all zeros".to_string(),
			));
		}

		Ok(())
	}

	/// Validate signature format
	pub fn validate_signature(signature: &[u8]) -> Result<()> {
		if signature.len() != 64 {
			return Err(NetworkingError::Protocol(format!(
				"Invalid signature length: expected 64 bytes, got {}",
				signature.len()
			)));
		}

		// Basic validation - ensure it's not all zeros
		if signature.iter().all(|&b| b == 0) {
			return Err(NetworkingError::Protocol(
				"Invalid signature: all zeros".to_string(),
			));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ed25519_dalek::{ed25519::signature::Keypair, SigningKey};
	use rand::rngs::OsRng;

	#[test]
	fn test_validate_public_key() {
		// Create a real ed25519 keypair for testing
		let signing_key = SigningKey::from_bytes(&[1u8; 32]);
		let public_key_bytes = signing_key.verifying_key().to_bytes();

		// Valid key
		assert!(PairingSecurity::validate_public_key(&public_key_bytes).is_ok());

		// Invalid length (too short)
		assert!(PairingSecurity::validate_public_key(&[0u8; 20]).is_err());

		// Invalid length (too long)
		assert!(PairingSecurity::validate_public_key(&[0u8; 40]).is_err());

		// All zeros (invalid)
		let invalid_key = [0u8; 32];
		assert!(PairingSecurity::validate_public_key(&invalid_key).is_err());
	}

	#[test]
	fn test_validate_challenge() {
		// Valid challenge
		let valid_challenge = [1u8; 32];
		assert!(PairingSecurity::validate_challenge(&valid_challenge).is_ok());

		// Invalid length
		assert!(PairingSecurity::validate_challenge(&[1u8; 31]).is_err());
		assert!(PairingSecurity::validate_challenge(&[1u8; 33]).is_err());

		// All zeros (weak)
		let weak_challenge = [0u8; 32];
		assert!(PairingSecurity::validate_challenge(&weak_challenge).is_err());
	}

	#[test]
	fn test_validate_signature() {
		// Valid signature
		let valid_signature = [1u8; 64];
		assert!(PairingSecurity::validate_signature(&valid_signature).is_ok());

		// Invalid length
		assert!(PairingSecurity::validate_signature(&[1u8; 63]).is_err());
		assert!(PairingSecurity::validate_signature(&[1u8; 65]).is_err());

		// All zeros (invalid)
		let invalid_signature = [0u8; 64];
		assert!(PairingSecurity::validate_signature(&invalid_signature).is_err());
	}

	#[test]
	fn test_verify_challenge_response() {
		use ed25519_dalek::Signer;

		// Create a real keypair and sign a challenge
		let signing_key = SigningKey::from_bytes(&[3u8; 32]);
		let public_key_bytes = signing_key.verifying_key().to_bytes();
		let challenge = [2u8; 32];
		let signature = signing_key.sign(&challenge);

		// Should verify successfully with REAL cryptographic verification
		let result = PairingSecurity::verify_challenge_response(
			&public_key_bytes,
			&challenge,
			&signature.to_bytes(),
		);
		assert!(result.is_ok());
		assert!(result.unwrap());
	}

	#[test]
	fn test_verify_challenge_response_invalid_signature() {
		use ed25519_dalek::Signer;

		// Create a keypair and sign wrong data
		let signing_key = SigningKey::from_bytes(&[4u8; 32]);
		let public_key_bytes = signing_key.verifying_key().to_bytes();
		let challenge = [2u8; 32];
		let wrong_challenge = [3u8; 32];
		let signature = signing_key.sign(&wrong_challenge);

		// Should fail verification (this proves crypto is REALLY working!)
		let result = PairingSecurity::verify_challenge_response(
			&public_key_bytes,
			&challenge,
			&signature.to_bytes(),
		);
		assert!(result.is_ok());
		assert!(!result.unwrap()); // Should be false
	}
}
