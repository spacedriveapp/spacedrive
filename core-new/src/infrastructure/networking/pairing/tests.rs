//! Integration tests for pairing module

#[cfg(test)]
mod tests {
	use super::super::*;
	use crate::networking::identity::{DeviceInfo, PublicKey};
	use uuid::Uuid;

	fn create_test_device_info() -> DeviceInfo {
		use crate::networking::{DeviceInfo, NetworkFingerprint, PublicKey};
		use chrono::Utc;

		let device_id = Uuid::new_v4();
		let public_key = PublicKey::from_bytes(vec![42u8; 32]).unwrap();

		DeviceInfo {
			device_id,
			device_name: "Test Device".to_string(),
			public_key: public_key.clone(),
			network_fingerprint: NetworkFingerprint::from_device(device_id, &public_key),
			last_seen: Utc::now(),
		}
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
		assert_eq!(
			original.discovery_fingerprint,
			reconstructed.discovery_fingerprint
		);

		// Words should match
		assert_eq!(original.words, reconstructed.words);
	}

	#[tokio::test]
	async fn test_pairing_state_transitions() {
		let initial_state = PairingState::Idle;
		assert_eq!(initial_state, PairingState::Idle);

		let generating_state = PairingState::GeneratingCode;
		assert_ne!(initial_state, generating_state);
	}

	#[test]
	fn test_pairing_message_serialization() {
		use chrono::Utc;

		let message = PairingMessage::Challenge {
			initiator_nonce: [1u8; 16],
			timestamp: Utc::now(),
		};

		// Test that we can serialize to JSON
		let serialized = serde_json::to_string(&message).unwrap();
		let deserialized: PairingMessage = serde_json::from_str(&serialized).unwrap();

		match (message, deserialized) {
			(
				PairingMessage::Challenge {
					initiator_nonce: n1,
					..
				},
				PairingMessage::Challenge {
					initiator_nonce: n2,
					..
				},
			) => {
				assert_eq!(n1, n2);
			}
			_ => panic!("Message types don't match"),
		}
	}

	#[test]
	fn test_pairing_messages() {
		use chrono::Utc;

		// Test that pairing messages can be created
		let challenge = PairingMessage::Challenge {
			initiator_nonce: [1u8; 16],
			timestamp: Utc::now(),
		};

		match challenge {
			PairingMessage::Challenge {
				initiator_nonce, ..
			} => {
				assert_eq!(initiator_nonce.len(), 16);
			}
			_ => panic!("Wrong message type"),
		}
	}

	#[test]
	fn test_challenge_hash_consistency() {
		let code = PairingCode::generate().unwrap();
		let initiator_nonce = [1u8; 16];
		let joiner_nonce = [2u8; 16];

		let hash1 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);
		let hash2 = code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);

		assert_eq!(hash1, hash2);
	}
}
