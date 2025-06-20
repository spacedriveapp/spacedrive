//! Integration tests for pairing module

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::networking::identity::{PublicKey, DeviceInfo};
    use uuid::Uuid;

    fn create_test_device_info() -> DeviceInfo {
        DeviceInfo::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
        )
    }

    #[tokio::test]
    async fn test_pairing_code_generation() {
        let code = PairingCode::generate().unwrap();
        
        assert_eq!(code.words.len(), 6);
        assert!(!code.is_expired());
        assert_eq!(code.discovery_fingerprint.len(), 16);
        
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
    async fn test_discovery_creation() {
        let device_info = create_test_device_info();
        let discovery = PairingDiscovery::new(device_info);
        
        assert!(!discovery.is_broadcasting());
        assert!(discovery.current_code().is_none());
    }

    #[test]
    fn test_pairing_message_serialization() {
        use chrono::Utc;
        
        let message = PairingMessage::Challenge {
            initiator_nonce: [1u8; 16],
            timestamp: Utc::now(),
        };
        
        let serialized = PairingProtocolHandler::serialize_message(&message).unwrap();
        let deserialized = PairingProtocolHandler::deserialize_message(&serialized).unwrap();
        
        match (message, deserialized) {
            (PairingMessage::Challenge { initiator_nonce: n1, .. }, 
             PairingMessage::Challenge { initiator_nonce: n2, .. }) => {
                assert_eq!(n1, n2);
            }
            _ => panic!("Message types don't match"),
        }
    }

    #[test]
    fn test_session_keys_derivation() {
        let shared_secret = [42u8; 32];
        let device1 = Uuid::new_v4();
        let device2 = Uuid::new_v4();
        
        let keys1 = SessionKeys::derive_from_shared_secret(&shared_secret, &device1, &device2).unwrap();
        let keys2 = SessionKeys::derive_from_shared_secret(&shared_secret, &device1, &device2).unwrap();
        
        // Same inputs should produce same keys
        assert_eq!(keys1.send_key, keys2.send_key);
        assert_eq!(keys1.receive_key, keys2.receive_key);
        assert_eq!(keys1.mac_key, keys2.mac_key);
        assert_eq!(keys1.initial_iv, keys2.initial_iv);
    }

    #[test]
    fn test_challenge_hash_consistency() {
        let code = PairingCode::generate().unwrap();
        let initiator_nonce = [1u8; 16];
        let joiner_nonce = [2u8; 16];
        let timestamp = chrono::Utc::now();
        
        let hash1 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce, timestamp).unwrap();
        let hash2 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce, timestamp).unwrap();
        
        assert_eq!(hash1, hash2);
    }
}