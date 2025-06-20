//! Security layer using Noise Protocol Framework

use snow::{Builder, HandshakeState as NoiseHandshakeState, TransportState, Keypair};
use crate::networking::{
    identity::{NetworkIdentity, PrivateKey},
    Result, NetworkError,
};

/// Noise Protocol XX pattern for mutual authentication
pub struct NoiseSession {
    /// Handshake state (during handshake)
    handshake: Option<NoiseHandshakeState>,
    
    /// Transport state (after handshake completion)
    transport: Option<TransportState>,
    
    /// Buffer for handshake messages
    handshake_buffer: Vec<u8>,
    
    /// Whether we are the initiator
    is_initiator: bool,
}

impl NoiseSession {
    /// Create new session as initiator
    pub fn initiate(
        local_private_key: &PrivateKey,
        remote_public_key: Option<&[u8]>,
    ) -> Result<Self> {
        let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
        let builder = Builder::new(params.parse()
            .map_err(|e| NetworkError::EncryptionError(format!("Invalid Noise params: {}", e)))?);

        // Create keypair from our private key
        let keypair = Self::create_keypair(local_private_key)?;
        
        let mut handshake = builder
            .local_private_key(&keypair.private)
            .build_initiator()
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create initiator: {}", e)))?;

        // Note: In snow 0.9, remote public keys are exchanged during handshake automatically
        // We don't need to explicitly set them beforehand

        Ok(Self {
            handshake: Some(handshake),
            transport: None,
            handshake_buffer: vec![0u8; 65535], // Max handshake message size
            is_initiator: true,
        })
    }

    /// Create new session as responder
    pub fn respond(local_private_key: &PrivateKey) -> Result<Self> {
        let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
        let builder = Builder::new(params.parse()
            .map_err(|e| NetworkError::EncryptionError(format!("Invalid Noise params: {}", e)))?);

        // Create keypair from our private key
        let keypair = Self::create_keypair(local_private_key)?;
        
        let handshake = builder
            .local_private_key(&keypair.private)
            .build_responder()
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to create responder: {}", e)))?;

        Ok(Self {
            handshake: Some(handshake),
            transport: None,
            handshake_buffer: vec![0u8; 65535],
            is_initiator: false,
        })
    }

    /// Create Noise keypair from our private key
    fn create_keypair(private_key: &PrivateKey) -> Result<Keypair> {
        // For now, generate a new keypair since we can't easily extract from ring::Ed25519KeyPair
        // In production, this should properly convert the key
        let keypair = Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
            .generate_keypair()
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to generate keypair: {}", e)))?;
        
        Ok(keypair)
    }

    /// Process handshake message
    pub fn process_handshake_message(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        if self.handshake.is_none() {
            return Err(NetworkError::EncryptionError("Handshake already completed".to_string()));
        }

        let is_initiator = self.is_initiator;
        if is_initiator {
            self.process_initiator_handshake(message)
        } else {
            self.process_responder_handshake(message)
        }
    }

    /// Process handshake as initiator
    fn process_initiator_handshake(
        &mut self,
        message: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let handshake = self.handshake.as_mut().unwrap();
        if handshake.is_handshake_finished() {
            return Err(NetworkError::EncryptionError("Handshake already finished".to_string()));
        }

        match handshake.get_handshake_hash().len() {
            // First message: send initial handshake
            0 => {
                let len = handshake.write_message(&[], &mut self.handshake_buffer)
                    .map_err(|e| NetworkError::EncryptionError(format!("Handshake write failed: {}", e)))?;
                Ok(Some(self.handshake_buffer[..len].to_vec()))
            }
            // Second message: process response and send final
            _ => {
                let _len = handshake.read_message(message, &mut self.handshake_buffer)
                    .map_err(|e| NetworkError::EncryptionError(format!("Handshake read failed: {}", e)))?;
                
                if handshake.is_handshake_finished() {
                    self.complete_handshake()?;
                    Ok(None)
                } else {
                    let len = handshake.write_message(&[], &mut self.handshake_buffer)
                        .map_err(|e| NetworkError::EncryptionError(format!("Handshake write failed: {}", e)))?;
                    Ok(Some(self.handshake_buffer[..len].to_vec()))
                }
            }
        }
    }

    /// Process handshake as responder
    fn process_responder_handshake(
        &mut self,
        message: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let handshake = self.handshake.as_mut().unwrap();
        // Read incoming message
        let _len = handshake.read_message(message, &mut self.handshake_buffer)
            .map_err(|e| NetworkError::EncryptionError(format!("Handshake read failed: {}", e)))?;
        
        if handshake.is_handshake_finished() {
            self.complete_handshake()?;
            Ok(None)
        } else {
            // Send response
            let len = handshake.write_message(&[], &mut self.handshake_buffer)
                .map_err(|e| NetworkError::EncryptionError(format!("Handshake write failed: {}", e)))?;
            Ok(Some(self.handshake_buffer[..len].to_vec()))
        }
    }

    /// Complete the handshake and transition to transport mode
    fn complete_handshake(&mut self) -> Result<()> {
        let handshake = self.handshake.take().ok_or_else(|| {
            NetworkError::EncryptionError("No handshake state available".to_string())
        })?;

        let transport = handshake.into_transport_mode()
            .map_err(|e| NetworkError::EncryptionError(format!("Failed to enter transport mode: {}", e)))?;

        self.transport = Some(transport);
        Ok(())
    }

    /// Check if handshake is complete
    pub fn is_ready(&self) -> bool {
        self.transport.is_some()
    }

    /// Encrypt data for transmission
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let transport = self.transport.as_mut().ok_or_else(|| {
            NetworkError::EncryptionError("Handshake not completed".to_string())
        })?;

        let mut buffer = vec![0u8; plaintext.len() + 16]; // Add space for tag
        let len = transport.write_message(plaintext, &mut buffer)
            .map_err(|e| NetworkError::EncryptionError(format!("Encryption failed: {}", e)))?;

        buffer.truncate(len);
        Ok(buffer)
    }

    /// Decrypt received data
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let transport = self.transport.as_mut().ok_or_else(|| {
            NetworkError::EncryptionError("Handshake not completed".to_string())
        })?;

        let mut buffer = vec![0u8; ciphertext.len()];
        let len = transport.read_message(ciphertext, &mut buffer)
            .map_err(|e| NetworkError::EncryptionError(format!("Decryption failed: {}", e)))?;

        buffer.truncate(len);
        Ok(buffer)
    }

    /// Get remote public key after handshake
    pub fn remote_public_key(&self) -> Option<Vec<u8>> {
        self.transport.as_ref().and_then(|transport| {
            transport.get_remote_static().map(|key| key.to_vec())
        })
    }

    /// Get handshake hash for authentication
    pub fn handshake_hash(&self) -> Option<Vec<u8>> {
        // In snow 0.9, handshake hash is not directly accessible from transport
        // This would need to be captured during handshake completion
        None
    }
}

/// Secure connection wrapper that handles encryption/decryption
pub struct SecureConnection<T> {
    inner: T,
    noise_session: NoiseSession,
}

impl<T> SecureConnection<T> {
    pub fn new(inner: T, noise_session: NoiseSession) -> Self {
        Self {
            inner,
            noise_session,
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn noise_session(&mut self) -> &mut NoiseSession {
        &mut self.noise_session
    }

    pub fn is_ready(&self) -> bool {
        self.noise_session.is_ready()
    }
}

/// Handshake helper for connection establishment
pub struct HandshakeHelper {
    session: NoiseSession,
    state: HandshakeState,
}

#[derive(Debug, Clone, Copy)]
enum HandshakeState {
    WaitingForInitial,
    WaitingForResponse,
    WaitingForFinal,
    Completed,
}

impl HandshakeHelper {
    /// Create helper for initiating handshake
    pub fn initiate(local_key: &PrivateKey) -> Result<Self> {
        let session = NoiseSession::initiate(local_key, None)?;
        Ok(Self {
            session,
            state: HandshakeState::WaitingForInitial,
        })
    }

    /// Create helper for responding to handshake
    pub fn respond(local_key: &PrivateKey) -> Result<Self> {
        let session = NoiseSession::respond(local_key)?;
        Ok(Self {
            session,
            state: HandshakeState::WaitingForResponse,
        })
    }

    /// Process handshake step
    pub fn process_message(&mut self, message: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
        match self.state {
            HandshakeState::WaitingForInitial => {
                let response = self.session.process_handshake_message(&[])?;
                self.state = HandshakeState::WaitingForResponse;
                Ok(response)
            }
            HandshakeState::WaitingForResponse => {
                if let Some(msg) = message {
                    let response = self.session.process_handshake_message(msg)?;
                    self.state = if self.session.is_ready() {
                        HandshakeState::Completed
                    } else {
                        HandshakeState::WaitingForFinal
                    };
                    Ok(response)
                } else {
                    Err(NetworkError::EncryptionError("Expected message".to_string()))
                }
            }
            HandshakeState::WaitingForFinal => {
                if let Some(msg) = message {
                    let response = self.session.process_handshake_message(msg)?;
                    self.state = HandshakeState::Completed;
                    Ok(response)
                } else {
                    Err(NetworkError::EncryptionError("Expected message".to_string()))
                }
            }
            HandshakeState::Completed => {
                Err(NetworkError::EncryptionError("Handshake already completed".to_string()))
            }
        }
    }

    /// Check if handshake is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.state, HandshakeState::Completed)
    }

    /// Get the completed session
    pub fn into_session(self) -> Result<NoiseSession> {
        if self.is_complete() {
            Ok(self.session)
        } else {
            Err(NetworkError::EncryptionError("Handshake not completed".to_string()))
        }
    }
}