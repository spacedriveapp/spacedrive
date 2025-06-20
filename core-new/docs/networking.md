# Spacedrive Networking Module

The Spacedrive networking module provides production-ready device-to-device communication using the libp2p networking stack. It enables secure device pairing, peer discovery, and encrypted communication between Spacedrive instances.

## Overview

The networking module is built on libp2p, the same networking stack used by IPFS, Polkadot, and other production systems. It provides:

- **Global DHT-based discovery** via Kademlia (no more mDNS limitations)
- **Multi-transport support** (TCP + QUIC)
- **NAT traversal and hole punching**
- **Noise Protocol encryption** (replaces TLS)
- **Production-ready networking** that works across networks and the internet
- **Secure device pairing** with BIP39 word-based codes
- **Request-response messaging** over libp2p

## Architecture

```
networking/
├── mod.rs                  # Main module exports and types
├── identity.rs             # Device identity and cryptographic keys
├── manager.rs              # LibP2P manager (legacy - replaced by protocol.rs)
├── behavior.rs             # LibP2P NetworkBehaviour implementation
├── codec.rs                # Message serialization for libp2p
├── discovery.rs            # DHT-based peer discovery
└── pairing/
    ├── mod.rs              # Pairing module exports and core types
    ├── code.rs             # BIP39-based pairing codes
    ├── ui.rs               # User interface abstractions for pairing
    ├── protocol.rs         # Complete libp2p pairing protocol
    └── tests.rs            # Test implementations
```

## Key Components

### 1. Device Identity (`identity.rs`)

Manages cryptographic identities for devices:

```rust
use sd_core_new::infrastructure::networking::{NetworkIdentity, DeviceInfo, PrivateKey};

// Create a network identity for this device
let identity = NetworkIdentity::new_temporary(
    device_id,
    device_name,
    "password"
)?;

// Get device information
let device_info = DeviceInfo::new(device_id, device_name, public_key);
```

**Key Types:**

- `NetworkIdentity` - Complete device identity with encrypted private key
- `DeviceInfo` - Public device information shared during pairing
- `PrivateKey` / `PublicKey` - Ed25519 cryptographic keys
- `NetworkFingerprint` - Unique device fingerprint for verification

### 2. LibP2P Pairing Protocol (`pairing/protocol.rs`)

The main production-ready pairing implementation:

```rust
use sd_core_new::infrastructure::networking::{LibP2PPairingProtocol, PairingCode};

// Create pairing protocol
let mut pairing_protocol = LibP2PPairingProtocol::new(
    &network_identity,
    local_device,
    private_key,
    "password"
).await?;

// Start listening for connections
let listening_addrs = pairing_protocol.start_listening().await?;

// As initiator: generate code and wait for joiner
let (remote_device, session_keys) = pairing_protocol
    .start_as_initiator(&ui).await?;

// As joiner: connect using pairing code
let (remote_device, session_keys) = pairing_protocol
    .start_as_joiner(&ui, pairing_code).await?;
```

### 3. Pairing Codes (`pairing/code.rs`)

BIP39-based 12-word pairing codes for device discovery:

```rust
use sd_core_new::infrastructure::networking::PairingCode;

// Generate a new pairing code
let code = PairingCode::generate()?;
let words = code.as_string(); // "word1 word2 word3 ... word12"

// Parse from user input
let words = ["risk", "cinnamon", "deny", /* ... 9 more words */];
let code = PairingCode::from_words(&words)?;

// Get discovery fingerprint for DHT lookup
let fingerprint = code.discovery_fingerprint;
```

### 4. LibP2P Behavior (`behavior.rs`)

Combines multiple libp2p protocols:

```rust
use sd_core_new::infrastructure::networking::SpacedriveBehaviour;

// The behavior combines:
// - Kademlia DHT for global discovery
// - mDNS for local network discovery
// - Request-response for pairing messages
```

### 5. User Interface Abstraction (`pairing/ui.rs`)

Defines the interface for pairing interactions:

```rust
use sd_core_new::infrastructure::networking::PairingUserInterface;

#[async_trait::async_trait]
impl PairingUserInterface for MyUI {
    async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32);
    async fn prompt_pairing_code(&self) -> Result<[String; 12]>;
    async fn confirm_pairing(&self, remote_device: &DeviceInfo) -> Result<bool>;
    async fn show_pairing_progress(&self, state: PairingState);
    async fn show_pairing_error(&self, error: &NetworkError);
}
```

## Protocol Flow

The device pairing protocol follows this sequence:

### 1. **Initiator Setup**

```
1. Generate BIP39 pairing code (12 words)
2. Start providing code on Kademlia DHT
3. Listen for incoming connections (TCP + QUIC)
4. Display pairing code to user
```

### 2. **Joiner Connection**

```
1. User enters 12-word pairing code
2. Search Kademlia DHT for code providers
3. Connect to discovered initiator
4. Send challenge message
```

### 3. **Authentication Exchange**

```
Joiner  →  Challenge          →  Initiator
Joiner  ←  DeviceInfo         ←  Initiator
Joiner  →  PairingAccepted    →  Initiator
Joiner  ←  Acknowledgment     ←  Initiator
```

### 4. **Session Key Generation**

```
Both devices generate identical session keys using HKDF:
- Input: pairing code discovery fingerprint + device ID
- Output: send_key, receive_key, mac_key (32 bytes each)
```

## Message Types

### Core Pairing Messages

```rust
pub enum PairingMessage {
    Challenge {
        initiator_nonce: [u8; 16],
        timestamp: DateTime<Utc>,
    },
    DeviceInfo {
        device_info: DeviceInfo,
        timestamp: DateTime<Utc>,
    },
    PairingAccepted {
        timestamp: DateTime<Utc>,
    },
    PairingRejected {
        reason: String,
        timestamp: DateTime<Utc>,
    },
}
```

### Session Keys

```rust
pub struct SessionKeys {
    pub send_key: [u8; 32],    // For encrypting outbound messages
    pub receive_key: [u8; 32], // For decrypting inbound messages
    pub mac_key: [u8; 32],     // For message authentication
}
```

## Error Handling

```rust
pub enum NetworkError {
    ConnectionFailed(String),
    DeviceNotFound(uuid::Uuid),
    AuthenticationFailed(String),
    EncryptionError(String),
    TransportError(String),
    ProtocolError(String),
    ConnectionTimeout,
    // ... others
}
```

## Usage Examples

### Complete Pairing Example

See `examples/production_pairing_demo.rs` for a full working example.

### Basic Integration

```rust
use sd_core_new::infrastructure::networking::*;

// 1. Create device identity
let (device_info, private_key) = create_device_identity("My Device").await?;

let network_identity = NetworkIdentity::new_temporary(
    device_info.device_id,
    device_info.device_name.clone(),
    "password"
)?;

// 2. Create pairing protocol
let mut protocol = LibP2PPairingProtocol::new(
    &network_identity,
    device_info,
    private_key,
    "password"
).await?;

// 3. Start listening
let _addrs = protocol.start_listening().await?;

// 4. Pair with another device
let ui = ConsolePairingUI::new("My Device");

// As initiator:
let (remote_device, keys) = protocol.start_as_initiator(&ui).await?;

// As joiner:
// let (remote_device, keys) = protocol.start_as_joiner(&ui, code).await?;

println!("Paired with: {}", remote_device.device_name);
println!("Session keys established");
```

## Implementation Status

### ✅ Completed Features

- **LibP2P Integration**: Full libp2p networking stack
- **BIP39 Pairing Codes**: 12-word codes with proper entropy
- **DHT Discovery**: Global peer discovery via Kademlia
- **Multi-transport**: TCP and QUIC support
- **Noise Encryption**: Secure transport layer
- **Challenge-Response Auth**: Cryptographic authentication
- **Session Key Derivation**: HKDF-based key generation
- **Connection Persistence**: Proper connection lifecycle management
- **Production Demo**: Working end-to-end example

### 🔄 Legacy Components

- **LibP2PManager** (`manager.rs`) - Replaced by `LibP2PPairingProtocol`
- **PairingManager** (`pairing/mod.rs`) - Basic state tracking only

### 🚧 Future Enhancements

- **File Transfer Protocol**: Encrypted file sharing over established sessions
- **Sync Protocol**: Real-time data synchronization between devices
- **Device Authentication**: Long-term device trust and authentication
- **Network Optimization**: Connection pooling and bandwidth management

## Security Considerations

### 1. **Pairing Code Security**

- 12-word BIP39 codes provide ~128 bits of entropy
- Codes expire after 5 minutes by default
- Discovery fingerprint prevents code enumeration attacks

### 2. **Transport Security**

- All communication encrypted with Noise Protocol
- Perfect forward secrecy through ephemeral keys
- Authenticated encryption prevents tampering

### 3. **Session Key Security**

- Keys derived using HKDF with device-specific inputs
- Separate keys for send/receive/MAC operations
- Keys are ephemeral and session-specific

### 4. **Device Authentication**

- Challenge-response prevents replay attacks
- Cryptographic device fingerprints ensure identity
- User confirmation required for all pairings

## Development Workflow

### Running Tests

```bash
# Run networking module tests
cargo test networking

# Run pairing-specific tests
cargo test networking::pairing

# Run with debug logging
RUST_LOG=debug cargo test networking
```

### Development Demo

```bash
# Terminal 1 (Initiator)
cargo run --example networking_pairing_demo
# Choose option 1

# Terminal 2 (Joiner)
cargo run --example networking_pairing_demo
# Choose option 2, enter the 12-word code
```

### Debug Logging

```bash
# Enable detailed libp2p logs
RUST_LOG=libp2p_swarm=debug,sd_core_new::networking=info cargo run

# View only pairing protocol logs
RUST_LOG=sd_core_new::networking::pairing::protocol=debug cargo run
```

## Dependencies

### Core Dependencies

```toml
libp2p = "0.55"           # Networking stack
tokio = "1.0"             # Async runtime
serde = "1.0"             # Serialization
ring = "0.17"             # Cryptography
bip39 = "2.0"             # BIP39 word lists
chrono = "0.4"            # Time handling
tracing = "0.1"           # Logging
```

### LibP2P Protocols Used

- **Kademlia DHT**: Global peer discovery and routing
- **mDNS**: Local network peer discovery
- **Request-Response**: Message exchange protocol
- **Noise**: Transport encryption
- **TCP**: Reliable transport
- **QUIC**: Low-latency transport with built-in encryption

## Migration Notes

### From Legacy Network Module

The original networking implementation has been replaced with the libp2p-based system:

**Old**: mDNS-only discovery, limited to local networks
**New**: Global DHT discovery + mDNS fallback

**Old**: Custom protocols and encryption
**New**: Production-ready libp2p protocols

**Old**: Complex manual connection management
**New**: Automatic connection lifecycle management

### API Changes

- `Network` class replaced by `LibP2PPairingProtocol`
- `SimplePairingProtocol` consolidated into main protocol
- Event-driven architecture with proper async/await support
- Standardized error types and result handling

---

This networking module provides the foundation for all device-to-device communication in Spacedrive, enabling secure pairing, peer discovery, and encrypted data exchange across the internet.
