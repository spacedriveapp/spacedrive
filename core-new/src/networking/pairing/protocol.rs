//! Pairing protocol messages and handlers

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

use crate::networking::{
    identity::{PrivateKey, PublicKey, Signature},
    DeviceInfo, NetworkError, Result,
};
use super::{PairingCode, PairingConnection};

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
    
    /// Device information exchange with signature
    DeviceInfo {
        device_info: DeviceInfo,
        public_key: Vec<u8>,
        signature: Vec<u8>,
    },
    
    /// Session key exchange
    SessionKeyExchange {
        ephemeral_public_key: [u8; 32],
        key_confirmation_hash: [u8; 32],
    },
    
    /// Pairing completed successfully
    PairingComplete,
    
    /// Pairing failed with error
    PairingError { 
        error: String 
    },
}

/// Session keys derived from ECDH
#[derive(Debug, Clone)]
pub struct SessionKeys {
    pub send_key: [u8; 32],
    pub receive_key: [u8; 32], 
    pub mac_key: [u8; 32],
}

/// Protocol handler for pairing operations
pub struct PairingProtocolHandler;

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
    
    /// Exchange device information as initiator (send first, then receive)
    pub async fn exchange_device_information_as_initiator(
        connection: &mut PairingConnection,
        local_private_key: &PrivateKey,
    ) -> Result<DeviceInfo> {
        let local_device = connection.local_device().clone();
        
        // Create and send our device info first
        let device_message = Self::create_device_info_message(&local_device, local_private_key)?;
        Self::send_message(connection, device_message).await?;
        
        // Then receive remote device info
        let remote_message = Self::receive_message(connection).await?;
        
        match remote_message {
            PairingMessage::DeviceInfo { device_info, public_key, signature } => {
                Self::verify_device_info(&device_info, &public_key, &signature)?;
                Ok(device_info)
            }
            _ => Err(NetworkError::ProtocolError("Expected device info".to_string()))
        }
    }
    
    /// Exchange device information as joiner (receive first, then send)
    pub async fn exchange_device_information_as_joiner(
        connection: &mut PairingConnection,
        local_private_key: &PrivateKey,
    ) -> Result<DeviceInfo> {
        // First receive remote device info
        let remote_message = Self::receive_message(connection).await?;
        
        let remote_device = match remote_message {
            PairingMessage::DeviceInfo { device_info, public_key, signature } => {
                Self::verify_device_info(&device_info, &public_key, &signature)?;
                device_info
            }
            _ => return Err(NetworkError::ProtocolError("Expected device info".to_string()))
        };
        
        // Then send our device info
        let local_device = connection.local_device().clone();
        let device_message = Self::create_device_info_message(&local_device, local_private_key)?;
        Self::send_message(connection, device_message).await?;
        
        Ok(remote_device)
    }
    
    /// Create device info message with signature
    fn create_device_info_message(
        device_info: &DeviceInfo,
        private_key: &PrivateKey,
    ) -> Result<PairingMessage> {
        // Serialize device data for signing
        let device_data = serde_json::to_vec(device_info)
            .map_err(|e| NetworkError::SerializationError(format!("Device serialization failed: {}", e)))?;
        
        // Sign the device data
        let signature = private_key.sign(&device_data)?;
        let public_key = private_key.public_key();
        
        Ok(PairingMessage::DeviceInfo {
            device_info: device_info.clone(),
            public_key: public_key.to_bytes(),
            signature: signature.to_bytes(),
        })
    }
    
    /// Verify device info signature
    fn verify_device_info(
        device_info: &DeviceInfo,
        public_key_bytes: &[u8],
        signature_bytes: &[u8],
    ) -> Result<()> {
        // Reconstruct the public key
        let remote_public_key = PublicKey::from_bytes(public_key_bytes.to_vec())
            .map_err(|e| NetworkError::EncryptionError(format!("Invalid public key: {}", e)))?;
        
        // Serialize device data for verification
        let device_data = serde_json::to_vec(device_info)
            .map_err(|e| NetworkError::SerializationError(format!("Device serialization failed: {}", e)))?;
        
        // Verify signature
        if !remote_public_key.verify(&device_data, signature_bytes) {
            return Err(NetworkError::AuthenticationFailed("Invalid device signature".to_string()));
        }
        
        Ok(())
    }
    
    /// Establish session keys as initiator (send first, then receive)
    pub async fn establish_session_keys_as_initiator(
        connection: &mut PairingConnection,
    ) -> Result<SessionKeys> {
        // Generate ephemeral key pair
        let local_ephemeral_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let local_ephemeral_public = X25519PublicKey::from(&local_ephemeral_secret);
        
        // Send our ephemeral public key first
        let key_exchange = PairingMessage::SessionKeyExchange {
            ephemeral_public_key: *local_ephemeral_public.as_bytes(),
            key_confirmation_hash: [0u8; 32], // Will be computed after ECDH
        };
        Self::send_message(connection, key_exchange).await?;
        
        // Receive remote ephemeral public key
        let remote_exchange = Self::receive_message(connection).await?;
        
        match remote_exchange {
            PairingMessage::SessionKeyExchange { 
                ephemeral_public_key: remote_public_bytes,
                key_confirmation_hash: _,
            } => {
                let remote_ephemeral_public = X25519PublicKey::from(remote_public_bytes);
                
                // Compute shared secret and derive keys
                let shared_secret = local_ephemeral_secret.diffie_hellman(&remote_ephemeral_public);
                SessionKeys::derive_from_shared_secret(shared_secret.as_bytes())
            }
            _ => Err(NetworkError::ProtocolError("Expected key exchange".to_string()))
        }
    }
    
    /// Establish session keys as joiner (receive first, then send)
    pub async fn establish_session_keys_as_joiner(
        connection: &mut PairingConnection,
    ) -> Result<SessionKeys> {
        // First receive remote ephemeral public key
        let remote_exchange = Self::receive_message(connection).await?;
        
        let remote_public_bytes = match remote_exchange {
            PairingMessage::SessionKeyExchange { 
                ephemeral_public_key: remote_public_bytes,
                key_confirmation_hash: _,
            } => remote_public_bytes,
            _ => return Err(NetworkError::ProtocolError("Expected key exchange".to_string()))
        };
        
        // Generate our ephemeral key pair
        let local_ephemeral_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let local_ephemeral_public = X25519PublicKey::from(&local_ephemeral_secret);
        
        // Send our ephemeral public key
        let key_exchange = PairingMessage::SessionKeyExchange {
            ephemeral_public_key: *local_ephemeral_public.as_bytes(),
            key_confirmation_hash: [0u8; 32], // Will be computed after ECDH
        };
        Self::send_message(connection, key_exchange).await?;
        
        // Compute shared secret and derive keys
        let remote_ephemeral_public = X25519PublicKey::from(remote_public_bytes);
        let shared_secret = local_ephemeral_secret.diffie_hellman(&remote_ephemeral_public);
        SessionKeys::derive_from_shared_secret(shared_secret.as_bytes())
    }
    
    /// Helper method to send messages
    async fn send_message(connection: &mut PairingConnection, message: PairingMessage) -> Result<()> {
        let data = crate::networking::serialization::serialize_with_context(&message, "Pairing message serialization failed")?;
        connection.send_message(&data).await
    }
    
    /// Helper method to receive messages
    async fn receive_message(connection: &mut PairingConnection) -> Result<PairingMessage> {
        let data = connection.receive_message().await?;
        crate::networking::serialization::deserialize_with_context(&data, "Pairing message deserialization failed")
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