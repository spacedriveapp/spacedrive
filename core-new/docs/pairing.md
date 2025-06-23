# Spacedrive Device Pairing System

## Overview

The Spacedrive pairing system enables secure device-to-device connection establishment using human-readable BIP39 pairing codes. This system is built on LibP2P networking with Ed25519 cryptographic verification and includes session persistence for reliability.

## Architecture

### Core Components

```
src/infrastructure/networking/protocols/pairing/
├── mod.rs           # Main PairingProtocolHandler with session management
├── types.rs         # PairingSession, PairingState, PairingCode definitions
├── messages.rs      # Protocol message types for communication
├── initiator.rs     # Alice-side pairing logic (code generator)
├── joiner.rs        # Bob-side pairing logic (code consumer)
├── security.rs      # Ed25519 signature verification and validation
└── persistence.rs   # Session persistence for restart survival
```

### System Flow

```
Device A (Initiator/Alice)              Device B (Joiner/Bob)
        │                                       │
        ▼                                       ▼
   Generate BIP39 Code                    Enter BIP39 Code
        │                                       │
        ▼                                       ▼
   Start Pairing Session                 Join Pairing Session
        │                                       │
        ▼                                       ▼
   Publish DHT Record                    Query DHT + mDNS Discovery
        │                                       │
        ▼                                       ▼
   Wait for Connection              ←────── Establish Connection
        │                                       │
        ▼                                       ▼
   Send Challenge                         Receive Challenge
        │                                       │
        ▼                                       ▼
   Verify Response               ←────── Sign & Send Response
        │                                       │
        ▼                                       ▼
   Send Completion                       Receive Completion
        │                                       │
        ▼                                       ▼
   Store Device Info                     Store Device Info
        │                                       │
        ▼                                       ▼
   PAIRING SUCCESS                       PAIRING SUCCESS
```

## Core Types

### PairingCode (BIP39-based)

```rust
pub struct PairingCode {
    /// 256-bit cryptographic secret
    secret: [u8; 32],
    
    /// 12 words from BIP39 wordlist (user-friendly)
    words: [String; 12],
    
    /// Session ID derived from secret
    session_id: Uuid,
    
    /// Expiration timestamp (5 minutes)
    expires_at: DateTime<Utc>,
}
```

**Key Features:**
- **Human-readable**: 12 BIP39 words like "frame garden half repair lucky swamp..."
- **Cryptographically secure**: 256-bit entropy
- **Deterministic**: Same words always produce same session ID
- **Short-lived**: 5-minute expiration for security

### PairingSession

```rust
pub struct PairingSession {
    pub id: Uuid,
    pub state: PairingState,
    pub remote_device_id: Option<Uuid>,
    pub remote_device_info: Option<DeviceInfo>,
    pub remote_public_key: Option<Vec<u8>>,  // For signature verification
    pub shared_secret: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
}
```

### PairingState

```rust
pub enum PairingState {
    WaitingForConnection,     // Alice waiting for Bob
    Scanning,                 // Bob searching for Alice
    ChallengeReceived { challenge: Vec<u8> },
    ResponsePending { 
        challenge: Vec<u8>, 
        response_data: Vec<u8>,
        remote_peer_id: Option<PeerId>,
    },
    ResponseSent,
    Completed,
    Failed { reason: String },
}
```

## Protocol Messages

### Message Types

```rust
pub enum PairingMessage {
    // Bob → Alice: Initial pairing request
    PairingRequest {
        session_id: Uuid,
        device_info: DeviceInfo,
        public_key: Vec<u8>,
    },
    
    // Alice → Bob: Challenge for authentication
    Challenge {
        session_id: Uuid,
        challenge: Vec<u8>,        // 32-byte random challenge
        device_info: DeviceInfo,   // Alice's device info
    },
    
    // Bob → Alice: Signed challenge response
    Response {
        session_id: Uuid,
        response: Vec<u8>,         // Ed25519 signature of challenge
        device_info: DeviceInfo,   // Bob's device info
    },
    
    // Alice → Bob: Completion confirmation
    Complete {
        session_id: Uuid,
        success: bool,
        reason: Option<String>,
    },
}
```

## Main Components

### PairingProtocolHandler

**Core functionality:**
```rust
impl PairingProtocolHandler {
    // Session management
    pub async fn start_pairing_session_with_id(&self, session_id: Uuid) -> Result<()>
    pub async fn join_pairing_session(&self, session_id: Uuid) -> Result<()>
    pub async fn get_active_sessions(&self) -> Vec<PairingSession>
    
    // Device info for DHT advertising
    pub async fn get_device_info(&self) -> Result<DeviceInfo>
    
    // Session cleanup
    pub async fn cleanup_expired_sessions(&self) -> Result<usize>
    
    // Persistence (optional)
    pub async fn load_persisted_sessions(&self) -> Result<usize>
}
```

**Key features:**
- **Session state management** with atomic transitions
- **Automatic persistence** on state changes (if enabled)
- **Background cleanup** of expired sessions
- **Role-based logging** with [INITIATOR]/[JOINER] prefixes

### Security Module

**Cryptographic verification:**
```rust
impl PairingSecurity {
    // Real Ed25519 signature verification using libp2p
    pub fn verify_challenge_response(
        public_key_bytes: &[u8],    // Protobuf-encoded libp2p key
        challenge: &[u8],           // 32-byte challenge
        signature: &[u8],           // 64-byte Ed25519 signature
    ) -> Result<bool>
    
    // Input validation
    pub fn validate_public_key(public_key_bytes: &[u8]) -> Result<()>
    pub fn validate_challenge(challenge: &[u8]) -> Result<()>
    pub fn validate_signature(signature: &[u8]) -> Result<()>
}
```

**Security features:**
- **Real Ed25519 verification** (not placeholders)
- **Protobuf key format** support for libp2p compatibility
- **Input validation** against weak/malformed data
- **Comprehensive test coverage** with crypto demonstrations

### Persistence Module

**Session persistence:**
```rust
impl PairingPersistence {
    // Save/load sessions to JSON files
    pub async fn save_sessions(&self, sessions: &HashMap<Uuid, PairingSession>) -> Result<()>
    pub async fn load_sessions(&self) -> Result<HashMap<Uuid, PairingSession>>
    
    // Cleanup operations
    pub async fn cleanup_expired_sessions(&self) -> Result<usize>
    pub async fn clear_all_sessions(&self) -> Result<()>
}
```

**Persistence features:**
- **Atomic file operations** (write to temp, then rename)
- **Automatic expiration** (1-hour session lifetime)
- **State filtering** (only persists meaningful states)
- **Error recovery** with proper IO error handling

## Discovery Methods

### 1. mDNS (Local Network)

**How it works:**
- Alice broadcasts mDNS service with session fingerprint
- Bob scans mDNS services for matching fingerprint
- Direct peer-to-peer connection on local network
- **Fast and reliable** for same-network devices

### 2. DHT (Distributed Hash Table)

**How it works:**
- Alice publishes session record to DHT using session ID as key
- Bob queries DHT using session ID from pairing code
- **Works across networks** and NAT boundaries
- Includes retry logic for network resilience

### 3. Unified Approach

The system runs **both methods simultaneously**:
```rust
// Alice (Initiator)
1. Generate pairing code with session ID
2. Start local session in WaitingForConnection state
3. Publish DHT record with device info and addresses
4. Broadcast mDNS service with session fingerprint
5. Wait for connections from any method

// Bob (Joiner) 
1. Parse pairing code to get session ID
2. Create local session in Scanning state
3. Query DHT for session record (with retries)
4. Scan mDNS for matching fingerprint
5. Connect via whichever method finds peer first
```

## Authentication Flow

### Challenge-Response Protocol

```
Alice                           Bob
  │                              │
  │ 1. PairingRequest             │
  │ ◄─────────────────────────────┤ (device_info + public_key)
  │                              │
  │ 2. Challenge                 │
  │ ──────────────────────────── ► │ (32-byte random + device_info)
  │                              │
  │ 3. Response                  │
  │ ◄─────────────────────────────┤ (Ed25519 signature + device_info)
  │                              │
  │ 4. Complete                  │
  │ ──────────────────────────── ► │ (success=true)
  │                              │
```

### Security Verification

**Step-by-step validation:**

1. **Public Key Validation**: Format and protobuf decoding
2. **Challenge Generation**: 32-byte cryptographically random
3. **Signature Creation**: Bob signs challenge with his private key  
4. **Signature Verification**: Alice verifies using Bob's public key
5. **Device Registration**: Store device info with session keys

**Cryptographic details:**
- **Key type**: Ed25519 (via libp2p identity)
- **Key encoding**: Protobuf format (32-44 bytes typically)
- **Signature size**: 64 bytes
- **Challenge size**: 32 bytes

## Integration with Core

### Initialization

```rust
// Basic handler (no persistence)
let pairing_handler = PairingProtocolHandler::new(
    identity,           // NetworkIdentity with Ed25519 keypair
    device_registry,    // Shared device state
    logger,            // Structured logging
);

// Handler with persistence (recommended)
let pairing_handler = PairingProtocolHandler::new_with_persistence(
    identity,
    device_registry,
    logger,
    data_dir,          // Path for session storage
);

// Load any persisted sessions
pairing_handler.load_persisted_sessions().await?;
```

### Starting Pairing (Alice)

```rust
// Generate BIP39 pairing code
let pairing_code = PairingCode::generate()?;
let session_id = pairing_code.session_id();

// Start pairing session
pairing_handler.start_pairing_session_with_id(session_id).await?;

// Register in device registry  
let device_registry = networking_core.device_registry();
device_registry.write().await.start_pairing(
    device_id, 
    peer_id, 
    session_id
)?;

// Publish to DHT for remote discovery
let advertisement = PairingAdvertisement {
    peer_id: peer_id.to_string(),
    addresses: external_addresses,
    device_info: device_info,
    expires_at: Utc::now() + Duration::minutes(5),
    created_at: Utc::now(),
};

let key = RecordKey::new(&session_id.as_bytes());
let value = serde_json::to_vec(&advertisement)?;
networking_core.publish_dht_record(key, value).await?;

println!("Pairing code: {}", pairing_code.to_string());
```

### Joining Pairing (Bob)

```rust
// Parse user-entered pairing code
let pairing_code = PairingCode::from_string(user_input)?;
let session_id = pairing_code.session_id();

// Join Alice's session
pairing_handler.join_pairing_session(session_id).await?;

// Query DHT for Alice's advertisement
let key = RecordKey::new(&session_id.as_bytes());
networking_core.query_dht_record(key).await?;

// mDNS discovery happens automatically in background
// Connection establishment and authentication follow
```

### Monitoring Progress

```rust
// Check pairing status
let sessions = pairing_handler.get_active_sessions().await;
for session in sessions {
    println!("Session {}: {:?}", session.id, session.state);
}

// Get connected devices after successful pairing
let devices = networking_core.get_connected_devices().await;
println!("Connected to {} devices", devices.len());
```

## Configuration

### Timeouts and Limits

```rust
// Session expiration (in cleanup_expired_sessions)
const SESSION_TIMEOUT: Duration = Duration::minutes(10);

// Pairing code expiration  
const CODE_EXPIRATION: Duration = Duration::minutes(5);

// DHT retry intervals
const DHT_RETRY_INTERVAL: Duration = Duration::seconds(3);
const DHT_MAX_RETRIES: usize = 3;

// Challenge size
const CHALLENGE_SIZE: usize = 32;

// Signature size (Ed25519)
const SIGNATURE_SIZE: usize = 64;
```

### Persistence Settings

```rust
// Session file location
let sessions_file = data_dir.join("pairing_sessions.json");

// Automatic persistence triggers
- Session state changes
- New session creation
- Session completion/failure

// Cleanup schedule  
- Expired sessions removed on load
- Background cleanup every 60 seconds
- Sessions older than 1 hour discarded
```

## Error Handling

### Common Errors

```rust
pub enum NetworkingError {
    // Connection issues
    LibP2P(String),              // LibP2P transport errors
    ConnectionFailed(String),    // Network connectivity
    Timeout(String),            // Operation timeouts
    
    // Protocol issues  
    Protocol(String),           // Invalid messages/states
    AuthenticationFailed(String), // Crypto verification failures
    
    // Device issues
    DeviceNotFound(Uuid),       // Unknown device ID
    
    // System issues
    Io(std::io::Error),         // File system operations
    Serialization(serde_json::Error), // JSON parsing
}
```

### Error Recovery

**Session failure handling:**
```rust
// Mark session as failed with reason
session.state = PairingState::Failed { 
    reason: error.to_string() 
};

// Clean up resources
pairing_handler.cancel_session(session_id).await?;

// Retry logic (user-initiated)
// Generate new pairing code and start fresh
```

**Network failure handling:**
```rust
// DHT queries include automatic retries
// mDNS discovery is continuous during scanning
// Connection failures trigger reconnection attempts
// Session persistence survives application restarts
```

## Testing

### Unit Tests

**Security module:**
```bash
cargo test infrastructure::networking::protocols::pairing::security::tests
```

**Persistence module:**
```bash
cargo test infrastructure::networking::protocols::pairing::persistence::tests  
```

### Integration Tests

**Alice/Bob pairing flow:**
```bash
# Terminal 1 (Alice)
cargo run --bin core_test_alice --data-dir /tmp/alice-test

# Terminal 2 (Bob)  
cargo run --bin core_test_bob --data-dir /tmp/bob-test
```

**Expected output:**
```
Alice: Generated pairing code: frame garden half repair lucky swamp...
Bob: Found pairing code and connecting...
Alice sees: Bob's Test Device 
Bob sees: Alice's Test Device
PAIRING_SUCCESS: Both devices connected
```

### Test Coverage

- ✅ **Cryptographic verification** (real Ed25519 signatures)
- ✅ **Session persistence** (save/load/cleanup)
- ✅ **Protocol message handling** (all message types)
- ✅ **Error conditions** (invalid data, timeouts)
- ✅ **End-to-end flow** (Alice/Bob integration)

## Performance Characteristics

### Typical Timings

- **Code generation**: < 1ms
- **mDNS discovery**: 1-3 seconds (local network)
- **DHT discovery**: 3-10 seconds (remote network)
- **Authentication**: < 100ms
- **Total pairing time**: 2-15 seconds

### Resource Usage

- **Memory**: ~1MB per active session
- **Storage**: ~1KB per persisted session  
- **Network**: ~2KB total protocol overhead
- **CPU**: Minimal (Ed25519 is fast)

### Scalability

- **Concurrent sessions**: Limited by memory (~1000s)
- **Session persistence**: Limited by disk space
- **DHT load**: Distributed across network
- **mDNS load**: Local network only

## Security Considerations

### Threat Model

**Protected against:**
- ✅ **Eavesdropping**: All crypto verification
- ✅ **Man-in-the-middle**: Challenge-response authentication  
- ✅ **Replay attacks**: Session IDs and timestamps
- ✅ **Brute force**: 256-bit entropy, short expiration
- ✅ **Invalid signatures**: Real Ed25519 verification

**Current limitations:**
- ⚠️ **No rate limiting** on pairing attempts
- ⚠️ **No user confirmation** (auto-accepts valid requests)
- ⚠️ **No device limits** (unlimited paired devices)

### Cryptographic Strength

- **Secret generation**: 256-bit cryptographically secure random
- **Key derivation**: Blake3 hashing for session IDs
- **Signature algorithm**: Ed25519 (industry standard)
- **Key encoding**: LibP2P protobuf format
- **Validation**: Comprehensive input checking

## Future Enhancements

### Planned Improvements

1. **Rate limiting** on pairing requests
2. **User confirmation UI** for pairing requests
3. **Device trust levels** and revocation
4. **Pairing request queuing** for better UX
5. **QR code generation** for mobile pairing
6. **Connection retry logic** for reliability

### Performance Optimizations

1. **Connection pooling** for reused connections
2. **DHT caching** to reduce query load
3. **mDNS response caching** for faster discovery
4. **Background session cleanup** optimization

The Spacedrive pairing system provides a robust, secure foundation for device-to-device connections with real cryptographic verification, automatic persistence, and comprehensive error handling.