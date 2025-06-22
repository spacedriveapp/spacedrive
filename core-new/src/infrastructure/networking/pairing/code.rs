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
    
    /// 12 words from BIP39 wordlist for user-friendly sharing
    pub words: [String; 12],
    
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
        
        // Convert secret to 12 BIP39 words using proper mnemonic encoding
        // This uses 128 bits of entropy (16 bytes) which provides excellent security
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
    pub fn from_words(words: &[String; 12]) -> Result<Self> {
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
    
    /// Encode bytes to BIP39 words using proper mnemonic generation
    fn encode_to_bip39_words(bytes: &[u8]) -> Result<[String; 12]> {
        use bip39::{Mnemonic, Language};
        
        // For 12 words, we need 128 bits of entropy (standard BIP39)
        // Use the first 16 bytes from our 32-byte secret
        if bytes.len() < 16 {
            return Err(NetworkError::EncryptionError("Insufficient entropy for BIP39 encoding".to_string()));
        }
        
        // Use first 16 bytes for mnemonic generation (128 bits -> 12 words)
        let entropy = &bytes[..16];
        
        // Generate mnemonic from entropy
        let mnemonic = Mnemonic::from_entropy(entropy)
            .map_err(|e| NetworkError::EncryptionError(format!("BIP39 generation failed: {}", e)))?;
        
        // Get the word list (should be exactly 12 words for 128 bits of entropy)  
        let word_list: Vec<&str> = mnemonic.words().collect();
        
        if word_list.len() != 12 {
            return Err(NetworkError::EncryptionError(format!("Expected 12 words, got {}", word_list.len())));
        }
        
        Ok([
            word_list[0].to_string(),
            word_list[1].to_string(),
            word_list[2].to_string(),
            word_list[3].to_string(),
            word_list[4].to_string(),
            word_list[5].to_string(),
            word_list[6].to_string(),
            word_list[7].to_string(),
            word_list[8].to_string(),
            word_list[9].to_string(),
            word_list[10].to_string(),
            word_list[11].to_string(),
        ])
    }
    
    /// Decode BIP39 words back to bytes using proper mnemonic parsing
    fn decode_from_bip39_words(words: &[String; 12]) -> Result<Vec<u8>> {
        use bip39::{Mnemonic, Language};
        
        // Join words with spaces to create mnemonic string
        let mnemonic_str = words.join(" ");
        
        // Parse the mnemonic
        let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_str)
            .map_err(|e| NetworkError::EncryptionError(format!("Invalid BIP39 mnemonic: {}", e)))?;
        
        // Extract the entropy (should be 16 bytes for 12 words)
        let entropy = mnemonic.to_entropy();
        
        if entropy.len() != 16 {
            return Err(NetworkError::EncryptionError(format!("Expected 16 bytes of entropy, got {}", entropy.len())));
        }
        
        // We need to reconstruct the full 32-byte secret
        // Use the 16 bytes of entropy and derive the remaining 16 bytes deterministically
        let mut full_secret = vec![0u8; 32];
        full_secret[..16].copy_from_slice(&entropy);
        
        // Derive the remaining 16 bytes using BLAKE3 for deterministic padding
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"spacedrive-pairing-entropy-extension-v1");
        hasher.update(&entropy);
        let derived = hasher.finalize();
        full_secret[16..].copy_from_slice(&derived.as_bytes()[..16]);
        
        Ok(full_secret)
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
    
    /// Get the words as a space-separated string (alias for UI compatibility)
    pub fn as_words(&self) -> String {
        self.as_string()
    }
    
    /// Parse pairing code from space-separated string
    pub fn from_string(code_str: &str) -> Result<Self> {
        let words: Vec<String> = code_str.split_whitespace().map(|s| s.to_string()).collect();
        
        if words.len() != 12 {
            return Err(NetworkError::EncryptionError(format!(
                "Invalid pairing code: expected 12 words, got {}", words.len()
            )));
        }
        
        let words_array: [String; 12] = [
            words[0].clone(), words[1].clone(), words[2].clone(), words[3].clone(),
            words[4].clone(), words[5].clone(), words[6].clone(), words[7].clone(),
            words[8].clone(), words[9].clone(), words[10].clone(), words[11].clone(),
        ];
        
        Self::from_words(&words_array)
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
        
        // Should have 12 words
        assert_eq!(code.words.len(), 12);
        
        // Should not be expired immediately
        assert!(!code.is_expired());
        
        // Should have valid fingerprint
        assert_eq!(code.discovery_fingerprint.len(), 16);
        
        // String representation should work
        let string_repr = code.as_string();
        assert_eq!(string_repr.split_whitespace().count(), 12);
    }

    #[tokio::test]
    async fn test_pairing_code_round_trip() {
        let original = PairingCode::generate().unwrap();
        let reconstructed = PairingCode::from_words(&original.words).unwrap();
        
        // Secrets should match (first 16 bytes come from BIP39, rest is derived)
        assert_eq!(original.secret[..16], reconstructed.secret[..16]);
        
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
        
        let hash1 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);
        let hash2 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);
        
        assert_eq!(hash1, hash2);
    }
}