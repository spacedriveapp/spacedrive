//! Security utilities for pairing protocol

use crate::services::networking::{NetworkingError, Result};
use libp2p::identity::PublicKey;

/// Security operations for pairing protocol
pub struct PairingSecurity;

impl PairingSecurity {
    /// Verify a challenge response signature using libp2p public key
    pub fn verify_challenge_response(
        public_key_bytes: &[u8],
        challenge: &[u8],
        signature: &[u8],
    ) -> Result<bool> {
        // Validate inputs first
        Self::validate_public_key(public_key_bytes)?;
        Self::validate_challenge(challenge)?;
        Self::validate_signature(signature)?;

        // Parse the public key from protobuf encoding (as used by libp2p)
        let public_key = PublicKey::try_decode_protobuf(public_key_bytes)
            .map_err(|e| NetworkingError::Protocol(format!("Failed to decode public key: {}", e)))?;

        // Verify the signature
        Ok(public_key.verify(challenge, signature))
    }

    /// Validate device public key format (protobuf-encoded libp2p key)
    pub fn validate_public_key(public_key_bytes: &[u8]) -> Result<()> {
        // Libp2p protobuf keys are variable length, typically 32-44 bytes for Ed25519
        if public_key_bytes.len() < 32 || public_key_bytes.len() > 100 {
            return Err(NetworkingError::Protocol(format!(
                "Invalid public key length: expected 32-100 bytes, got {}",
                public_key_bytes.len()
            )));
        }

        // Basic validation - ensure it's not all zeros
        if public_key_bytes.iter().all(|&b| b == 0) {
            return Err(NetworkingError::Protocol(
                "Invalid public key: all zeros".to_string(),
            ));
        }

        // Try to decode it to ensure it's valid
        PublicKey::try_decode_protobuf(public_key_bytes)
            .map_err(|e| NetworkingError::Protocol(format!("Invalid protobuf public key: {}", e)))?;

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
    use libp2p::identity::Keypair;

    #[test]
    fn test_validate_public_key() {
        // Create a real libp2p keypair for testing
        let keypair = Keypair::generate_ed25519();
        let public_key_bytes = keypair.public().encode_protobuf();

        // Valid key
        assert!(PairingSecurity::validate_public_key(&public_key_bytes).is_ok());

        // Invalid length (too short)
        assert!(PairingSecurity::validate_public_key(&[0u8; 20]).is_err());

        // Invalid length (too long)
        assert!(PairingSecurity::validate_public_key(&[0u8; 200]).is_err());

        // All zeros (invalid)
        let invalid_key = vec![0u8; 40];
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
        // Create a real keypair and sign a challenge
        let keypair = Keypair::generate_ed25519();
        let public_key_bytes = keypair.public().encode_protobuf();
        let challenge = [2u8; 32];
        let signature = keypair.sign(&challenge).expect("Failed to sign challenge");

        println!("🔐 Testing REAL Ed25519 signature verification:");
        println!("   Public key: {} bytes", public_key_bytes.len());
        println!("   Challenge: {} bytes", challenge.len());
        println!("   Signature: {} bytes", signature.len());

        // Should verify successfully with REAL cryptographic verification
        let result = PairingSecurity::verify_challenge_response(&public_key_bytes, &challenge, &signature);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        println!("✅ REAL cryptographic signature verification PASSED!");
    }

    #[test]
    fn test_verify_challenge_response_invalid_signature() {
        // Create a keypair and sign wrong data
        let keypair = Keypair::generate_ed25519();
        let public_key_bytes = keypair.public().encode_protobuf();
        let challenge = [2u8; 32];
        let wrong_challenge = [3u8; 32];
        let signature = keypair.sign(&wrong_challenge).expect("Failed to sign challenge");

        println!("🔒 Testing REAL Ed25519 signature rejection:");
        println!("   Signed data: {:?}", &wrong_challenge[..4]);
        println!("   Verify data: {:?}", &challenge[..4]);

        // Should fail verification (this proves crypto is REALLY working!)
        let result = PairingSecurity::verify_challenge_response(&public_key_bytes, &challenge, &signature);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should be false
        
        println!("✅ REAL cryptographic signature rejection PASSED!");
        println!("   🎯 This proves we're doing REAL crypto verification!");
    }

    #[test]
    fn test_different_signatures_for_different_data() {
        let keypair = Keypair::generate_ed25519();
        let mut data1 = [1u8; 32]; // 32-byte challenge as expected
        let mut data2 = [1u8; 32]; // Same base, but we'll change one byte
        data1[31] = 0xAA; // Last byte different
        data2[31] = 0xBB; // Last byte different
        
        let sig1 = keypair.sign(&data1).expect("Failed to sign data1");
        let sig2 = keypair.sign(&data2).expect("Failed to sign data2");
        
        println!("🔐 Demonstrating REAL Ed25519 cryptographic uniqueness:");
        println!("   Data1 last 4 bytes: {:02x?}", &data1[28..]);
        println!("   Data2 last 4 bytes: {:02x?}", &data2[28..]);
        println!("   Sig1 first 8 bytes: {:02x?}", &sig1[..8]);
        println!("   Sig2 first 8 bytes: {:02x?}", &sig2[..8]);
        
        // Signatures should be completely different
        assert_ne!(sig1, sig2);
        println!("✅ Different data produces different signatures!");
        
        // Each signature should only verify its own data
        let public_key_bytes = keypair.public().encode_protobuf();
        let verify1_with_data1 = PairingSecurity::verify_challenge_response(&public_key_bytes, &data1, &sig1).unwrap();
        let verify1_with_data2 = PairingSecurity::verify_challenge_response(&public_key_bytes, &data2, &sig1).unwrap();
        
        assert!(verify1_with_data1);  // Should pass
        assert!(!verify1_with_data2); // Should fail
        
        println!("✅ Signature verification is cryptographically secure!");
        println!("   🎯 One byte difference = complete verification failure!");
    }
}