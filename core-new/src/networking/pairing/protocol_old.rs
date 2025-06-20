//! Pairing protocol messages and handlers

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

use crate::networking::{
    identity::{PrivateKey, PublicKey},
    DeviceInfo, NetworkError, Result,
};
use super::{PairingCode, PairingConnection, PairingConnectionState};

// Session keys and protocol handler are defined later in the file

/// Pairing protocol messages
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PairingMessage {
    /// Initiator sends challenge
    Challenge {
        initiator_nonce: [u8; 16],
        timestamp: DateTime<Utc>,
    },
    
    /// Joiner responds with proof of pairing code knowledge
    ChallengeResponse {
        response_hash: [u8; 32],
        joiner_nonce: [u8; 16],
        timestamp: DateTime<Utc>,
    },
    
    /// Initiator confirms joiner's response and proves own knowledge
    ChallengeConfirmation {
        confirmation_hash: [u8; 32],
        timestamp: DateTime<Utc>,
    },
    
    /// Device information exchange
    DeviceInfo {
        device_info: DeviceInfo,
        public_key: PublicKey,
        signature: Vec<u8>,
    },
    
    /// User confirmation of pairing
    PairingConfirmation {
        accepted: bool,
        user_confirmation_signature: Vec<u8>,
    },
    
    /// Session key exchange using X25519 ECDH
    SessionKeyExchange {
        ephemeral_public_key: [u8; 32],
        key_confirmation_hash: [u8; 32],
    },
    
    /// Error message
    Error {
        message: String,
        code: ErrorCode,
    },
}

/// Error codes for pairing protocol
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ErrorCode {
    InvalidChallenge,
    InvalidResponse,
    AuthenticationFailed,
    UserRejected,
    Timeout,
    ProtocolError,
}

// SessionKeys struct moved to end of file

impl SessionKeys {
    /// Derive session keys using HKDF
    pub fn derive_from_shared_secret(
        shared_secret: &[u8; 32],
        local_device_id: &uuid::Uuid,
        remote_device_id: &uuid::Uuid,
    ) -> Result<Self> {
        use hkdf::Hkdf;
        use sha2::Sha256;
        
        let salt = b"spacedrive-session-keys-v1";
        let info_base = format!("{}:{}", local_device_id, remote_device_id);
        
        let hk = Hkdf::<Sha256>::new(Some(salt), shared_secret);
        
        let mut send_key = [0u8; 32];
        let mut receive_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        let mut initial_iv = [0u8; 12];
        
        hk.expand(format!("{}-send", info_base).as_bytes(), &mut send_key)
            .map_err(|e| NetworkError::EncryptionError(format!("HKDF expand failed: {:?}", e)))?;
        hk.expand(format!("{}-receive", info_base).as_bytes(), &mut receive_key)
            .map_err(|e| NetworkError::EncryptionError(format!("HKDF expand failed: {:?}", e)))?;
        hk.expand(format!("{}-mac", info_base).as_bytes(), &mut mac_key)
            .map_err(|e| NetworkError::EncryptionError(format!("HKDF expand failed: {:?}", e)))?;
        hk.expand(format!("{}-iv", info_base).as_bytes(), &mut initial_iv)
            .map_err(|e| NetworkError::EncryptionError(format!("HKDF expand failed: {:?}", e)))?;
        
        Ok(SessionKeys {
            send_key,
            receive_key,
            mac_key,
            initial_iv,
        })
    }
}

// PairingProtocolHandler moved to end of file

// Temporary struct for tests
struct OldPairingProtocolHandler;

impl OldPairingProtocolHandler {
    /// Serialize pairing message
    pub fn serialize_message(message: &PairingMessage) -> Result<Vec<u8>> {
        serde_json::to_vec(message)
            .map_err(|e| NetworkError::SerializationError(format!("Message serialization failed: {}", e)))
    }
    
    /// Deserialize pairing message
    pub fn deserialize_message(data: &[u8]) -> Result<PairingMessage> {
        serde_json::from_slice(data)
            .map_err(|e| NetworkError::SerializationError(format!("Message deserialization failed: {}", e)))
    }
    
    /// Send pairing message
    pub async fn send_message(
        connection: &mut PairingConnection,
        message: PairingMessage,
    ) -> Result<()> {
        let data = Self::serialize_message(&message)?;
        connection.send_message(&data).await
    }
    
    /// Receive pairing message
    pub async fn receive_message(
        connection: &mut PairingConnection,
    ) -> Result<PairingMessage> {
        let data = connection.receive_message().await?;
        Self::deserialize_message(&data)
    }
    
    /// Perform challenge-response authentication as initiator
    pub async fn authenticate_as_initiator(
        connection: &mut PairingConnection,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        connection.set_state(PairingConnectionState::Authenticating);
        
        // 1. Send challenge to joiner
        let challenge = PairingMessage::Challenge {
            initiator_nonce: pairing_code.nonce,
            timestamp: Utc::now(),
        };
        Self::send_message(connection, challenge).await?;
        
        // 2. Receive and verify joiner's response
        let response = Self::receive_message(connection).await?;
        match response {
            PairingMessage::ChallengeResponse { 
                response_hash, 
                joiner_nonce, 
                timestamp 
            } => {
                // Verify joiner knows the pairing code
                let expected_hash = pairing_code.compute_challenge_hash(
                    &pairing_code.nonce,
                    &joiner_nonce,
                    timestamp,
                )?;
                
                if response_hash != expected_hash {
                    let error = PairingMessage::Error {
                        message: "Invalid challenge response".to_string(),
                        code: ErrorCode::InvalidResponse,
                    };
                    let _ = Self::send_message(connection, error).await;
                    return Err(NetworkError::AuthenticationFailed(
                        "Invalid challenge response".to_string()
                    ));
                }
                
                // 3. Send confirmation proving we also know the code
                let confirmation_hash = pairing_code.compute_challenge_hash(
                    &joiner_nonce,
                    &pairing_code.nonce,
                    timestamp,
                )?;
                
                let confirmation = PairingMessage::ChallengeConfirmation {
                    confirmation_hash,
                    timestamp: Utc::now(),
                };
                Self::send_message(connection, confirmation).await?;
                
                Ok(())
            }
            PairingMessage::Error { message, .. } => {
                Err(NetworkError::AuthenticationFailed(format!("Remote error: {}", message)))
            }
            _ => Err(NetworkError::ProtocolError("Unexpected message type".to_string())),
        }
    }
    
    /// Perform challenge-response authentication as joiner
    pub async fn authenticate_as_joiner(
        connection: &mut PairingConnection,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        connection.set_state(PairingConnectionState::Authenticating);
        
        // 1. Receive challenge from initiator
        let challenge = Self::receive_message(connection).await?;
        match challenge {
            PairingMessage::Challenge { initiator_nonce, timestamp } => {
                // 2. Verify challenge is recent (prevent replay attacks)
                let age = Utc::now().signed_duration_since(timestamp);
                if age.num_seconds() > 30 {
                    let error = PairingMessage::Error {
                        message: "Challenge too old".to_string(),
                        code: ErrorCode::Timeout,
                    };
                    let _ = Self::send_message(connection, error).await;
                    return Err(NetworkError::AuthenticationFailed(
                        "Challenge too old".to_string()
                    ));
                }
                
                // 3. Generate response proving we know the pairing code
                let joiner_nonce = Self::generate_nonce();
                let response_hash = pairing_code.compute_challenge_hash(
                    &initiator_nonce,
                    &joiner_nonce,
                    timestamp,
                )?;
                
                let response = PairingMessage::ChallengeResponse {
                    response_hash,
                    joiner_nonce,
                    timestamp: Utc::now(),
                };
                Self::send_message(connection, response).await?;
                
                // 4. Receive and verify initiator's confirmation
                let confirmation = Self::receive_message(connection).await?;
                match confirmation {
                    PairingMessage::ChallengeConfirmation { 
                        confirmation_hash, 
                        timestamp: conf_timestamp 
                    } => {
                        let expected_hash = pairing_code.compute_challenge_hash(
                            &joiner_nonce,
                            &initiator_nonce,
                            conf_timestamp,
                        )?;
                        
                        if confirmation_hash != expected_hash {
                            return Err(NetworkError::AuthenticationFailed(
                                "Invalid challenge confirmation".to_string()
                            ));
                        }
                        
                        Ok(())
                    }
                    PairingMessage::Error { message, .. } => {
                        Err(NetworkError::AuthenticationFailed(format!("Remote error: {}", message)))
                    }
                    _ => Err(NetworkError::ProtocolError("Unexpected message type".to_string())),
                }
            }
            PairingMessage::Error { message, .. } => {
                Err(NetworkError::AuthenticationFailed(format!("Remote error: {}", message)))
            }
            _ => Err(NetworkError::ProtocolError("Expected challenge message".to_string())),
        }
    }
    
    /// Exchange device information and public keys
    pub async fn exchange_device_information(
        connection: &mut PairingConnection,
        local_private_key: &PrivateKey,
    ) -> Result<DeviceInfo> {
        connection.set_state(PairingConnectionState::ExchangingKeys);
        
        let local_device = connection.local_device().clone();
        
        // 1. Prepare our device information with signature
        let device_message = Self::create_signed_device_message(
            &local_device,
            local_private_key,
        )?;
        
        // 2. Send our device information first
        Self::send_message(connection, device_message).await?;
        
        // 3. Receive remote device information
        let remote_message = Self::receive_message(connection).await?;
        
        // 4. Verify remote device information
        match remote_message {
            PairingMessage::DeviceInfo { 
                device_info, 
                public_key, 
                signature 
            } => {
                // Verify signature over device info + public key
                let signed_data = Self::serialize_for_signature(&device_info, &public_key)?;
                if !Self::verify_signature(&public_key, &signed_data, &signature) {
                    return Err(NetworkError::AuthenticationFailed(
                        "Invalid device signature".to_string()
                    ));
                }
                
                // Verify network fingerprint matches computed value
                let expected_fingerprint = crate::networking::NetworkFingerprint::from_device(
                    device_info.device_id, 
                    &public_key
                );
                if device_info.network_fingerprint != expected_fingerprint {
                    return Err(NetworkError::AuthenticationFailed(
                        "Network fingerprint mismatch".to_string()
                    ));
                }
                
                connection.set_remote_device(device_info.clone());
                Ok(device_info)
            }
            PairingMessage::Error { message, .. } => {
                Err(NetworkError::AuthenticationFailed(format!("Remote error: {}", message)))
            }
            _ => Err(NetworkError::ProtocolError("Expected device info message".to_string())),
        }
    }
    
    /// Establish session keys using X25519 ECDH
    pub async fn establish_session_keys(
        connection: &mut PairingConnection,
    ) -> Result<SessionKeys> {
        // Generate ephemeral key pair for forward secrecy
        let local_ephemeral_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let local_ephemeral_public = X25519PublicKey::from(&local_ephemeral_secret);
        
        // Send our ephemeral public key
        let key_exchange = PairingMessage::SessionKeyExchange {
            ephemeral_public_key: *local_ephemeral_public.as_bytes(),
            key_confirmation_hash: [0u8; 32], // Will be computed after ECDH
        };
        
        // Send our ephemeral public key
        Self::send_message(connection, key_exchange).await?;
        
        // Receive remote ephemeral public key
        let remote_exchange = Self::receive_message(connection).await?;
        
        match remote_exchange {
            PairingMessage::SessionKeyExchange { 
                ephemeral_public_key: remote_public_bytes,
                key_confirmation_hash: _,
            } => {
                let remote_ephemeral_public = X25519PublicKey::from(remote_public_bytes);
                
                // Perform ECDH to get shared secret
                let shared_secret = local_ephemeral_secret.diffie_hellman(&remote_ephemeral_public);
                
                // Derive session keys
                let local_device = connection.local_device();
                let remote_device = connection.remote_device()
                    .ok_or_else(|| NetworkError::ProtocolError("No remote device info".to_string()))?;
                
                let session_keys = SessionKeys::derive_from_shared_secret(
                    shared_secret.as_bytes(),
                    &local_device.device_id,
                    &remote_device.device_id,
                )?;
                
                Ok(session_keys)
            }
            PairingMessage::Error { message, .. } => {
                Err(NetworkError::AuthenticationFailed(format!("Remote error: {}", message)))
            }
            _ => Err(NetworkError::ProtocolError("Expected key exchange message".to_string())),
        }
    }
    
    /// Create signed device message
    fn create_signed_device_message(
        device_info: &DeviceInfo,
        private_key: &PrivateKey,
    ) -> Result<PairingMessage> {
        let public_key = private_key.public_key();
        let signed_data = Self::serialize_for_signature(device_info, &public_key)?;
        let signature = private_key.sign(&signed_data);
        
        Ok(PairingMessage::DeviceInfo {
            device_info: device_info.clone(),
            public_key,
            signature,
        })
    }
    
    /// Serialize data for signature
    fn serialize_for_signature(
        device_info: &DeviceInfo,
        public_key: &PublicKey,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(device_info.device_id.as_bytes());
        data.extend_from_slice(device_info.device_name.as_bytes());
        data.extend_from_slice(public_key.as_bytes());
        data.extend_from_slice(b"spacedrive-device-signature-v1");
        Ok(data)
    }
    
    /// Verify signature (simplified - should use proper Ed25519 verification)
    fn verify_signature(public_key: &PublicKey, data: &[u8], signature: &[u8]) -> bool {
        use ring::signature;
        
        let public_key_ring = signature::UnparsedPublicKey::new(&signature::ED25519, public_key.as_bytes());
        public_key_ring.verify(data, signature).is_ok()
    }
    
    /// Generate random nonce
    fn generate_nonce() -> [u8; 16] {
        use ring::rand::{SecureRandom, SystemRandom};
        
        let mut nonce = [0u8; 16];
        let rng = SystemRandom::new();
        rng.fill(&mut nonce).expect("Random generation should not fail");
        nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::identity::PublicKey;
    use uuid::Uuid;

    fn create_test_device_info() -> DeviceInfo {
        DeviceInfo::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            PublicKey::from_bytes(vec![0u8; 32]).unwrap(),
        )
    }

    #[test]
    fn test_message_serialization() {
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
}

impl PairingProtocolHandler {
    /// Authenticate as the initiator (server side)
    pub async fn authenticate_as_initiator(
        connection: &mut PairingConnection,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        // Generate challenge for joiner
        let mut initiator_nonce = [0u8; 16];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut initiator_nonce);
        
        let challenge = PairingMessage::Challenge {
            initiator_nonce,
            timestamp: Utc::now(),
        };
        
        // Send challenge
        Self::send_message(connection, challenge).await?;
        
        // Receive response
        let response = Self::receive_message(connection).await?;
        
        match response {
            PairingMessage::ChallengeResponse { response_hash, joiner_nonce, timestamp: _ } => {
                // Verify the joiner knows the pairing code
                let expected_hash = pairing_code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);
                if response_hash != expected_hash {
                    return Err(NetworkError::AuthenticationFailed("Invalid challenge response".to_string()));
                }
                
                // Send confirmation proving we also know the pairing code
                let confirmation_hash = pairing_code.compute_challenge_hash(&joiner_nonce, &initiator_nonce);
                let confirmation = PairingMessage::ChallengeConfirmation {
                    confirmation_hash,
                    timestamp: Utc::now(),
                };
                
                Self::send_message(connection, confirmation).await?;
                Ok(())
            }
            _ => Err(NetworkError::ProtocolError("Expected challenge response".to_string()))
        }
    }
    
    /// Authenticate as the joiner (client side)
    pub async fn authenticate_as_joiner(
        connection: &mut PairingConnection,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        // Receive challenge from initiator
        let challenge = Self::receive_message(connection).await?;
        
        match challenge {
            PairingMessage::Challenge { initiator_nonce, timestamp: _ } => {
                // Generate our nonce
                let mut joiner_nonce = [0u8; 16];
                use rand::RngCore;
                rand::rngs::OsRng.fill_bytes(&mut joiner_nonce);
                
                // Compute response proving we know the pairing code
                let response_hash = pairing_code.compute_challenge_hash(&initiator_nonce, &joiner_nonce);
                
                let response = PairingMessage::ChallengeResponse {
                    response_hash,
                    joiner_nonce,
                    timestamp: Utc::now(),
                };
                
                // Send response
                Self::send_message(connection, response).await?;
                
                // Receive confirmation
                let confirmation = Self::receive_message(connection).await?;
                
                match confirmation {
                    PairingMessage::ChallengeConfirmation { confirmation_hash, timestamp: _ } => {
                        // Verify initiator also knows the pairing code
                        let expected_hash = pairing_code.compute_challenge_hash(&joiner_nonce, &initiator_nonce);
                        if confirmation_hash != expected_hash {
                            return Err(NetworkError::AuthenticationFailed("Invalid confirmation".to_string()));
                        }
                        Ok(())
                    }
                    _ => Err(NetworkError::ProtocolError("Expected challenge confirmation".to_string()))
                }
            }
            _ => Err(NetworkError::ProtocolError("Expected challenge".to_string()))
        }
    }
    
    /// Exchange device information
    pub async fn exchange_device_information(
        connection: &mut PairingConnection,
        local_private_key: &PrivateKey,
    ) -> Result<DeviceInfo> {
        // Create device info message with signature
        let local_device = connection.local_device().clone();
        let device_data = serde_json::to_vec(&local_device)
            .map_err(|e| NetworkError::SerializationError(format!("Device serialization failed: {}", e)))?;
        
        // Sign the device data
        let signature = local_private_key.sign(&device_data)
            .map_err(|e| NetworkError::EncryptionError(format!("Signing failed: {}", e)))?;
        
        let public_key = local_private_key.public_key();
        
        let device_message = PairingMessage::DeviceInfo {
            device_info: local_device,
            public_key: public_key.to_bytes(),
            signature: signature.to_bytes(),
        };
        
        // Send our device info
        Self::send_message(connection, device_message).await?;
        
        // Receive remote device info
        let remote_message = Self::receive_message(connection).await?;
        
        match remote_message {
            PairingMessage::DeviceInfo { device_info, public_key, signature } => {
                // Verify the signature
                let remote_public_key = PublicKey::from_bytes(public_key)
                    .map_err(|e| NetworkError::EncryptionError(format!("Invalid public key: {}", e)))?;
                
                let device_data = serde_json::to_vec(&device_info)
                    .map_err(|e| NetworkError::SerializationError(format!("Device serialization failed: {}", e)))?;
                
                if !remote_public_key.verify(&device_data, &signature) {
                    return Err(NetworkError::AuthenticationFailed("Invalid device signature".to_string()));
                }
                
                Ok(device_info)
            }
            _ => Err(NetworkError::ProtocolError("Expected device info".to_string()))
        }
    }
    
    /// Establish session keys using ECDH
    pub async fn establish_session_keys(
        connection: &mut PairingConnection,
    ) -> Result<SessionKeys> {
        // Generate ephemeral key pair
        let local_ephemeral_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let local_ephemeral_public = X25519PublicKey::from(&local_ephemeral_secret);
        
        // Create key exchange message
        let key_exchange = PairingMessage::SessionKeyExchange {
            ephemeral_public_key: *local_ephemeral_public.as_bytes(),
            key_confirmation_hash: [0u8; 32], // Placeholder
        };
        
        // Send our ephemeral public key
        Self::send_message(connection, key_exchange).await?;
        
        // Receive remote ephemeral public key
        let remote_exchange = Self::receive_message(connection).await?;
        
        match remote_exchange {
            PairingMessage::SessionKeyExchange { 
                ephemeral_public_key: remote_public_bytes,
                key_confirmation_hash: _,
            } => {
                let remote_ephemeral_public = X25519PublicKey::from(remote_public_bytes);
                
                // Compute shared secret
                let shared_secret = local_ephemeral_secret.diffie_hellman(&remote_ephemeral_public);
                
                // Derive session keys using HKDF
                SessionKeys::derive_from_shared_secret(shared_secret.as_bytes())
            }
            _ => Err(NetworkError::ProtocolError("Expected key exchange".to_string()))
        }
    }
    
    /// Helper method to send messages
    async fn send_message(connection: &mut PairingConnection, message: PairingMessage) -> Result<()> {
        let data = serde_json::to_vec(&message)
            .map_err(|e| NetworkError::SerializationError(format!("Message serialization failed: {}", e)))?;
        
        connection.send_message(&data).await
    }
    
    /// Helper method to receive messages
    async fn receive_message(connection: &mut PairingConnection) -> Result<PairingMessage> {
        let data = connection.receive_message().await?;
        
        serde_json::from_slice(&data)
            .map_err(|e| NetworkError::SerializationError(format!("Message deserialization failed: {}", e)))
    }
}

impl SessionKeys {
    /// Derive session keys from shared secret using HKDF
    fn derive_from_shared_secret(shared_secret: &[u8]) -> Result<Self> {
        use hkdf::Hkdf;
        use sha2::Sha256;
        
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret);
        let mut send_key = [0u8; 32];
        let mut receive_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        
        hkdf.expand(b"spacedrive-pairing-send-key-v1", &mut send_key)
            .map_err(|e| NetworkError::EncryptionError(format!("Key derivation failed: {:?}", e)))?;
        hkdf.expand(b"spacedrive-pairing-receive-key-v1", &mut receive_key)
            .map_err(|e| NetworkError::EncryptionError(format!("Key derivation failed: {:?}", e)))?;
        hkdf.expand(b"spacedrive-pairing-mac-key-v1", &mut mac_key)
            .map_err(|e| NetworkError::EncryptionError(format!("Key derivation failed: {:?}", e)))?;
        
        Ok(SessionKeys {
            send_key,
            receive_key,
            mac_key,
        })
    }
}