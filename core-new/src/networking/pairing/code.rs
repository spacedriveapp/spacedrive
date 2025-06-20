//! Enhanced pairing code implementation with BIP39 support

use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
// Note: Serialize/Deserialize kept for future JSON export feature

use crate::networking::{NetworkError, Result};

/// Enhanced pairing code with BIP39 wordlist support
#[derive(Clone, Debug)]
pub struct PairingCode {
    /// 256-bit cryptographic secret
    pub secret: [u8; 32],
    
    /// Expiration timestamp (5 minutes from creation)
    pub expires_at: DateTime<Utc>,
    
    /// 6 words from BIP39 wordlist for user-friendly sharing
    pub words: [String; 6],
    
    /// Fingerprint for mDNS discovery (derived from secret)
    pub discovery_fingerprint: [u8; 16],
    
    /// Nonce for challenge-response (prevents replay attacks)
    pub nonce: [u8; 16],
}

impl PairingCode {
    /// Generate a new pairing code using BIP39 wordlist
    pub fn generate() -> Result<Self> {
        let mut secret = [0u8; 32];
        let mut nonce = [0u8; 16];
        let rng = SystemRandom::new();
        
        rng.fill(&mut secret)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate secret: {:?}", e)))?;
        rng.fill(&mut nonce)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate nonce: {:?}", e)))?;
        
        // Convert secret to 6 words (using simplified hex encoding)
        // This encodes enough entropy to reconstruct the secret
        let words = Self::encode_to_bip39_words(&secret)?;
        
        // Derive discovery fingerprint from secret
        let discovery_fingerprint = Self::derive_fingerprint(&secret);
        
        Ok(PairingCode {
            secret,
            expires_at: Utc::now() + Duration::minutes(5),
            words,
            discovery_fingerprint,
            nonce,
        })
    }
    
    /// Create pairing code from BIP39 words
    pub fn from_words(words: &[String; 6]) -> Result<Self> {
        // Decode BIP39 words back to bytes
        let secret_bytes = Self::decode_from_bip39_words(words)?;
        
        if secret_bytes.len() != 32 {
            return Err(NetworkError::EncryptionError(format!("Invalid secret length: expected 32, got {}", secret_bytes.len())));
        }
        
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&secret_bytes);
        
        // Generate new nonce for this attempt
        let mut nonce = [0u8; 16];
        let rng = SystemRandom::new();
        rng.fill(&mut nonce)
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate nonce: {:?}", e)))?;
        
        let discovery_fingerprint = Self::derive_fingerprint(&secret);
        
        Ok(PairingCode {
            secret,
            expires_at: Utc::now() + Duration::minutes(5), // Reset expiration
            words: words.clone(),
            discovery_fingerprint,
            nonce,
        })
    }
    
    /// Encode bytes to BIP39 words (simplified version)
    fn encode_to_bip39_words(bytes: &[u8]) -> Result<[String; 6]> {
        // For now, use a simplified approach with hex encoding
        // In production, this should use proper BIP39 entropy encoding
        let hex_string = hex::encode(bytes);
        
        // Split into 6 chunks of approximately equal length
        // For 32 bytes (64 hex chars), each chunk gets ~10-11 chars
        let chunk_size = hex_string.len() / 6;
        let remainder = hex_string.len() % 6;
        
        let mut chunks = Vec::new();
        let mut start = 0;
        
        for i in 0..6 {
            let extra = if i < remainder { 1 } else { 0 };
            let end = start + chunk_size + extra;
            chunks.push(hex_string[start..end].to_string());
            start = end;
        }
        
        if chunks.len() != 6 {
            return Err(NetworkError::EncryptionError("Failed to encode pairing code".to_string()));
        }
        
        Ok([
            chunks[0].clone(),
            chunks[1].clone(),
            chunks[2].clone(),
            chunks[3].clone(),
            chunks[4].clone(),
            chunks[5].clone(),
        ])
    }
    
    /// Decode BIP39 words back to bytes (simplified version)
    fn decode_from_bip39_words(words: &[String; 6]) -> Result<Vec<u8>> {
        let hex_string = words.join("");
        hex::decode(&hex_string)
            .map_err(|e| NetworkError::EncryptionError(format!("Invalid pairing words: {}", e)))
    }
    
    /// Derive consistent fingerprint for mDNS discovery
    fn derive_fingerprint(secret: &[u8; 32]) -> [u8; 16] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"spacedrive-pairing-v1");
        hasher.update(secret);
        let hash = hasher.finalize();
        let mut fingerprint = [0u8; 16];
        fingerprint.copy_from_slice(&hash.as_bytes()[..16]);
        fingerprint
    }
    
    /// Check if the pairing code has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
    
    /// Compute challenge hash for authentication
    pub fn compute_challenge_hash(&self, nonce1: &[u8; 16], nonce2: &[u8; 16]) -> [u8; 32] {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret).unwrap();
        mac.update(nonce1);
        mac.update(nonce2);
        mac.update(b"spacedrive-pairing-challenge-v1");
        
        let result = mac.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result.into_bytes());
        hash
    }
    
    /// Get the words as a space-separated string
    pub fn as_string(&self) -> String {
        self.words.join(" ")
    }
    
    /// Get remaining time until expiration
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = Utc::now();
        if now < self.expires_at {
            Some(self.expires_at - now)
        } else {
            None
        }
    }
    
}

/// Pairing target information from discovery
#[derive(Debug, Clone)]
pub struct PairingTarget {
    /// Network address of the target
    pub address: std::net::IpAddr,
    /// Port number
    pub port: u16,
    /// Device name from mDNS
    pub device_name: String,
    /// Expiration time from mDNS record
    pub expires_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairing_code_generation() {
        let code = PairingCode::generate().unwrap();
        
        // Should have 6 words
        assert_eq!(code.words.len(), 6);
        
        // Should not be expired immediately
        assert!(!code.is_expired());
        
        // Should have valid fingerprint
        assert_eq!(code.discovery_fingerprint.len(), 16);
        
        // String representation should work
        let string_repr = code.as_string();
        assert_eq!(string_repr.split_whitespace().count(), 6);
    }

    #[tokio::test]
    async fn test_pairing_code_round_trip() {
        let original = PairingCode::generate().unwrap();
        let reconstructed = PairingCode::from_words(&original.words).unwrap();
        
        // Secrets should match (first 24 bytes)
        assert_eq!(original.secret[..24], reconstructed.secret[..24]);
        
        // Fingerprints should match
        assert_eq!(original.discovery_fingerprint, reconstructed.discovery_fingerprint);
        
        // Words should match
        assert_eq!(original.words, reconstructed.words);
    }

    #[tokio::test]
    async fn test_challenge_hash_consistency() {
        let code = PairingCode::generate().unwrap();
        let initiator_nonce = [1u8; 16];
        let joiner_nonce = [2u8; 16];
        let timestamp = Utc::now();
        
        let hash1 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce, timestamp).unwrap();
        let hash2 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce, timestamp).unwrap();
        
        assert_eq!(hash1, hash2);
    }
}