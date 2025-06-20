# Spacedrop Implementation Plan

## Implementation Overview

This document outlines the step-by-step implementation of Spacedrop on top of Spacedrive's existing libp2p networking infrastructure. The implementation reuses as much existing code as possible while adding the new file sharing capabilities.

## File Structure

```
core-new/src/networking/
├── mod.rs                          # Add spacedrop exports
├── behavior.rs                     # Extend with spacedrop protocol
├── spacedrop/                      # New spacedrop module
│   ├── mod.rs                      # Module exports and main types
│   ├── protocol.rs                 # Core spacedrop protocol
│   ├── messages.rs                 # Message types and serialization
│   ├── codec.rs                    # LibP2P codec for spacedrop
│   ├── discovery.rs                # Device discovery and advertisement
│   ├── transfer.rs                 # File transfer engine
│   ├── encryption.rs               # Crypto utilities
│   ├── manager.rs                  # Session management
│   ├── ui.rs                       # UI abstraction
│   └── config.rs                   # Configuration types
└── examples/
    └── spacedrop_demo.rs           # Demo application
```

## Phase 1: Foundation (Week 1)

### 1.1 Create Module Structure

**File**: `core-new/src/networking/spacedrop/mod.rs`

```rust
//! Spacedrop: Cross-platform file sharing protocol
//!
//! Built on libp2p for secure, ephemeral file transfers between devices.

pub mod messages;
pub mod codec;
pub mod protocol;
pub mod discovery;
pub mod transfer;
pub mod encryption;
pub mod manager;
pub mod ui;
pub mod config;

// Re-exports
pub use messages::*;
pub use codec::SpacedropCodec;
pub use protocol::SpacedropProtocol;
pub use discovery::SpacedropDiscovery;
pub use transfer::{FileTransfer, TransferProgress};
pub use manager::SpacedropManager;
pub use ui::SpacedropUserInterface;
pub use config::SpacedropConfig;

use uuid::Uuid;
use std::collections::HashMap;

pub type TransferId = Uuid;
pub type DeviceId = Uuid;
```

### 1.2 Define Message Types

**File**: `core-new/src/networking/spacedrop/messages.rs`

```rust
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::networking::{DeviceInfo, PublicKey};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpacedropMessage {
    // Discovery
    AvailabilityAnnounce {
        device_info: DeviceInfo,
        capabilities: SpacedropCapabilities,
        timestamp: DateTime<Utc>,
    },

    // Transfer initiation
    TransferRequest {
        transfer_id: Uuid,
        file_metadata: FileMetadata,
        sender_ephemeral_key: PublicKey,
        timestamp: DateTime<Utc>,
    },

    // Response messages
    TransferAccepted {
        transfer_id: Uuid,
        receiver_ephemeral_key: PublicKey,
        timestamp: DateTime<Utc>,
    },

    TransferRejected {
        transfer_id: Uuid,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // File streaming
    FileChunk {
        transfer_id: Uuid,
        chunk_index: u64,
        chunk_data: Vec<u8>,
        is_final: bool,
        checksum: [u8; 32],
    },

    ChunkAck {
        transfer_id: Uuid,
        chunk_index: u64,
        status: ChunkStatus,
    },

    // Completion
    TransferComplete {
        transfer_id: Uuid,
        final_checksum: [u8; 32],
        timestamp: DateTime<Utc>,
    },

    TransferError {
        transfer_id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub checksum: [u8; 32],
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedropCapabilities {
    pub max_file_size: u64,
    pub supported_protocols: Vec<String>,
    pub encryption_methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkStatus {
    Received,
    CorruptedRetry,
    Error(String),
}
```

### 1.3 Implement LibP2P Codec

**File**: `core-new/src/networking/spacedrop/codec.rs`

```rust
use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;
use std::io;
use super::messages::SpacedropMessage;

#[derive(Debug, Clone, Default)]
pub struct SpacedropCodec;

#[async_trait]
impl Codec for SpacedropCodec {
    type Protocol = StreamProtocol;
    type Request = SpacedropMessage;
    type Response = SpacedropMessage;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read length-prefixed message
        let mut len_buf = [0u8; 4];
        AsyncReadExt::read_exact(io, &mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Protect against DoS
        if len > 100 * 1024 * 1024 { // 100MB max message
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }

        let mut buf = vec![0u8; len];
        AsyncReadExt::read_exact(io, &mut buf).await?;

        // Deserialize with bincode for efficiency
        bincode::deserialize(&buf).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Deserialization error: {}", e))
        })
    }

    async fn read_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        self.read_request(protocol, io).await
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = bincode::serialize(&req).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Serialization error: {}", e))
        })?;

        let len = data.len() as u32;
        AsyncWriteExt::write_all(io, &len.to_be_bytes()).await?;
        AsyncWriteExt::write_all(io, &data).await?;
        AsyncWriteExt::flush(io).await?;

        Ok(())
    }

    async fn write_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T, res: Self::Response) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.write_request(protocol, io, res).await
    }
}
```

## Phase 2: Core Protocol (Week 2)

### 2.1 Extend NetworkBehaviour

**File**: `core-new/src/networking/behavior.rs` (modify existing)

```rust
use libp2p::request_response::{
    Behaviour as RequestResponseBehaviour, Config as RequestResponseConfig, ProtocolSupport,
};
use super::spacedrop::SpacedropCodec;

#[derive(NetworkBehaviour)]
pub struct SpacedriveBehaviour {
    pub kademlia: KadBehaviour<MemoryStore>,
    pub pairing: RequestResponseBehaviour<PairingCodec>,
    pub spacedrop: RequestResponseBehaviour<SpacedropCodec>, // New protocol
    pub mdns: mdns::tokio::Behaviour,
}

impl SpacedriveBehaviour {
    pub fn new(peer_id: PeerId) -> Result<Self, Box<dyn std::error::Error>> {
        // ... existing code ...

        // Add Spacedrop protocol
        let spacedrop_protocols = std::iter::once((
            StreamProtocol::new("/spacedrive/spacedrop/1.0.0"),
            ProtocolSupport::Full,
        ));
        let spacedrop_cfg = RequestResponseConfig::default();
        let spacedrop = RequestResponseBehaviour::with_codec(
            SpacedropCodec::default(),
            spacedrop_protocols,
            spacedrop_cfg
        );

        Ok(Self {
            kademlia,
            pairing,
            spacedrop, // Add this
            mdns,
        })
    }
}
```

### 2.2 Implement Discovery

**File**: `core-new/src/networking/spacedrop/discovery.rs`

```rust
use libp2p::{PeerId, Multiaddr};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, debug};
use crate::networking::{DeviceInfo, Result};
use super::messages::{SpacedropMessage, SpacedropCapabilities};

pub struct SpacedropDiscovery {
    discovered_devices: HashMap<PeerId, DiscoveredDevice>,
    event_sender: mpsc::UnboundedSender<SpacedropEvent>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub peer_id: PeerId,
    pub device_info: DeviceInfo,
    pub capabilities: SpacedropCapabilities,
    pub addresses: Vec<Multiaddr>,
    pub last_seen: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum SpacedropEvent {
    DeviceDiscovered(DiscoveredDevice),
    DeviceLost(PeerId),
    TransferRequested {
        from: PeerId,
        transfer_id: uuid::Uuid,
        metadata: super::messages::FileMetadata,
    },
}

impl SpacedropDiscovery {
    pub fn new(event_sender: mpsc::UnboundedSender<SpacedropEvent>) -> Self {
        Self {
            discovered_devices: HashMap::new(),
            event_sender,
        }
    }

    pub fn handle_discovery_message(&mut self, peer_id: PeerId, message: SpacedropMessage) -> Result<()> {
        match message {
            SpacedropMessage::AvailabilityAnnounce { device_info, capabilities, .. } => {
                let device = DiscoveredDevice {
                    peer_id,
                    device_info,
                    capabilities,
                    addresses: Vec::new(), // Will be populated from libp2p events
                    last_seen: std::time::Instant::now(),
                };

                self.discovered_devices.insert(peer_id, device.clone());
                let _ = self.event_sender.send(SpacedropEvent::DeviceDiscovered(device));
            }
            _ => {}
        }
        Ok(())
    }

    pub fn get_discovered_devices(&self) -> Vec<&DiscoveredDevice> {
        self.discovered_devices.values().collect()
    }

    pub fn cleanup_stale_devices(&mut self) {
        let now = std::time::Instant::now();
        let stale_threshold = std::time::Duration::from_secs(60); // 1 minute

        let stale_peers: Vec<PeerId> = self.discovered_devices
            .iter()
            .filter(|(_, device)| now.duration_since(device.last_seen) > stale_threshold)
            .map(|(peer_id, _)| *peer_id)
            .collect();

        for peer_id in stale_peers {
            self.discovered_devices.remove(&peer_id);
            let _ = self.event_sender.send(SpacedropEvent::DeviceLost(peer_id));
        }
    }
}
```

## Phase 3: File Transfer Engine (Week 3)

### 3.1 Encryption Utilities

**File**: `core-new/src/networking/spacedrop/encryption.rs`

```rust
use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    digest, hkdf, rand::{self, SecureRandom},
};
use crate::networking::{PrivateKey, PublicKey, Result, NetworkError};
use uuid::Uuid;

pub struct TransferEncryption {
    encryption_key: LessSafeKey,
    auth_key: [u8; 32],
    nonce_counter: u64,
}

impl TransferEncryption {
    pub fn new_from_ecdh(
        local_private: &PrivateKey,
        remote_public: &PublicKey,
        transfer_id: &Uuid,
    ) -> Result<Self> {
        // Perform ECDH
        let shared_secret = local_private.diffie_hellman(remote_public)
            .map_err(|e| NetworkError::EncryptionError(format!("ECDH failed: {}", e)))?;

        // Derive keys using HKDF
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, transfer_id.as_bytes());
        let prk = salt.extract(&shared_secret);

        let mut key_material = [0u8; 64]; // 32 bytes for ChaCha20Poly1305 + 32 bytes for auth
        prk.expand(&[b"spacedrop-file-transfer"], hkdf::HKDF_SHA256)
            .map_err(|_| NetworkError::EncryptionError("Key derivation failed".to_string()))?
            .fill(&mut key_material)
            .map_err(|_| NetworkError::EncryptionError("Key derivation failed".to_string()))?;

        let encryption_key = LessSafeKey::new(
            UnboundKey::new(&aead::CHACHA20_POLY1305, &key_material[0..32])
                .map_err(|_| NetworkError::EncryptionError("Failed to create encryption key".to_string()))?
        );

        let mut auth_key = [0u8; 32];
        auth_key.copy_from_slice(&key_material[32..64]);

        Ok(Self {
            encryption_key,
            auth_key,
            nonce_counter: 0,
        })
    }

    pub fn encrypt_chunk(&mut self, chunk_data: &[u8]) -> Result<Vec<u8>> {
        let nonce = self.get_next_nonce()?;
        let nonce_obj = Nonce::assume_unique_for_key(nonce);

        let mut in_out = chunk_data.to_vec();
        self.encryption_key
            .seal_in_place_append_tag(nonce_obj, Aad::empty(), &mut in_out)
            .map_err(|_| NetworkError::EncryptionError("Encryption failed".to_string()))?;

        // Prepend nonce to encrypted data
        let mut result = nonce.to_vec();
        result.extend_from_slice(&in_out);
        Ok(result)
    }

    pub fn decrypt_chunk(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            return Err(NetworkError::EncryptionError("Invalid encrypted data".to_string()));
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::try_assume_unique_for_key(nonce_bytes)
            .map_err(|_| NetworkError::EncryptionError("Invalid nonce".to_string()))?;

        let mut in_out = ciphertext.to_vec();
        self.encryption_key
            .open_in_place(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| NetworkError::EncryptionError("Decryption failed".to_string()))?;

        Ok(in_out)
    }

    fn get_next_nonce(&mut self) -> Result<[u8; 12]> {
        let mut nonce = [0u8; 12];
        nonce[0..8].copy_from_slice(&self.nonce_counter.to_le_bytes());
        self.nonce_counter += 1;
        Ok(nonce)
    }
}

pub fn calculate_file_checksum(data: &[u8]) -> [u8; 32] {
    let digest = digest::digest(&digest::SHA256, data);
    let mut result = [0u8; 32];
    result.copy_from_slice(digest.as_ref());
    result
}

pub fn calculate_chunk_checksum(data: &[u8]) -> [u8; 32] {
    calculate_file_checksum(data) // Same algorithm for consistency
}
```

### 3.2 File Transfer Engine

**File**: `core-new/src/networking/spacedrop/transfer.rs`

```rust
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use crate::networking::{Result, NetworkError};
use super::messages::{FileMetadata, SpacedropMessage};
use super::encryption::{TransferEncryption, calculate_file_checksum, calculate_chunk_checksum};

const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transfer_id: Uuid,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub chunks_completed: u64,
    pub total_chunks: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
}

pub struct FileTransfer {
    pub transfer_id: Uuid,
    pub metadata: FileMetadata,
    pub encryption: Option<TransferEncryption>,
    pub file_handle: Option<File>,
    pub progress: TransferProgress,
    pub is_sender: bool,
    start_time: std::time::Instant,
}

impl FileTransfer {
    pub fn new_sender(
        transfer_id: Uuid,
        file_path: &Path,
        encryption: TransferEncryption,
    ) -> Result<Self> {
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| NetworkError::IoError(e))?;

        let file_metadata = FileMetadata {
            name: file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            size: metadata.len(),
            mime_type: mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string(),
            checksum: [0; 32], // Will be calculated during transfer
            created: metadata.created().ok().map(|t| t.into()),
            modified: metadata.modified().ok().map(|t| t.into()),
        };

        let total_chunks = (metadata.len() + CHUNK_SIZE as u64 - 1) / CHUNK_SIZE as u64;

        Ok(Self {
            transfer_id,
            metadata: file_metadata,
            encryption: Some(encryption),
            file_handle: None,
            progress: TransferProgress {
                transfer_id,
                bytes_transferred: 0,
                total_bytes: metadata.len(),
                chunks_completed: 0,
                total_chunks,
                speed_bps: 0,
                eta_seconds: None,
            },
            is_sender: true,
            start_time: std::time::Instant::now(),
        })
    }

    pub fn new_receiver(
        transfer_id: Uuid,
        metadata: FileMetadata,
        save_path: &Path,
        encryption: TransferEncryption,
    ) -> Result<Self> {
        let total_chunks = (metadata.size + CHUNK_SIZE as u64 - 1) / CHUNK_SIZE as u64;

        Ok(Self {
            transfer_id,
            metadata,
            encryption: Some(encryption),
            file_handle: None,
            progress: TransferProgress {
                transfer_id,
                bytes_transferred: 0,
                total_bytes: metadata.size,
                chunks_completed: 0,
                total_chunks,
                speed_bps: 0,
                eta_seconds: None,
            },
            is_sender: false,
            start_time: std::time::Instant::now(),
        })
    }

    pub async fn open_file(&mut self, file_path: &Path) -> Result<()> {
        let file = if self.is_sender {
            File::open(file_path).await?
        } else {
            File::create(file_path).await?
        };

        self.file_handle = Some(file);
        Ok(())
    }

    pub async fn read_next_chunk(&mut self) -> Result<Option<SpacedropMessage>> {
        if !self.is_sender {
            return Err(NetworkError::ProtocolError("Receiver cannot read chunks".to_string()));
        }

        let file = self.file_handle.as_mut()
            .ok_or_else(|| NetworkError::ProtocolError("File not opened".to_string()))?;

        let mut buffer = vec![0u8; CHUNK_SIZE];
        let bytes_read = file.read(&mut buffer).await?;

        if bytes_read == 0 {
            return Ok(None); // End of file
        }

        buffer.truncate(bytes_read);
        let chunk_checksum = calculate_chunk_checksum(&buffer);

        // Encrypt the chunk
        let encrypted_data = if let Some(ref mut encryption) = self.encryption {
            encryption.encrypt_chunk(&buffer)?
        } else {
            buffer
        };

        let is_final = self.progress.bytes_transferred + bytes_read as u64 >= self.progress.total_bytes;

        Ok(Some(SpacedropMessage::FileChunk {
            transfer_id: self.transfer_id,
            chunk_index: self.progress.chunks_completed,
            chunk_data: encrypted_data,
            is_final,
            checksum: chunk_checksum,
        }))
    }

    pub async fn write_chunk(&mut self, chunk: &[u8], expected_checksum: [u8; 32]) -> Result<()> {
        if self.is_sender {
            return Err(NetworkError::ProtocolError("Sender cannot write chunks".to_string()));
        }

        // Decrypt the chunk
        let decrypted_data = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_chunk(chunk)?
        } else {
            chunk.to_vec()
        };

        // Verify checksum
        let actual_checksum = calculate_chunk_checksum(&decrypted_data);
        if actual_checksum != expected_checksum {
            return Err(NetworkError::ProtocolError("Chunk checksum mismatch".to_string()));
        }

        // Write to file
        let file = self.file_handle.as_mut()
            .ok_or_else(|| NetworkError::ProtocolError("File not opened".to_string()))?;

        file.write_all(&decrypted_data).await?;

        self.progress.bytes_transferred += decrypted_data.len() as u64;
        self.progress.chunks_completed += 1;
        self.update_progress_stats();

        Ok(())
    }

    fn update_progress_stats(&mut self) {
        let elapsed = self.start_time.elapsed();
        if elapsed.as_secs() > 0 {
            self.progress.speed_bps = self.progress.bytes_transferred / elapsed.as_secs();

            if self.progress.speed_bps > 0 {
                let remaining_bytes = self.progress.total_bytes - self.progress.bytes_transferred;
                self.progress.eta_seconds = Some(remaining_bytes / self.progress.speed_bps);
            }
        }
    }

    pub fn progress(&self) -> &TransferProgress {
        &self.progress
    }

    pub fn is_complete(&self) -> bool {
        self.progress.bytes_transferred >= self.progress.total_bytes
    }
}
```

## Phase 4: Integration (Week 4)

### 4.1 Session Manager

**File**: `core-new/src/networking/spacedrop/manager.rs`

```rust
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc;
use libp2p::PeerId;
use uuid::Uuid;
use tracing::{info, error, debug};

use crate::networking::{DeviceInfo, PrivateKey, Result, NetworkError};
use super::{
    messages::{SpacedropMessage, FileMetadata},
    transfer::{FileTransfer, TransferProgress},
    discovery::{SpacedropDiscovery, SpacedropEvent, DiscoveredDevice},
    encryption::TransferEncryption,
    ui::SpacedropUserInterface,
};

pub struct SpacedropManager {
    local_device: DeviceInfo,
    local_private_key: PrivateKey,
    discovery: SpacedropDiscovery,
    active_transfers: HashMap<Uuid, FileTransfer>,
    event_receiver: mpsc::UnboundedReceiver<SpacedropEvent>,
}

impl SpacedropManager {
    pub fn new(
        local_device: DeviceInfo,
        local_private_key: PrivateKey,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let discovery = SpacedropDiscovery::new(event_sender);

        Self {
            local_device,
            local_private_key,
            discovery,
            active_transfers: HashMap::new(),
            event_receiver,
        }
    }

    pub async fn send_file_to_device(
        &mut self,
        target_device: &DiscoveredDevice,
        file_path: &Path,
        ui: &dyn SpacedropUserInterface,
    ) -> Result<Uuid> {
        let transfer_id = Uuid::new_v4();

        // Show transfer initiation in UI
        ui.show_transfer_initiated(transfer_id, &target_device.device_info).await;

        // Create ephemeral key pair for this transfer
        let ephemeral_private = PrivateKey::generate();
        let ephemeral_public = ephemeral_private.public_key();

        // Create transfer encryption (we'll complete this after handshake)
        let metadata = FileMetadata {
            name: file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            size: std::fs::metadata(file_path)?.len(),
            mime_type: mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string(),
            checksum: [0; 32], // Will be calculated
            created: None,
            modified: None,
        };

        // Send transfer request
        let request = SpacedropMessage::TransferRequest {
            transfer_id,
            file_metadata: metadata.clone(),
            sender_ephemeral_key: ephemeral_public,
            timestamp: chrono::Utc::now(),
        };

        // TODO: Send message via libp2p to target_device.peer_id
        // This would be handled by the protocol layer

        info!("Initiated file transfer {} to device {}", transfer_id, target_device.device_info.device_name);
        Ok(transfer_id)
    }

    pub async fn handle_transfer_request(
        &mut self,
        from_peer: PeerId,
        transfer_id: Uuid,
        metadata: FileMetadata,
        sender_ephemeral_key: crate::networking::PublicKey,
        ui: &dyn SpacedropUserInterface,
    ) -> Result<()> {
        // Find the device that sent this request
        let sender_device = self.discovery.get_discovered_devices()
            .iter()
            .find(|d| d.peer_id == from_peer)
            .ok_or_else(|| NetworkError::DeviceNotFound(from_peer.into()))?;

        // Ask user for consent
        let accepted = ui.prompt_accept_transfer(
            &sender_device.device_info,
            &metadata,
        ).await?;

        if accepted {
            // Generate ephemeral key for this transfer
            let ephemeral_private = PrivateKey::generate();
            let ephemeral_public = ephemeral_private.public_key();

            // Create encryption for this transfer
            let encryption = TransferEncryption::new_from_ecdh(
                &ephemeral_private,
                &sender_ephemeral_key,
                &transfer_id,
            )?;

            // Get save location from user
            let save_path = ui.get_save_location(&metadata).await?;

            // Create receiver transfer
            let mut transfer = FileTransfer::new_receiver(
                transfer_id,
                metadata,
                &save_path,
                encryption,
            )?;

            transfer.open_file(&save_path).await?;
            self.active_transfers.insert(transfer_id, transfer);

            // Send acceptance
            let response = SpacedropMessage::TransferAccepted {
                transfer_id,
                receiver_ephemeral_key: ephemeral_public,
                timestamp: chrono::Utc::now(),
            };

            // TODO: Send response via libp2p
            info!("Accepted transfer {} from {}", transfer_id, sender_device.device_info.device_name);
        } else {
            // Send rejection
            let response = SpacedropMessage::TransferRejected {
                transfer_id,
                reason: Some("User declined".to_string()),
                timestamp: chrono::Utc::now(),
            };

            // TODO: Send response via libp2p
            info!("Rejected transfer {} from {}", transfer_id, sender_device.device_info.device_name);
        }

        Ok(())
    }

    pub fn get_active_transfers(&self) -> Vec<&TransferProgress> {
        self.active_transfers.values().map(|t| t.progress()).collect()
    }

    pub fn get_discovered_devices(&self) -> Vec<&DiscoveredDevice> {
        self.discovery.get_discovered_devices()
    }

    pub async fn cleanup_stale_connections(&mut self) {
        self.discovery.cleanup_stale_devices();

        // Clean up completed transfers
        let completed_transfers: Vec<Uuid> = self.active_transfers
            .iter()
            .filter(|(_, transfer)| transfer.is_complete())
            .map(|(id, _)| *id)
            .collect();

        for transfer_id in completed_transfers {
            self.active_transfers.remove(&transfer_id);
            info!("Cleaned up completed transfer {}", transfer_id);
        }
    }
}
```

## Phase 5: User Interface & Demo (Week 5)

### 5.1 UI Abstraction

**File**: `core-new/src/networking/spacedrop/ui.rs`

```rust
use async_trait::async_trait;
use std::path::PathBuf;
use crate::networking::{DeviceInfo, Result};
use super::messages::FileMetadata;
use uuid::Uuid;

#[async_trait]
pub trait SpacedropUserInterface: Send + Sync {
    async fn show_transfer_initiated(&self, transfer_id: Uuid, target_device: &DeviceInfo);

    async fn prompt_accept_transfer(
        &self,
        sender_device: &DeviceInfo,
        file_metadata: &FileMetadata
    ) -> Result<bool>;

    async fn get_save_location(&self, file_metadata: &FileMetadata) -> Result<PathBuf>;

    async fn show_transfer_progress(&self, progress: &super::transfer::TransferProgress);

    async fn show_transfer_complete(&self, transfer_id: Uuid, success: bool);

    async fn show_error(&self, error: &str);
}

// Console implementation for testing
pub struct ConsoleSpacedropUI;

#[async_trait]
impl SpacedropUserInterface for ConsoleSpacedropUI {
    async fn show_transfer_initiated(&self, transfer_id: Uuid, target_device: &DeviceInfo) {
        println!("🚀 Initiating file transfer {} to {}", transfer_id, target_device.device_name);
    }

    async fn prompt_accept_transfer(
        &self,
        sender_device: &DeviceInfo,
        file_metadata: &FileMetadata
    ) -> Result<bool> {
        println!("📤 {} wants to send you:", sender_device.device_name);
        println!("   📄 {} ({} bytes)", file_metadata.name, file_metadata.size);
        println!("   📁 Type: {}", file_metadata.mime_type);

        print!("Accept transfer? (y/n): ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        Ok(input.trim().to_lowercase().starts_with('y'))
    }

    async fn get_save_location(&self, file_metadata: &FileMetadata) -> Result<PathBuf> {
        // For demo, save to current directory
        Ok(PathBuf::from(&file_metadata.name))
    }

    async fn show_transfer_progress(&self, progress: &super::transfer::TransferProgress) {
        let percent = (progress.bytes_transferred as f64 / progress.total_bytes as f64) * 100.0;
        println!("📊 Transfer {}: {:.1}% ({}/{} bytes)",
                 progress.transfer_id, percent, progress.bytes_transferred, progress.total_bytes);
    }

    async fn show_transfer_complete(&self, transfer_id: Uuid, success: bool) {
        if success {
            println!("✅ Transfer {} completed successfully", transfer_id);
        } else {
            println!("❌ Transfer {} failed", transfer_id);
        }
    }

    async fn show_error(&self, error: &str) {
        println!("❌ Error: {}", error);
    }
}
```

### 5.2 Demo Application

**File**: `core-new/examples/spacedrop_demo.rs`

```rust
use tokio;
use tracing::{info, Level};
use tracing_subscriber;
use std::path::PathBuf;

use sd_core_new::infrastructure::networking::{
    create_device_identity, NetworkIdentity,
    spacedrop::{SpacedropManager, ConsoleSpacedropUI},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    println!("🚀 Spacedrop Demo");
    println!("=================");

    // Create device identity
    let (device_info, private_key) = create_device_identity("Demo Device").await?;
    println!("📱 Device: {} ({})", device_info.device_name, device_info.device_id);

    // Create Spacedrop manager
    let mut manager = SpacedropManager::new(device_info, private_key);
    let ui = ConsoleSpacedropUI;

    println!("\n🔍 Scanning for nearby devices...");

    // In a real implementation, this would:
    // 1. Start discovery
    // 2. Handle incoming requests
    // 3. Show discovered devices
    // 4. Allow user to select files and targets

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Cleanup stale connections
        manager.cleanup_stale_connections().await;

        // Show discovered devices
        let devices = manager.get_discovered_devices();
        if !devices.is_empty() {
            println!("📡 Discovered devices:");
            for device in devices {
                println!("   • {} ({})", device.device_info.device_name, device.peer_id);
            }
        }

        // Show active transfers
        let transfers = manager.get_active_transfers();
        for progress in transfers {
            ui.show_transfer_progress(progress).await;
        }
    }
}
```

## Integration Points

### Update Main Module

**File**: `core-new/src/networking/mod.rs` (modify)

```rust
// Add spacedrop module
pub mod spacedrop;

// Add exports
pub use spacedrop::{
    SpacedropManager, SpacedropUserInterface, ConsoleSpacedropUI,
    SpacedropMessage, FileMetadata, TransferProgress,
};
```

### Update Cargo.toml

Add required dependencies:

```toml
[dependencies]
# ... existing dependencies ...
bincode = "1.3"
mime_guess = "2.0"
ring = "0.17"
blake3 = "1.5"
```

## Testing Strategy

### Unit Tests

- Message serialization/deserialization
- Encryption/decryption utilities
- File chunking and checksums
- Discovery logic

### Integration Tests

- End-to-end file transfer
- Error handling and recovery
- Large file transfers
- Multiple concurrent transfers

### Performance Tests

- Transfer speed benchmarks
- Memory usage during transfers
- Network efficiency measurements

## Security Validation

### Cryptographic Review

- ECDH key exchange implementation
- ChaCha20-Poly1305 usage
- Key derivation (HKDF)
- Nonce handling

### Attack Vector Analysis

- DoS protection (message size limits)
- Replay attack prevention
- Man-in-the-middle resistance
- File integrity validation

This implementation plan provides a complete roadmap for building Spacedrop on top of the existing networking infrastructure while maintaining security and performance standards.
