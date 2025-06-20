# Spacedrop Protocol Design

## Overview

Spacedrop is a cross-platform, AirDrop-like file sharing protocol built on top of Spacedrive's existing libp2p networking infrastructure. Unlike the device pairing system which establishes long-term relationships between owned devices, Spacedrop enables secure, ephemeral file sharing between any two devices with user consent.

## Architecture Principles

### 1. **Ephemeral Security**
- No long-term device relationships required
- Perfect forward secrecy for each file transfer
- Session keys derived per transfer, not per device pairing

### 2. **Proximity-Based Discovery**
- Local network discovery (mDNS) for immediate availability
- DHT fallback for internet-wide discovery when needed
- User-friendly device names and avatars

### 3. **User Consent Model**
- Sender initiates transfer with file metadata
- Receiver explicitly accepts/rejects each transfer
- No automatic file acceptance

## Protocol Design

### Discovery Phase

Instead of 12-word pairing codes, Spacedrop uses:

1. **Broadcast Availability**: Devices advertise their Spacedrop availability on local network
2. **Device Metadata**: Share device name, type, and public key for identification
3. **Proximity Indication**: Show signal strength/network proximity to users

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedropAdvertisement {
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub public_key: PublicKey,
    pub avatar_hash: Option<[u8; 32]>,
    pub timestamp: DateTime<Utc>,
}
```

### File Transfer Protocol

New libp2p protocol: `/spacedrive/spacedrop/1.0.0`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpacedropMessage {
    // Discovery and handshake
    AvailabilityAnnounce {
        advertisement: SpacedropAdvertisement,
    },
    
    // File transfer initiation
    TransferRequest {
        transfer_id: Uuid,
        file_metadata: FileMetadata,
        sender_ephemeral_key: PublicKey,
        timestamp: DateTime<Utc>,
    },
    
    // Receiver responses
    TransferAccepted {
        transfer_id: Uuid,
        receiver_ephemeral_key: PublicKey,
        session_key: [u8; 32], // Derived from ECDH
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
    
    ChunkAcknowledgment {
        transfer_id: Uuid,
        chunk_index: u64,
        received_checksum: [u8; 32],
    },
    
    // Transfer completion
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
```

### Security Model

1. **Device Authentication**: Each device has persistent Ed25519 identity
2. **Ephemeral Key Exchange**: ECDH for each transfer session
3. **File Encryption**: ChaCha20-Poly1305 with derived session keys
4. **Integrity**: Blake3 checksums for chunks and final file
5. **Forward Secrecy**: Ephemeral keys deleted after transfer

```rust
// Key derivation for each transfer
fn derive_transfer_keys(
    sender_ephemeral: &PrivateKey,
    receiver_ephemeral: &PublicKey,
    transfer_id: &Uuid,
) -> TransferKeys {
    let shared_secret = sender_ephemeral.diffie_hellman(receiver_ephemeral);
    let salt = transfer_id.as_bytes();
    
    // HKDF key derivation
    let keys = hkdf::extract_and_expand(&shared_secret, salt, 96);
    
    TransferKeys {
        encryption_key: keys[0..32].try_into().unwrap(),
        auth_key: keys[32..64].try_into().unwrap(),
        chunk_key: keys[64..96].try_into().unwrap(),
    }
}
```

## Implementation Architecture

### Core Components

```
networking/spacedrop/
├── mod.rs                  # Main module exports
├── protocol.rs             # Spacedrop protocol implementation
├── discovery.rs            # Device discovery and advertisement
├── transfer.rs             # File transfer engine
├── encryption.rs           # Encryption/decryption utilities
├── ui.rs                   # User interface abstractions
└── manager.rs              # Overall Spacedrop session management
```

### Integration with Existing System

1. **Reuse LibP2P Infrastructure**: Same swarm, transports, and behavior
2. **Extend NetworkBehaviour**: Add Spacedrop protocol alongside pairing
3. **Share Device Identity**: Use existing device identity system
4. **Independent Sessions**: Spacedrop doesn't interfere with device pairing

```rust
#[derive(NetworkBehaviour)]
pub struct SpacedriveFullBehaviour {
    pub kademlia: KadBehaviour<MemoryStore>,
    pub pairing: RequestResponseBehaviour<PairingCodec>,
    pub spacedrop: RequestResponseBehaviour<SpacedropCodec>,
    pub mdns: mdns::tokio::Behaviour,
}
```

## User Experience Flow

### Sending Files

1. **Discovery**: User sees nearby Spacedrop-enabled devices
2. **Selection**: User selects files and target device
3. **Request**: System sends transfer request with file metadata
4. **Confirmation**: Wait for receiver acceptance
5. **Transfer**: Stream encrypted file chunks with progress
6. **Completion**: Verify transfer integrity and cleanup

### Receiving Files

1. **Notification**: "Device 'MacBook Pro' wants to send you 'presentation.pdf' (2.5 MB)"
2. **Preview**: Show file name, size, type, sender device
3. **Decision**: Accept/Decline with optional save location
4. **Transfer**: Show progress bar with speed/ETA
5. **Completion**: File saved, transfer cleanup

## Security Considerations

### Threat Model

1. **Network Attackers**: Cannot decrypt files (E2E encryption)
2. **Malicious Senders**: Receiver must explicitly accept each file
3. **File Integrity**: Blake3 checksums prevent tampering
4. **Replay Attacks**: Timestamp validation and unique transfer IDs
5. **DoS Attacks**: Rate limiting and size limits

### Privacy Protections

1. **Device Anonymity**: Only share device names, not personal info
2. **Network Isolation**: Local network discovery preferred
3. **Metadata Minimal**: Only essential file metadata shared
4. **Ephemeral**: No transfer history stored permanently

## Implementation Plan

### Phase 1: Core Protocol (Weeks 1-2)
- [ ] Implement SpacedropMessage types and serialization
- [ ] Create SpacedropCodec for libp2p communication
- [ ] Build basic discovery mechanism with mDNS
- [ ] Implement ephemeral key exchange (ECDH)

### Phase 2: File Transfer Engine (Weeks 3-4)
- [ ] Chunked file streaming with flow control
- [ ] ChaCha20-Poly1305 encryption/decryption
- [ ] Blake3 integrity checking
- [ ] Progress tracking and error handling

### Phase 3: Integration (Week 5)
- [ ] Extend existing NetworkBehaviour
- [ ] Create SpacedropManager for session management
- [ ] Implement UI abstraction layer
- [ ] Add configuration and preferences

### Phase 4: Security & Testing (Week 6)
- [ ] Security audit of crypto implementation
- [ ] Comprehensive test suite
- [ ] Performance testing with large files
- [ ] Cross-platform compatibility testing

### Phase 5: User Experience (Week 7)
- [ ] Native UI integration points
- [ ] File type icons and previews
- [ ] Device avatar system
- [ ] Transfer history and statistics

## Performance Considerations

### Optimization Strategies

1. **Parallel Transfers**: Multiple chunks in flight
2. **Adaptive Chunking**: Larger chunks for large files
3. **Compression**: Optional compression for text files
4. **Bandwidth Management**: QoS integration with other network traffic

### Scalability Limits

- **File Size**: Up to 100GB per transfer (configurable)
- **Concurrent Transfers**: 5 active transfers per device
- **Network Usage**: Respect system bandwidth limits
- **Storage**: Temporary storage for partial transfers

## Deployment Strategy

### Backwards Compatibility

- Graceful degradation when Spacedrop not available
- Version negotiation in protocol handshake
- Feature flags for gradual rollout

### Platform Support

- All platforms supported by libp2p (Windows, macOS, Linux, iOS, Android)
- Native file picker integration
- Platform-specific optimizations (iOS file provider, Android SAF)

## Future Extensions

### Advanced Features

1. **Multi-File Transfers**: Folders and file collections
2. **Resume Capability**: Pause/resume large transfers
3. **QR Code Sharing**: QR codes for cross-network discovery
4. **Bandwidth Scheduling**: Time-based transfer windows
5. **Cloud Relay**: Relay service for NAT traversal

### Integration Opportunities

1. **Spacedrive Sync**: Use Spacedrop for initial sync bootstrap
2. **Library Sharing**: Share library items between devices
3. **Collaborative Features**: Real-time document collaboration
4. **Backup Integration**: Automated backup to nearby devices

---

This design provides a secure, user-friendly file sharing experience while leveraging Spacedrive's existing networking infrastructure. The ephemeral nature ensures privacy while the libp2p foundation provides production-ready networking capabilities.