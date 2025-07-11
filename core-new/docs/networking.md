# Spacedrive v2 Networking Module: Iroh-Powered P2P

The Spacedrive v2 networking module provides robust device-to-device communication using Iroh, a modern peer-to-peer networking library. It enables secure device pairing, peer discovery, and encrypted data transfer between Spacedrive instances, forming the backbone of the Virtual Distributed File System (VDFS).

This implementation leverages Iroh's QUIC-based transport for reliable connections with excellent NAT traversal capabilities (90%+ success rate) and built-in encryption.

## Overview

The networking module is tightly integrated into the `Core` struct and provides:

- **Simplified Transport**: QUIC-based transport with built-in encryption and multiplexing
- **Relay Fallback**: Automatic relay server fallback when direct connections fail  
- **Protocol Negotiation**: ALPN-based protocol selection for pairing, messaging, and file transfer
- **Centralized State Management**: A single `DeviceRegistry` tracks the state of all known peers, from discovered to paired and connected
- **Extensible Protocol System**: A modular `ProtocolRegistry` allows for clean separation of concerns, routing incoming messages to the correct handler
- **Secure Device Pairing**: A robust, challenge-response pairing protocol secured with cryptographic signatures and initiated with user-friendly BIP39 word codes
- **End-to-End Encrypted File Transfer**: High-level APIs for sharing files between devices, built on an underlying protocol that handles chunking, encryption, and verification

## Architecture

The networking architecture uses Iroh's endpoint model for simplicity and reliability. All network operations are managed by a single `NetworkingService` instance, which is initialized and managed by the main `Core` struct.

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
|    NetworkingService (`core/mod.rs`)        |
| - Endpoint (Iroh)                           |
| - DeviceRegistry (Tracks all devices)       |
| - ProtocolRegistry (Routes messages)        |
| - NetworkIdentity (Ed25519-based)           |
+---------------------------------------------+
                   |
                   v
+---------------------------------------------+
|  NetworkingEventLoop (`core/event_loop.rs`) |
| - Handles incoming connections              |
| - Routes based on ALPN protocol             |
| - Manages command processing                |
+---------------------------------------------+
      |            |                |
      v            v                v
+-----------+ +------------+ +----------------+
|  Pairing  | | Messaging  | | File Transfer  |
| Protocol  | | Protocol   | | Protocol       |
+-----------+ +------------+ +----------------+
```

## Key Changes from libp2p

1. **Transport**: Replaced TCP+Noise+Yamux with QUIC (better NAT traversal, built-in encryption)
2. **Identity**: Uses Iroh's Ed25519-based NodeId instead of libp2p's PeerId
3. **Addressing**: NodeAddr replaces Multiaddr for simpler address handling
4. **Discovery**: Currently manual (DHT discovery to be implemented separately)
5. **Protocols**: ALPN-based protocol negotiation instead of libp2p's protocol strings

## Components

### NetworkingService
The main entry point for all networking operations. Manages the Iroh endpoint and coordinates between different components.

### NetworkIdentity  
Manages the device's cryptographic identity, compatible with both Iroh's NodeId system and legacy Ed25519 signing.

### DeviceRegistry
Central registry tracking all known devices and their states (discovered, pairing, paired, connected, disconnected).

### ProtocolRegistry
Routes incoming messages to appropriate protocol handlers based on protocol name.

### Event Loop
Processes incoming connections and routes them to protocol handlers based on ALPN negotiation.

## Protocols

### Pairing Protocol
- Secure device pairing using challenge-response authentication
- BIP39 mnemonic codes for user-friendly pairing
- Ed25519 signatures for cryptographic verification

### Messaging Protocol  
- Real-time message exchange between paired devices
- JSON-serialized messages for flexibility

### File Transfer Protocol
- Chunked file transfer with progress tracking
- End-to-end encryption using session keys
- Automatic resume for interrupted transfers

## Future Enhancements

1. **Discovery**: Implement DHT-based discovery for finding peers
2. **Stream Integration**: Port protocols to use Iroh's native stream handling
3. **Relay Deployment**: Deploy custom relay servers for Spacedrive Cloud
4. **Protocol Optimization**: Optimize protocols for Iroh's capabilities