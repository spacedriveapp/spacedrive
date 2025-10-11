<!--CREATED: 2025-06-19-->
# Spacedrive Device Pairing Protocol Design

## Overview

This document describes the complete design for Spacedrive's secure device pairing protocol. The pairing system allows two Spacedrive devices to establish trust and begin secure communication using a human-readable pairing code.

## Goals

### Primary Goals
- **Security**: Cryptographically secure pairing resistant to common attacks
- **Usability**: Simple 6-word pairing codes that users can easily share
- **Reliability**: Robust discovery and connection establishment
- **Privacy**: No sensitive data transmitted in plaintext during pairing
- **Scalability**: Support for pairing multiple devices in a mesh network

### Security Goals
- Protection against man-in-the-middle (MITM) attacks
- Protection against eavesdropping on pairing communications
- Protection against replay attacks and brute force attempts
- Forward secrecy for post-pairing communications
- Mutual authentication of both devices

## Architecture Overview

```
Device A (Initiator)                    Device B (Joiner)
        │                                       │
        ▼                                       ▼
   Generate Code                          Enter Code
        │                                       │
        ▼                                       ▼
   Start mDNS Broadcast              Scan for mDNS Announcements
        │                                       │
        ▼                                       ▼
   Listen for Connections            Establish Connection
        │◄──────────────────────────────────────┤
        ▼                                       ▼
   Challenge-Response Authentication
        │◄──────────────────────────────────────┤
        ▼                                       ▼
   Exchange Device Information & Public Keys
        │◄──────────────────────────────────────┤
        ▼                                       ▼
   User Confirmation                    User Confirmation
        │                                       │
        ▼                                       ▼
   Store Device Info                    Store Device Info
        │                                       │
        ▼                                       ▼
   Establish Session Keys              Establish Session Keys
```

## Component Design

### 1. Pairing Code System

#### Enhanced PairingCode Structure
```rust
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
```

#### BIP39 Word Encoding
```rust
impl PairingCode {
    /// Generate using proper BIP39 wordlist instead of hex
    pub fn generate() -> Result<Self> {
        let mut secret = [0u8; 32];
        let mut nonce = [0u8; 16];
        let rng = ring::rand::SystemRandom::new();
        
        rng.fill(&mut secret)?;
        rng.fill(&mut nonce)?;
        
        // Convert first 24 bytes to 6 BIP39 words (4 bytes per word)
        let words = bip39::encode_bytes(&secret[..24])?;
        
        // Derive discovery fingerprint from secret + device context
        let discovery_fingerprint = Self::derive_fingerprint(&secret);
        
        Ok(PairingCode {
            secret,
            expires_at: Utc::now() + Duration::minutes(5),
            words,
            discovery_fingerprint,
            nonce,
        })
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
}
```

### 2. Network Discovery System

#### mDNS Broadcasting
```rust
pub struct PairingBroadcaster {
    mdns_service: mdns::Service,
    pairing_code: PairingCode,
    device_info: DeviceInfo,
}

impl PairingBroadcaster {
    pub async fn start_broadcast(
        &self,
        code: &PairingCode,
        device_info: &DeviceInfo,
    ) -> Result<()> {
        // Broadcast mDNS service with pairing fingerprint
        let service_name = format!(
            "_spacedrive-pairing._tcp.local."
        );
        
        let txt_records = vec![
            format!("fp={}", hex::encode(code.discovery_fingerprint)),
            format!("device={}", device_info.device_name),
            format!("version=1"),
            format!("expires={}", code.expires_at.timestamp()),
        ];
        
        self.mdns_service.register(
            service_name,
            self.get_local_port(),
            txt_records,
        ).await
    }
}
```

#### Device Discovery
```rust
pub struct PairingScanner {
    mdns_scanner: mdns::Scanner,
}

impl PairingScanner {
    pub async fn find_pairing_device(
        &self,
        code: &PairingCode,
    ) -> Result<PairingTarget> {
        let target_fingerprint = hex::encode(code.discovery_fingerprint);
        
        // Scan for mDNS services matching our pairing fingerprint
        let services = self.mdns_scanner
            .scan_for("_spacedrive-pairing._tcp.local.", Duration::from_secs(10))
            .await?;
        
        for service in services {
            if let Some(fp) = service.txt_record("fp") {
                if fp == target_fingerprint {
                    return Ok(PairingTarget {
                        address: service.address(),
                        port: service.port(),
                        device_name: service.txt_record("device").unwrap_or_default(),
                        expires_at: service.txt_record("expires")
                            .and_then(|s| s.parse::<i64>().ok())
                            .map(|ts| DateTime::from_timestamp(ts, 0))
                            .flatten(),
                    });
                }
            }
        }
        
        Err(NetworkError::DeviceNotFound("No matching pairing device found".into()))
    }
}
```

### 3. Secure Transport Layer

#### Pairing Connection
```rust
pub struct PairingConnection {
    transport: Box<dyn SecureTransport>,
    state: PairingState,
    local_device: DeviceInfo,
}

#[derive(Debug, Clone)]
pub enum PairingState {
    Connecting,
    Authenticating,
    ExchangingKeys,
    AwaitingConfirmation,
    Completed,
    Failed(String),
}

impl PairingConnection {
    /// Establish secure connection for pairing
    pub async fn connect_for_pairing(
        target: PairingTarget,
        local_device: DeviceInfo,
    ) -> Result<Self> {
        // Use TLS with ephemeral certificates for initial security
        let tls_config = Self::create_ephemeral_tls_config()?;
        let transport = TlsTransport::connect(target.address, target.port, tls_config).await?;
        
        Ok(PairingConnection {
            transport: Box::new(transport),
            state: PairingState::Connecting,
            local_device,
        })
    }
    
    /// Create self-signed certificate for pairing session
    fn create_ephemeral_tls_config() -> Result<TlsConfig> {
        // Generate ephemeral key pair for this pairing session
        let key_pair = rcgen::KeyPair::generate(&rcgen::PKCS_ED25519)?;
        let cert = rcgen::Certificate::from_params(
            rcgen::CertificateParams::new(vec!["spacedrive-pairing".to_string()])?
        )?;
        
        Ok(TlsConfig {
            certificate: cert.serialize_der()?,
            private_key: key_pair.serialize_der(),
            verify_mode: TlsVerifyMode::AllowSelfSigned, // For pairing only
        })
    }
}
```

### 4. Challenge-Response Authentication

#### Authentication Protocol
```rust
#[derive(Serialize, Deserialize, Debug)]
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
        signature: Vec<u8>, // Signature over device_info + public_key
    },
    
    /// User confirmation of pairing
    PairingConfirmation {
        accepted: bool,
        user_confirmation_signature: Vec<u8>,
    },
    
    /// Final session key establishment
    SessionKeyExchange {
        encrypted_session_key: Vec<u8>,
        key_confirmation_hash: [u8; 32],
    },
}

impl PairingConnection {
    /// Perform challenge-response authentication
    pub async fn authenticate_pairing_code(
        &mut self,
        pairing_code: &PairingCode,
        is_initiator: bool,
    ) -> Result<()> {
        self.state = PairingState::Authenticating;
        
        if is_initiator {
            self.authenticate_as_initiator(pairing_code).await
        } else {
            self.authenticate_as_joiner(pairing_code).await
        }
    }
    
    async fn authenticate_as_initiator(
        &mut self,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        // 1. Send challenge to joiner
        let challenge = PairingMessage::Challenge {
            initiator_nonce: pairing_code.nonce,
            timestamp: Utc::now(),
        };
        self.send_message(challenge).await?;
        
        // 2. Receive and verify joiner's response
        let response = self.receive_message().await?;
        match response {
            PairingMessage::ChallengeResponse { 
                response_hash, 
                joiner_nonce, 
                timestamp 
            } => {
                // Verify joiner knows the pairing code
                let expected_hash = Self::compute_challenge_hash(
                    &pairing_code.secret,
                    &pairing_code.nonce,
                    &joiner_nonce,
                    timestamp,
                )?;
                
                if response_hash != expected_hash {
                    return Err(NetworkError::AuthenticationFailed(
                        "Invalid challenge response".into()
                    ));
                }
                
                // 3. Send confirmation proving we also know the code
                let confirmation_hash = Self::compute_challenge_hash(
                    &pairing_code.secret,
                    &joiner_nonce,
                    &pairing_code.nonce,
                    timestamp,
                )?;
                
                let confirmation = PairingMessage::ChallengeConfirmation {
                    confirmation_hash,
                    timestamp: Utc::now(),
                };
                self.send_message(confirmation).await?;
                
                Ok(())
            }
            _ => Err(NetworkError::ProtocolError("Unexpected message type".into())),
        }
    }
    
    async fn authenticate_as_joiner(
        &mut self,
        pairing_code: &PairingCode,
    ) -> Result<()> {
        // 1. Receive challenge from initiator
        let challenge = self.receive_message().await?;
        match challenge {
            PairingMessage::Challenge { initiator_nonce, timestamp } => {
                // 2. Verify challenge is recent (prevent replay attacks)
                let age = Utc::now().signed_duration_since(timestamp);
                if age.num_seconds() > 30 {
                    return Err(NetworkError::AuthenticationFailed(
                        "Challenge too old".into()
                    ));
                }
                
                // 3. Generate response proving we know the pairing code
                let joiner_nonce = Self::generate_nonce();
                let response_hash = Self::compute_challenge_hash(
                    &pairing_code.secret,
                    &initiator_nonce,
                    &joiner_nonce,
                    timestamp,
                )?;
                
                let response = PairingMessage::ChallengeResponse {
                    response_hash,
                    joiner_nonce,
                    timestamp: Utc::now(),
                };
                self.send_message(response).await?;
                
                // 4. Receive and verify initiator's confirmation
                let confirmation = self.receive_message().await?;
                match confirmation {
                    PairingMessage::ChallengeConfirmation { 
                        confirmation_hash, 
                        timestamp: conf_timestamp 
                    } => {
                        let expected_hash = Self::compute_challenge_hash(
                            &pairing_code.secret,
                            &joiner_nonce,
                            &initiator_nonce,
                            conf_timestamp,
                        )?;
                        
                        if confirmation_hash != expected_hash {
                            return Err(NetworkError::AuthenticationFailed(
                                "Invalid challenge confirmation".into()
                            ));
                        }
                        
                        Ok(())
                    }
                    _ => Err(NetworkError::ProtocolError("Unexpected message type".into())),
                }
            }
            _ => Err(NetworkError::ProtocolError("Expected challenge message".into())),
        }
    }
    
    /// Compute HMAC-based challenge hash
    fn compute_challenge_hash(
        secret: &[u8; 32],
        nonce1: &[u8; 16],
        nonce2: &[u8; 16],
        timestamp: DateTime<Utc>,
    ) -> Result<[u8; 32]> {
        use ring::hmac;
        
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let mut context = hmac::Context::with_key(&key);
        
        context.update(nonce1);
        context.update(nonce2);
        context.update(&timestamp.timestamp().to_le_bytes());
        context.update(b"spacedrive-pairing-challenge-v1");
        
        let tag = context.sign();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(tag.as_ref());
        Ok(hash)
    }
}
```

### 5. Device Information Exchange

#### Secure Key Exchange
```rust
impl PairingConnection {
    /// Exchange device information and public keys
    pub async fn exchange_device_information(
        &mut self,
        local_device: &DeviceInfo,
        local_private_key: &PrivateKey,
    ) -> Result<DeviceInfo> {
        self.state = PairingState::ExchangingKeys;
        
        // 1. Prepare our device information with signature
        let device_message = self.create_signed_device_message(
            local_device,
            local_private_key,
        ).await?;
        
        // 2. Exchange device information simultaneously
        let (send_result, receive_result) = tokio::join!(
            self.send_message(device_message),
            self.receive_message()
        );
        
        send_result?;
        let remote_message = receive_result?;
        
        // 3. Verify remote device information
        match remote_message {
            PairingMessage::DeviceInfo { 
                device_info, 
                public_key, 
                signature 
            } => {
                // Verify signature over device info + public key
                let signed_data = Self::serialize_for_signature(&device_info, &public_key)?;
                if !public_key.verify(&signed_data, &signature) {
                    return Err(NetworkError::AuthenticationFailed(
                        "Invalid device signature".into()
                    ));
                }
                
                // Verify network fingerprint matches computed value
                let expected_fingerprint = NetworkFingerprint::from_device(
                    device_info.device_id, 
                    &public_key
                );
                if device_info.network_fingerprint != expected_fingerprint {
                    return Err(NetworkError::AuthenticationFailed(
                        "Network fingerprint mismatch".into()
                    ));
                }
                
                Ok(device_info)
            }
            _ => Err(NetworkError::ProtocolError("Expected device info message".into())),
        }
    }
    
    async fn create_signed_device_message(
        &self,
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
}
```

### 6. User Confirmation Flow

#### Interactive Confirmation
```rust
pub trait PairingUserInterface {
    /// Ask user to confirm pairing with remote device
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool>;
    
    /// Show pairing progress to user
    async fn show_pairing_progress(&self, state: PairingState);
    
    /// Display pairing error to user
    async fn show_pairing_error(&self, error: &NetworkError);
}

impl PairingConnection {
    /// Handle user confirmation on both sides
    pub async fn handle_user_confirmation(
        &mut self,
        remote_device: &DeviceInfo,
        ui: &dyn PairingUserInterface,
        is_initiator: bool,
    ) -> Result<bool> {
        self.state = PairingState::AwaitingConfirmation;
        
        // Show device info to user and get confirmation
        let user_accepted = ui.confirm_pairing(remote_device).await?;
        
        // Create confirmation message with user's decision
        let confirmation = PairingMessage::PairingConfirmation {
            accepted: user_accepted,
            user_confirmation_signature: self.create_confirmation_signature(
                user_accepted,
                remote_device,
            )?,
        };
        
        if is_initiator {
            // Initiator sends first, then receives
            self.send_message(confirmation).await?;
            let remote_confirmation = self.receive_message().await?;
            self.verify_remote_confirmation(remote_confirmation, user_accepted)
        } else {
            // Joiner receives first, then sends
            let remote_confirmation = self.receive_message().await?;
            self.send_message(confirmation).await?;
            self.verify_remote_confirmation(remote_confirmation, user_accepted)
        }
    }
    
    fn verify_remote_confirmation(
        &self,
        message: PairingMessage,
        local_accepted: bool,
    ) -> Result<bool> {
        match message {
            PairingMessage::PairingConfirmation { 
                accepted: remote_accepted, 
                user_confirmation_signature: _ 
            } => {
                // Both users must accept for pairing to succeed
                Ok(local_accepted && remote_accepted)
            }
            _ => Err(NetworkError::ProtocolError("Expected confirmation message".into())),
        }
    }
}
```

### 7. Session Key Establishment

#### Forward Secrecy Keys
```rust
impl PairingConnection {
    /// Establish session keys for future communication
    pub async fn establish_session_keys(
        &mut self,
        remote_device: &DeviceInfo,
        local_private_key: &PrivateKey,
    ) -> Result<SessionKeys> {
        // Generate ephemeral key pair for forward secrecy
        let ephemeral_private = PrivateKey::generate()?;
        let ephemeral_public = ephemeral_private.public_key();
        
        // Perform Elliptic Curve Diffie-Hellman key exchange
        let shared_secret = self.perform_ecdh_exchange(
            &ephemeral_private,
            &ephemeral_public,
            &remote_device.public_key,
        ).await?;
        
        // Derive session keys using HKDF
        let session_keys = SessionKeys::derive_from_shared_secret(
            &shared_secret,
            &self.local_device.device_id,
            &remote_device.device_id,
        )?;
        
        // Confirm both sides derived the same keys
        self.confirm_session_keys(&session_keys).await?;
        
        self.state = PairingState::Completed;
        Ok(session_keys)
    }
    
    async fn perform_ecdh_exchange(
        &mut self,
        local_ephemeral_private: &PrivateKey,
        local_ephemeral_public: &PublicKey,
        remote_static_public: &PublicKey,
    ) -> Result<[u8; 32]> {
        // Send our ephemeral public key
        let key_exchange = PairingMessage::SessionKeyExchange {
            encrypted_session_key: local_ephemeral_public.as_bytes().to_vec(),
            key_confirmation_hash: [0u8; 32], // Will be filled after ECDH
        };
        
        // Exchange ephemeral public keys
        let (send_result, receive_result) = tokio::join!(
            self.send_message(key_exchange),
            self.receive_message()
        );
        
        send_result?;
        let remote_exchange = receive_result?;
        
        match remote_exchange {
            PairingMessage::SessionKeyExchange { 
                encrypted_session_key: remote_ephemeral_public_bytes,
                key_confirmation_hash: _,
            } => {
                let remote_ephemeral_public = PublicKey::from_bytes(
                    remote_ephemeral_public_bytes
                )?;
                
                // Perform ECDH to get shared secret
                let shared_secret = local_ephemeral_private.ecdh(
                    &remote_ephemeral_public
                )?;
                
                Ok(shared_secret)
            }
            _ => Err(NetworkError::ProtocolError("Expected key exchange message".into())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionKeys {
    /// Key for encrypting outgoing messages
    pub send_key: [u8; 32],
    
    /// Key for decrypting incoming messages  
    pub receive_key: [u8; 32],
    
    /// Key for message authentication codes
    pub mac_key: [u8; 32],
    
    /// Initialization vector for first message
    pub initial_iv: [u8; 12],
}

impl SessionKeys {
    /// Derive session keys using HKDF
    pub fn derive_from_shared_secret(
        shared_secret: &[u8; 32],
        local_device_id: &Uuid,
        remote_device_id: &Uuid,
    ) -> Result<Self> {
        use ring::hkdf;
        
        let salt = b"spacedrive-session-keys-v1";
        let info_base = format!("{}:{}", local_device_id, remote_device_id);
        
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt)
            .extract(shared_secret);
        
        let mut send_key = [0u8; 32];
        let mut receive_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        let mut initial_iv = [0u8; 12];
        
        prk.expand(&[format!("{}-send", info_base).as_bytes()], &mut send_key)?;
        prk.expand(&[format!("{}-receive", info_base).as_bytes()], &mut receive_key)?;
        prk.expand(&[format!("{}-mac", info_base).as_bytes()], &mut mac_key)?;
        prk.expand(&[format!("{}-iv", info_base).as_bytes()], &mut initial_iv)?;
        
        Ok(SessionKeys {
            send_key,
            receive_key,
            mac_key,
            initial_iv,
        })
    }
}
```

### 8. Persistent Storage Integration

#### Enhanced Network Key Storage
```rust
impl NetworkIdentity {
    /// Enhanced persistent storage with proper cryptography
    fn save_network_keys(
        device_id: &Uuid,
        public_key: &PublicKey,
        private_key: &EncryptedPrivateKey,
        paired_devices: &HashMap<Uuid, PairedDeviceInfo>,
        password: &str,
    ) -> Result<()> {
        let path = Self::network_keys_path(device_id)?;
        
        // Create comprehensive key storage
        let key_storage = NetworkKeyStorage {
            version: 1,
            device_id: *device_id,
            public_key: public_key.clone(),
            encrypted_private_key: private_key.clone(),
            paired_devices: paired_devices.clone(),
            session_keys: HashMap::new(), // Ephemeral, not stored
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Encrypt entire storage with master password
        let encrypted_storage = Self::encrypt_key_storage(&key_storage, password)?;
        
        // Atomic write with backup
        Self::atomic_write_with_backup(&path, &encrypted_storage)?;
        
        Ok(())
    }
    
    fn encrypt_key_storage(
        storage: &NetworkKeyStorage,
        password: &str,
    ) -> Result<EncryptedKeyStorage> {
        use ring::{aead, pbkdf2};
        use std::num::NonZeroU32;
        
        // Serialize storage
        let plaintext = serde_json::to_vec(storage)?;
        
        // Generate salt and nonce
        let mut salt = [0u8; 32];
        let mut nonce = [0u8; 12];
        let rng = ring::rand::SystemRandom::new();
        rng.fill(&mut salt)?;
        rng.fill(&mut nonce)?;
        
        // Derive encryption key using PBKDF2
        let iterations = NonZeroU32::new(100_000).unwrap();
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &salt,
            password.as_bytes(),
            &mut key,
        );
        
        // Encrypt with AES-256-GCM
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key)?;
        let sealing_key = aead::LessSafeKey::new(unbound_key);
        
        let mut ciphertext = plaintext;
        sealing_key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::empty(),
            &mut ciphertext,
        )?;
        
        Ok(EncryptedKeyStorage {
            version: 1,
            ciphertext,
            salt,
            nonce,
            iterations: iterations.get(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct NetworkKeyStorage {
    version: u32,
    device_id: Uuid,
    public_key: PublicKey,
    encrypted_private_key: EncryptedPrivateKey,
    paired_devices: HashMap<Uuid, PairedDeviceInfo>,
    session_keys: HashMap<Uuid, SessionKeys>, // Not persisted
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PairedDeviceInfo {
    device_info: DeviceInfo,
    paired_at: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    trust_level: TrustLevel,
    session_history: Vec<SessionRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
enum TrustLevel {
    /// Device was manually paired by user
    Trusted,
    /// Device was paired but user hasn't confirmed recently
    Verified,
    /// Device pairing is suspect or expired
    Untrusted,
}
```

## Security Analysis

### Attack Resistance

#### Man-in-the-Middle (MITM)
- **Protection**: TLS during initial connection + Challenge-response proves both parties know pairing code
- **Additional**: Network fingerprint verification ensures device identity hasn't been tampered with

#### Eavesdropping
- **Protection**: All sensitive data encrypted with TLS + Pairing code never transmitted in plaintext
- **Additional**: Forward secrecy ensures past sessions remain secure even if long-term keys compromised

#### Replay Attacks  
- **Protection**: Timestamps and nonces in all challenge-response messages
- **Additional**: 5-minute expiration on pairing codes

#### Brute Force
- **Protection**: 256-bit pairing codes + Rate limiting + Device lockout after failed attempts
- **Additional**: mDNS discovery prevents passive enumeration

#### Insider Attacks
- **Protection**: User confirmation required on both devices + Visual verification of device names
- **Additional**: Trust levels allow revoking suspicious devices

### Cryptographic Primitives

- **Key Generation**: ring::rand::SystemRandom (cryptographically secure)
- **Symmetric Encryption**: AES-256-GCM (authenticated encryption)
- **Asymmetric Encryption**: Ed25519 (modern elliptic curve cryptography)
- **Key Derivation**: PBKDF2-HMAC-SHA256 (100,000 iterations)
- **Message Authentication**: HMAC-SHA256
- **Hashing**: Blake3 (fast, secure, tree-based hashing)
- **Key Exchange**: X25519 ECDH (elliptic curve Diffie-Hellman)
- **Random Generation**: Hardware-backed entropy where available

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1-2)
- [ ] Enhanced PairingCode with BIP39 wordlist  
- [ ] mDNS broadcasting and discovery
- [ ] Basic secure transport (TLS with ephemeral certs)
- [ ] Challenge-response authentication

### Phase 2: Device Exchange (Week 3)
- [ ] Device information exchange with signatures
- [ ] Network fingerprint verification
- [ ] Basic user confirmation interface
- [ ] Session key establishment with ECDH

### Phase 3: Persistence & Security (Week 4)
- [ ] Enhanced encrypted key storage
- [ ] Proper private key serialization
- [ ] Trust level management
- [ ] Rate limiting and attack prevention

### Phase 4: Integration & Testing (Week 5-6)
- [ ] Integration with existing Network API
- [ ] Comprehensive security testing
- [ ] Error handling and recovery
- [ ] Performance optimization

### Phase 5: Production Hardening (Week 7-8)
- [ ] Audit and fuzzing
- [ ] Documentation and examples  
- [ ] Monitoring and telemetry
- [ ] Deployment and rollout

## Testing Strategy

### Unit Tests
- Cryptographic primitives and key derivation
- Message serialization and protocol handling
- Error conditions and edge cases

### Integration Tests  
- Full pairing flow between two simulated devices
- Network discovery and connection establishment
- Persistence and recovery scenarios

### Security Tests
- Penetration testing of pairing protocol
- Fuzzing of network messages
- Timing attack resistance
- Memory safety verification

### Performance Tests
- Pairing latency and throughput
- Battery usage on mobile devices
- Network efficiency and bandwidth usage

## Dependencies

### New Crates Required
```toml
# BIP39 wordlist support
bip39 = "2.0"

# mDNS service discovery  
mdns = "3.0"

# Additional cryptography
x25519-dalek = "2.0"
hkdf = "0.12"

# TLS support
rustls = "0.21"
rcgen = "0.11"

# Network utilities
if-watch = "3.0"
local-ip-address = "0.5"
```

### Platform Considerations
- **iOS**: Network Extension framework may be required for mDNS
- **Android**: DISCOVER_SERVICE permission needed  
- **Windows**: Windows Firewall configuration for mDNS
- **Linux**: avahi-daemon integration for better mDNS support

## Future Enhancements

### Advanced Features
- **Multi-device pairing**: Chain trust through existing devices
- **QR code pairing**: Visual pairing codes for mobile devices  
- **NFC pairing**: Tap-to-pair on supported devices
- **Cloud-assisted pairing**: Fallback for devices behind strict firewalls
- **Enterprise management**: Centralized device provisioning

### Performance Optimizations  
- **Connection pooling**: Reuse connections for multiple operations
- **Background sync**: Maintain persistent connections between trusted devices
- **Adaptive discovery**: Intelligent scanning based on network topology
- **Caching**: Cache mDNS results and device information

This design provides a comprehensive, secure foundation for Spacedrive's device pairing system while maintaining usability and performance.