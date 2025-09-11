# Spacedrive v2: Device Pairing Protocol

The Spacedrive v2 device pairing system is a secure, robust protocol for establishing a trusted, end-to-end encrypted connection between two devices, regardless of whether they are on the same local network or across the internet. It is built upon the unified libp2p networking stack and uses a combination of modern cryptographic principles and user-friendly codes to deliver a seamless pairing experience.

## Overview

The primary goal of the pairing system is to create a secure relationship between two devices, allowing them to communicate directly, exchange session keys, and perform operations like file transfer and synchronization. This is achieved through a challenge-response handshake initiated by a user-friendly 12-word BIP39 code.

## Key Features

- **Cryptographic Security**: Pairing is secured using an Ed25519 challenge-response signature verification, ensuring that only the intended device can complete the process. All transport-level communication is encrypted using the Noise Protocol.
- **Dual-Discovery Mechanism**: The system uses a unified discovery approach. It queries the Kademlia Distributed Hash Table (DHT) for remote discovery (across different networks) while simultaneously listening for local peers via mDNS. The first successful discovery method is used, providing both speed on local networks and reachability over the internet.
- **User-Friendly Pairing Codes**: Instead of complex hashes, the system generates a 12-word BIP39 mnemonic code. This code is easy for users to read and type, yet contains enough entropy to securely identify a pairing session.
- **State Machine Architecture**: The entire pairing process is managed by a robust state machine within the `PairingProtocolHandler`, which tracks each session from initiation to completion or failure.
- **Automatic Device Registration**: Upon successful pairing, devices are automatically added to each other's `DeviceRegistry`, and persistent, encrypted session keys are established for all future communication.

## Core Components

The pairing system is a collaboration between several key components in the new architecture:

1.  **`Core` (`src/lib.rs`)**: Provides the high-level public API for initiating pairing. The `start_pairing_as_initiator()` and `start_pairing_as_joiner()` methods are the entry points for the entire process.
2.  **`PairingProtocolHandler` (`src/infrastructure/networking/protocols/pairing/mod.rs`)**: This is the heart of the pairing system. It acts as a state machine, managing active pairing sessions, generating cryptographic challenges, verifying responses, and orchestrating the entire protocol flow.
3.  **`PairingCode` & `PairingSession` (`types.rs`)**: These structs define the data model for pairing.
    - `PairingCode`: Represents the 12-word code and the cryptographic secret from which the session ID is derived.
    - `PairingSession`: Tracks the state (`PairingState` enum) of a single pairing attempt, including remote device info and derived keys.
4.  **`DeviceRegistry` (`device/registry.rs`)**: The central registry for device state. During pairing, it maps an ephemeral `session_id` to a permanent `device_id` and stores the final `SessionKeys` upon completion.
5.  **`UnifiedBehaviour` (`core/behavior.rs`)**: The unified libp2p behavior that enables discovery. Its Kademlia DHT component is used to publish and query pairing advertisements, while the mDNS component listens for local peers.

## The Pairing Flow

The pairing process involves two roles: the **Initiator** (who generates the code) and the **Joiner** (who uses the code).

### Initiator Flow (e.g., Alice)

1.  **Initiation**: A user triggers the pairing process, calling `core.start_pairing_as_initiator()`.
2.  **Code Generation**: A cryptographically secure `PairingCode` is generated. A unique `session_id` is derived from this code's entropy.
3.  **DHT Advertisement**:
    - The Initiator gathers its public `DeviceInfo` (name, OS, etc.) and its external network addresses (e.g., `/ip4/1.2.3.4/tcp/5678`).
    - This information is packaged into a `PairingAdvertisement`.
    - The advertisement is published to the Kademlia DHT. The DHT `RecordKey` is the `session_id`, making it discoverable by the Joiner.
4.  **Waiting State**: The Initiator's `PairingSession` enters the `WaitingForConnection` state. It now listens for incoming connections from any peer.
5.  **Challenge Issuance**:
    - When a Joiner connects and sends a `PairingRequest` message containing its public key and `DeviceInfo`, the Initiator's `PairingProtocolHandler` receives it.
    - The handler generates a random 32-byte cryptographic `challenge`.
    - It sends this `challenge` back to the Joiner in a `Challenge` message. The session state transitions to `ChallengeReceived`.
6.  **Response Verification**:
    - The Initiator receives a `Response` message from the Joiner. This message contains the original challenge signed with the Joiner's private device key.
    - The `PairingSecurity` module is used to verify the signature against the Joiner's public key (received in step 5).
7.  **Completion**:
    - If the signature is valid, the pairing is successful.
    - The Initiator derives the shared session keys.
    - It updates its `DeviceRegistry` to mark the Joiner as a trusted, paired device.
    - It sends a final `Complete` message to the Joiner. The session is now `Completed`.

### Joiner Flow (e.g., Bob)

1.  **Code Entry**: The user enters the 12-word `PairingCode` provided by the Initiator.
2.  **Session ID Extraction**: The `PairingCode` is parsed to deterministically reconstruct the same `session_id` the Initiator created.
3.  **Unified Discovery**:
    - The `core.start_pairing_as_joiner()` method is called.
    - The `NetworkingCore` immediately begins querying the DHT using the `session_id` to find the Initiator's `PairingAdvertisement`.
    - Simultaneously, the mDNS service listens for the Initiator on the local network.
4.  **Connection**:
    - Once the Initiator's address is discovered (via DHT or mDNS), the system establishes a direct TCP connection.
5.  **Sending Request**:
    - As soon as the connection is established, the Joiner sends a `PairingRequest` message. This message includes its own `DeviceInfo` and, crucially, its public key.
6.  **Signing Challenge**:
    - The Joiner receives the `Challenge` message from the Initiator.
    - It uses its private `NetworkIdentity` key to sign the 32-byte challenge.
    - It sends the resulting 64-byte signature back in a `Response` message.
7.  **Completion**:
    - The Joiner receives the final `Complete` message from the Initiator.
    - It derives the same shared session keys.
    - It updates its `DeviceRegistry` to add the Initiator as a trusted, paired device. The connection is now fully authenticated and encrypted for all future communication.

## Security Model

The pairing protocol is designed with security as a primary concern.

- **Transport Encryption**: All communication, from the very first connection attempt, is encrypted using the **Noise Protocol**, which provides forward secrecy.
- **Cryptographic Authentication**: The identity of the joining device is verified using an **Ed25519 digital signature**. The challenge-response mechanism prevents replay attacks and ensures that the device joining is the one that possesses the private key corresponding to the public key it presented.
- **Session Key Derivation**: Once authenticated, the shared secret from the pairing code is used as input to a **Key Derivation Function (HKDF)**. This generates strong, unique symmetric keys for sending and receiving data between the two devices, ensuring all subsequent communication is confidential and authenticated.
- **Ephemeral & Discoverable Session ID**: The `session_id` used for DHT discovery is derived from the pairing code but is not the secret itself. This allows the session to be publicly discoverable for a short period without exposing any sensitive information. The codes and sessions expire after 5-10 minutes to limit the window of opportunity for attacks.

## Implementation Details

The core of the logic is implemented in the `PairingProtocolHandler`, which uses a `PairingState` enum to manage the lifecycle of each `PairingSession`.

```rust
// from src/infrastructure/networking/protocols/pairing/messages.rs
pub enum PairingMessage {
    PairingRequest {
        session_id: Uuid,
        device_info: DeviceInfo,
        public_key: Vec<u8>,
    },
    Challenge {
        session_id: Uuid,
        challenge: Vec<u8>,
        device_info: DeviceInfo,
    },
    Response {
        session_id: Uuid,
        response: Vec<u8>,
        device_info: DeviceInfo,
    },
    Complete {
        session_id: Uuid,
        success: bool,
        reason: Option<String>,
    },
}
```

This message-passing design, combined with a robust state machine, ensures that the pairing process is reliable and secure from start to finish.
