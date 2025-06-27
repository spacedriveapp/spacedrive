# Spacedrive v2 Networking Module: Unified and Simplified

The Spacedrive v2 networking module provides robust device-to-device communication using a modern, unified libp2p-based architecture. It enables secure device pairing, peer discovery, and encrypted data transfer between Spacedrive instances, forming the backbone of the Virtual Distributed File System (VDFS).

This new implementation refactors the original concept into a simpler, more integrated system built around a **single, unified `libp2p` Swarm**. This approach eliminates the complexity of multiple competing swarm instances and provides a clear, centralized model for all network operations.

## Overview

The networking module is tightly integrated into the `Core` struct and provides:

- **Unified Discovery**: Simultaneous local discovery via **mDNS** and global discovery via a **Kademlia DHT**.
- **Simplified Transport**: A stable and reliable transport stack using **TCP**, encrypted with the **Noise Protocol**, and multiplexed with **Yamux**.
- **Centralized State Management**: A single `DeviceRegistry` tracks the state of all known peers, from discovered to paired and connected.
- **Extensible Protocol System**: A modular `ProtocolRegistry` allows for clean separation of concerns, routing incoming messages to the correct handler (e.g., Pairing, File Transfer).
- **Secure Device Pairing**: A robust, challenge-response pairing protocol secured with cryptographic signatures and initiated with user-friendly BIP39 word codes.
- **End-to-End Encrypted File Transfer**: High-level APIs for sharing files between devices, built on an underlying protocol that handles chunking, encryption, and verification.

## Architecture

The networking architecture is designed for simplicity and direct integration with the `Core` application logic. All network operations are managed by a single `NetworkingCore` instance, which is initialized and managed by the main `Core` struct.

```
+---------------------------------------------+
|                Core (`lib.rs`)              |
| - init_networking()                         |
| - start_pairing_as_initiator()              |
| - share_with_device()                       |
+---------------------------------------------+
                   |
                   v
+---------------------------------------------+
|    NetworkingCore (`core/mod.rs`)           |
| - Swarm<UnifiedBehaviour>                   |
| - DeviceRegistry (Tracks all devices)       |
| - ProtocolRegistry (Routes messages)        |
+---------------------------------------------+
                   |
                   v
+---------------------------------------------+
|  UnifiedBehaviour (`core/behavior.rs`)      |
| - Kademlia (DHT)                            |
| - mDNS (Local Discovery)                    |
| - Request/Response (for protocols)          |
+---------------------------------------------+
      |            |                |
      v            v                v
+-----------+ +------------+ +----------------+
|  Pairing  | | Messaging  | | File Transfer  |
| Protocol  | | Protocol   | | Protocol       |
+-----------+ +------------+ +----------------+
```

### Key Components

#### 1. Core Integration (`lib.rs`)

The `Core` struct serves as the primary public API for all networking operations. It abstracts away the complexity of the underlying swarm and protocols.

```rust
// In `Core` from src/lib.rs
pub async fn init_networking(&mut self) -> Result<(), Box<dyn std::error::Error>>

pub async fn start_pairing_as_initiator(
    &self,
) -> Result<(String, u32), Box<dyn std::error::Error>>

pub async fn start_pairing_as_joiner(
    &self,
    code: &str,
) -> Result<(), Box<dyn std::error::Error>>

pub async fn share_with_device(
    &mut self,
    files: Vec<PathBuf>,
    device_id: uuid::Uuid,
    destination_path: Option<PathBuf>,
) -> Result<Vec<TransferId>, SharingError>
```

**Key Features**:

- **Simplified API**: All network functionality is exposed through the main `Core` object.
- **Lazy Initialization**: Networking is an optional component that can be initialized on demand with `init_networking`.
- **Centralized Logic**: High-level workflows like pairing and file sharing are orchestrated within `Core`, ensuring consistent behavior.

#### 2. Networking Core (`infrastructure/networking/core/mod.rs`)

This is the central engine that owns the libp2p `Swarm` and manages all networking state.

```rust
// from src/infrastructure/networking/core/mod.rs
pub struct NetworkingCore {
	identity: NetworkIdentity,
	swarm: Swarm<UnifiedBehaviour>,
	protocol_registry: Arc<RwLock<ProtocolRegistry>>,
	device_registry: Arc<RwLock<DeviceRegistry>>,
	// ... event and command channels
}
```

**Key Features**:

- **Single Source of Truth**: Manages the state of the network, including the swarm, devices, and protocols.
- **Event-Driven**: Communicates with the rest of the application via an event bus and command channels.
- **Runs in Background**: The `NetworkingEventLoop` runs in a separate Tokio task, processing all swarm events asynchronously.

#### 3. Unified Behavior (`infrastructure/networking/core/behavior.rs`)

A key simplification is the `UnifiedBehaviour`, which combines all necessary libp2p protocols into a single struct. This ensures all protocols operate on the same underlying network state.

```rust
// from src/infrastructure/networking/core/behavior.rs
#[derive(NetworkBehaviour)]
pub struct UnifiedBehaviour {
	pub kademlia: kad::Behaviour<MemoryStore>,
	pub mdns: mdns::tokio::Behaviour,
	pub pairing: request_response::cbor::Behaviour<PairingMessage, PairingMessage>,
	pub messaging: request_response::cbor::Behaviour<DeviceMessage, DeviceMessage>,
	pub file_transfer: request_response::cbor::Behaviour<FileTransferMessage, FileTransferMessage>,
}
```

**Key Features**:

- **Kademlia DHT**: Enables global peer discovery using the same mechanism as IPFS.
- **mDNS**: Provides zero-configuration peer discovery on local networks.
- **Request/Response**: A single, flexible pattern is used for all interaction-based protocols like pairing and file transfer.

#### 4. Device Identity and Registry (`device/`, `utils/identity.rs`)

Device identity is managed cryptographically, and the `DeviceRegistry` acts as the state machine for device relationships.

```rust
// from src/infrastructure/networking/device/mod.rs
pub enum DeviceState {
	Discovered,
	Pairing,
	Paired,
	Connected,
	Disconnected,
}

// from src/infrastructure/networking/utils/identity.rs
pub struct NetworkIdentity {
	keypair: Keypair,
	peer_id: PeerId,
}
```

**Key Features**:

- **Cryptographic Identity**: Each device has a stable `NetworkIdentity` derived from its master key, ensuring a consistent `PeerId` across sessions.
- **Stateful Tracking**: The `DeviceRegistry` tracks every known device's state, from initial discovery through pairing and connection.
- **Session Key Management**: The registry securely manages session keys required for encrypted communication with paired devices.

## Key Protocols

#### 1. Device Pairing Protocol

A secure protocol to establish trust between devices, using 12-word BIP39 codes.

- **Workflow**:
  1.  **Initiator** (`alice_pairing_scenario`) generates a pairing code and publishes a record to the DHT.
  2.  **Joiner** (`bob_pairing_scenario`) uses the code to find the initiator's record on the DHT and/or discover them via mDNS.
  3.  A secure, challenge-response handshake occurs to verify both parties.
  4.  Session keys are derived, and the devices are registered as `Paired`.
- **Security**: The process is secured using Ed25519 signatures, as implemented in `PairingSecurity`.
- **Messages**: The protocol uses the `PairingMessage` enum for all communication stages.

#### 2\. File Transfer Protocol

A protocol for sending files and directories between paired devices, forming the basis of cross-device copy operations.

- **Workflow**:
  1.  A `FileCopyJob` is dispatched for a cross-device copy.
  2.  The job initiates a transfer session via the `FileTransferProtocolHandler`.
  3.  A `TransferRequest` message is sent to the target device.
  4.  Upon acceptance, the file is broken into encrypted chunks and streamed to the receiver.
  5.  The receiver writes the chunks to disk and verifies the final file checksum.
- **Encryption**: Each chunk of data is encrypted using session keys derived during pairing, ensuring end-to-end security.

## Implementation Status

### ✅ Completed Features

- **Unified Swarm Architecture**: A single `NetworkingCore` with a `UnifiedBehaviour` is fully implemented and functional.
- **Dual-Discovery Mechanism**: Both Kademlia DHT and mDNS are integrated and used for peer discovery.
- **Device Pairing**: The end-to-end pairing flow is complete and validated by integration tests (`test_core_pairing.rs`).
- **Encrypted File Transfer**: The end-to-end file transfer flow is complete and validated by integration tests (`test_core_file_transfer.rs`).
- **Protocol Handler System**: The `ProtocolRegistry` and `ProtocolHandler` trait provide a working, extensible system for routing messages.
- **Core API Integration**: High-level methods like `init_networking`, `start_pairing`, and `share_with_device` are implemented on the `Core` struct.

### 🚧 Future Enhancements

- **Real-time Sync Protocol**: The framework exists, but the application logic for synchronizing library changes needs to be implemented.
- **Connection Resilience**: While the foundation is strong, advanced logic for handling poor network conditions, more sophisticated retries, and NAT traversal improvements can be built out.
- **Spacedrop UI**: The underlying protocol is present, but a user-facing interaction layer for accepting Spacedrop requests is needed.
- **Performance Metrics**: The architecture allows for collecting detailed connection metrics (latency, bandwidth), which can be exposed to the user.

## Development Workflow

### Running Tests

The most important networking tests are the integration tests that spawn multiple processes.

```bash
# Run the device pairing integration test
cargo test --test test_core_pairing -- --ignored --nocapture

# Run the file transfer integration test
cargo test --test test_core_file_transfer -- --ignored --nocapture

# Run the mDNS-only discovery helper test
cargo test --test mdns_discovery_test -- --nocapture
```

### Debug Logging

Enable detailed libp2p and networking logs using the `RUST_LOG` environment variable:

```bash
# Enable detailed swarm and networking logs
RUST_LOG=info,sd_core_new::infrastructure::networking=debug,libp2p_swarm=debug cargo test ...
```
