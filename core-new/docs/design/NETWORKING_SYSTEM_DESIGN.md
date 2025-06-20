# Networking System Design

## Overview

This document outlines a flexible networking system for Spacedrive that supports both local P2P connections and internet-based communication through a relay service. The design prioritizes security, simplicity, and transport flexibility while leveraging existing libraries to minimize development effort.

## Core Requirements

1. **Dual Transport** - Works seamlessly over local network and internet
2. **End-to-End Encryption** - All connections encrypted, no exceptions
3. **File Sharing** - Stream large files efficiently
4. **Sync Operations** - Low-latency sync protocol support
5. **Authentication** - 1Password-style master key setup
6. **Zero Configuration** - Automatic discovery on local networks

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Application Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ File Sharing │  │     Sync     │  │  Remote Control    │   │
│  │   Service    │  │   Protocol   │  │    (Future)        │   │
│  └──────────────┘  └──────────────┘  └────────────────────┘   │
└─────────────────────────┬───────────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────────┐
│                      Transport Abstraction                       │
│  ┌────────────────────────────────────────────────────────┐    │
│  │              NetworkConnection Interface                │    │
│  │  - send(data) / receive() → data                      │    │
│  │  - stream_file(path) / receive_file() → stream        │    │
│  │  - reliable & ordered delivery                        │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────┬───────────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────────┐
│                     Transport Implementations                    │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │   Local P2P      │  │  Internet Relay  │  │ Direct Internet│ │
│  │                  │  │                  │  │   (Future)     │ │
│  │ - mDNS Discovery │  │ - Relay Server   │  │ - STUN/TURN    │ │
│  │ - Direct Connect │  │ - WebSocket/QUIC │  │ - Hole Punching│ │
│  │ - LAN Only      │  │ - NAT Traversal  │  │ - Public IPs   │ │
│  └──────────────────┘  └──────────────────┘  └──────────────┐  │
└─────────────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────────┐
│                    Security & Crypto Layer                       │
│  ┌────────────────────────────────────────────────────────┐    │
│  │         Noise Protocol Framework (or similar)           │    │
│  │  - XX Pattern: mutual authentication                   │    │
│  │  - Forward secrecy                                     │    │
│  │  - Zero round-trip encryption                          │    │
│  └────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Device Identity & Authentication

**CRITICAL: Integration with Existing Device Identity**

The networking module MUST integrate with Spacedrive's existing persistent device identity system (see `core-new/src/device/`). The current device system provides:

- **Persistent Device UUID**: Stored in `device.json`, survives restarts
- **Device Configuration**: Name, OS, hardware model, creation time
- **Cross-Instance Consistency**: Multiple Spacedrive instances on same device share identity

**Problem with Original Design:**
- NetworkingDeviceId derived from public key changes each restart
- No persistence of cryptographic keys
- Multiple instances would have different network identities
- Device pairing would break after restart

**Corrected Architecture:**

```rust
/// Network identity tied to persistent device identity
pub struct NetworkIdentity {
    /// MUST match the persistent device UUID from DeviceManager
    pub device_id: Uuid, // From existing device system
    
    /// Device's public key (Ed25519) - STORED PERSISTENTLY
    pub public_key: PublicKey,
    
    /// Device's private key (encrypted at rest) - STORED PERSISTENTLY  
    private_key: EncryptedPrivateKey,
    
    /// Human-readable device name (from DeviceConfig)
    pub device_name: String,
    
    /// Network-specific identifier (derived from device_id + public_key)
    pub network_fingerprint: NetworkFingerprint,
}

/// Network fingerprint for wire protocol identification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetworkFingerprint([u8; 32]);

impl NetworkFingerprint {
    /// Create network fingerprint from device UUID and public key
    fn from_device(device_id: Uuid, public_key: &PublicKey) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(device_id.as_bytes());
        hasher.update(public_key.as_bytes());
        let hash = hasher.finalize();
        let mut fingerprint = [0u8; 32];
        fingerprint.copy_from_slice(hash.as_bytes());
        NetworkFingerprint(fingerprint)
    }
}

/// Extended device configuration with networking keys
#[derive(Serialize, Deserialize)]
pub struct ExtendedDeviceConfig {
    /// Base device configuration
    #[serde(flatten)]
    pub device: DeviceConfig,
    
    /// Network cryptographic keys (encrypted)
    pub network_keys: Option<EncryptedNetworkKeys>,
    
    /// When network identity was created
    pub network_identity_created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
pub struct EncryptedNetworkKeys {
    /// Ed25519 private key encrypted with user password
    pub encrypted_private_key: EncryptedPrivateKey,
    
    /// Public key (not encrypted)
    pub public_key: PublicKey,
    
    /// Salt for key derivation
    pub salt: [u8; 32],
    
    /// Key derivation parameters
    pub kdf_params: KeyDerivationParams,
}

/// Integration with DeviceManager
impl NetworkIdentity {
    /// Create network identity from existing device configuration
    pub async fn from_device_manager(
        device_manager: &DeviceManager,
        password: &str,
    ) -> Result<Self, NetworkError> {
        let device_config = device_manager.config()?;
        
        // Try to load existing network keys
        if let Some(keys) = Self::load_network_keys(&device_config.id, password)? {
            return Ok(Self {
                device_id: device_config.id,
                public_key: keys.public_key,
                private_key: keys.encrypted_private_key,
                device_name: device_config.name,
                network_fingerprint: NetworkFingerprint::from_device(
                    device_config.id, 
                    &keys.public_key
                ),
            });
        }
        
        // Generate new network keys if none exist
        let (public_key, private_key) = Self::generate_keys(password)?;
        let network_fingerprint = NetworkFingerprint::from_device(
            device_config.id, 
            &public_key
        );
        
        // Save keys persistently
        Self::save_network_keys(&device_config.id, &public_key, &private_key, password)?;
        
        Ok(Self {
            device_id: device_config.id,
            public_key,
            private_key,
            device_name: device_config.name,
            network_fingerprint,
        })
    }
    
    /// Load network keys from device-specific storage
    fn load_network_keys(
        device_id: &Uuid, 
        password: &str
    ) -> Result<Option<EncryptedNetworkKeys>, NetworkError> {
        // Keys stored in device-specific file: <data_dir>/network_keys.json
        // This ensures multiple Spacedrive instances share the same keys
        todo!("Load from persistent storage")
    }
    
    /// Save network keys to device-specific storage  
    fn save_network_keys(
        device_id: &Uuid,
        public_key: &PublicKey,
        private_key: &EncryptedPrivateKey,
        password: &str,
    ) -> Result<(), NetworkError> {
        // Store encrypted keys alongside device.json
        todo!("Save to persistent storage")
    }
}

pub struct MasterKey {
    /// User's master password derives this
    key_encryption_key: [u8; 32],
    
    /// Encrypted with key_encryption_key - NOW USES PERSISTENT DEVICE IDs
    device_private_keys: HashMap<Uuid, EncryptedPrivateKey>, // UUID not derived ID
}

/// Pairing process for new devices
pub struct PairingCode {
    /// Temporary shared secret
    secret: [u8; 32],
    
    /// Expires after 5 minutes
    expires_at: DateTime<Utc>,
    
    /// Visual representation (6 words from BIP39 wordlist)
    words: [String; 6],
}
```

**Integration Flow:**

```rust
// In Core initialization
impl Core {
    pub async fn init_networking(&mut self, password: &str) -> Result<()> {
        // Use existing device manager - NO separate identity creation
        let network_identity = NetworkIdentity::from_device_manager(
            &self.device, 
            password
        ).await?;
        
        let network = Network::new(network_identity, config).await?;
        self.network = Some(Arc::new(network));
        Ok(())
    }
}
```

**Key Benefits of This Approach:**

1. **Persistent Identity**: Device ID survives restarts, OS reinstalls (if backed up)
2. **Cross-Instance Consistency**: Multiple Spacedrive instances = same network identity
3. **Pairing Persistence**: Paired devices stay paired across restarts
4. **Migration Support**: Network identity travels with device backup/restore
5. **Debugging**: Easy to correlate network traffic with device logs

**Wire Protocol Changes:**

```rust
// Network messages now include persistent device UUID for correlation
#[derive(Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Persistent device UUID (for logs, correlation)
    pub device_id: Uuid,
    
    /// Network fingerprint (for wire protocol security)
    pub network_fingerprint: NetworkFingerprint,
    
    /// Message payload
    pub payload: MessagePayload,
    
    /// Cryptographic signature
    pub signature: Signature,
}
```

### 2. Connection Establishment

Abstract connection interface:

```rust
#[async_trait]
pub trait NetworkConnection: Send + Sync {
    /// Send data reliably
    async fn send(&mut self, data: &[u8]) -> Result<()>;
    
    /// Receive data
    async fn receive(&mut self) -> Result<Vec<u8>>;
    
    /// Stream a file efficiently
    async fn send_file(&mut self, path: &Path) -> Result<()>;
    
    /// Receive file stream
    async fn receive_file(&mut self, path: &Path) -> Result<()>;
    
    /// Get remote device info
    fn remote_device(&self) -> &DeviceInfo;
    
    /// Check if connection is alive
    fn is_connected(&self) -> bool;
}

/// Connection manager handles all transports
pub struct ConnectionManager {
    /// Our device identity
    identity: Arc<DeviceIdentity>,
    
    /// Active connections
    connections: Arc<RwLock<HashMap<DeviceId, Box<dyn NetworkConnection>>>>,
    
    /// Available transports
    transports: Vec<Box<dyn Transport>>,
}
```

### 3. Transport Implementations

#### Local P2P Transport

Using existing libraries:

```rust
/// Local network transport using mDNS + direct TCP/QUIC
pub struct LocalTransport {
    /// mDNS for discovery (using mdns crate)
    mdns: ServiceDiscovery,
    
    /// QUIC for connections (using quinn)
    quinn_endpoint: quinn::Endpoint,
}

impl LocalTransport {
    pub async fn new(identity: Arc<DeviceIdentity>) -> Result<Self> {
        // Setup mDNS service
        let mdns = ServiceDiscovery::new(
            "_spacedrive._tcp.local",
            identity.device_id.to_string(),
        )?;
        
        // Setup QUIC endpoint
        let config = quinn::ServerConfig::with_crypto(
            Arc::new(noise_crypto_config(identity))
        );
        
        let endpoint = quinn::Endpoint::server(
            config,
            "0.0.0.0:0".parse()? // Random port
        )?;
        
        Ok(Self { mdns, quinn_endpoint: endpoint })
    }
}
```

#### Internet Relay Transport

For NAT traversal and internet connectivity:

```rust
/// Internet transport via Spacedrive relay service
pub struct RelayTransport {
    /// WebSocket or QUIC connection to relay
    relay_client: RelayClient,
    
    /// Our registration with relay
    registration: RelayRegistration,
}

/// Relay protocol messages
pub enum RelayMessage {
    /// Register device with relay
    Register { 
        device_id: DeviceId,
        public_key: PublicKey,
        auth_token: String, // From Spacedrive account
    },
    
    /// Request connection to another device
    Connect { 
        target_device_id: DeviceId,
        offer: SessionOffer, // Crypto handshake
    },
    
    /// Relay data between devices
    Data {
        session_id: SessionId,
        encrypted_payload: Vec<u8>,
    },
}
```

### 4. Security Layer

Using Noise Protocol or similar:

```rust
/// Noise Protocol XX pattern for mutual authentication
pub struct NoiseSession {
    /// Handshake state
    handshake: snow::HandshakeState,
    
    /// Transport state (after handshake)
    transport: Option<snow::TransportState>,
}

impl NoiseSession {
    /// Initiator side
    pub fn initiate(
        local_key: &PrivateKey,
        remote_public_key: Option<&PublicKey>,
    ) -> Result<Self> {
        let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
        let builder = snow::Builder::new(params.parse()?);
        
        let handshake = builder
            .local_private_key(&local_key.to_bytes())
            .build_initiator()?;
            
        Ok(Self { handshake, transport: None })
    }
    
    /// Complete handshake and establish encrypted transport
    pub fn complete_handshake(&mut self) -> Result<()> {
        if self.handshake.is_handshake_finished() {
            self.transport = Some(self.handshake.into_transport_mode()?);
        }
        Ok(())
    }
}
```

## Library Choices

### Core Networking

1. **quinn** - QUIC implementation in Rust
   - Pros: Built-in encryption, multiplexing, modern protocol
   - Cons: Requires UDP, might have firewall issues
   - Use for: Local P2P, future direct internet

2. **tokio-tungstenite** - WebSocket for relay
   - Pros: Works everywhere, HTTP-based
   - Cons: TCP head-of-line blocking
   - Use for: Relay connections, fallback

3. **libp2p** - Full P2P stack (alternative)
   - Pros: Complete solution, many transports
   - Cons: Complex, large dependency
   - Consider for: Future enhancement

### Discovery

1. **mdns** - mDNS/DNS-SD implementation
   - Pros: Simple, works on all platforms
   - Use for: Local device discovery

2. **if-watch** - Network interface monitoring
   - Pros: Detect network changes
   - Use for: Adaptive transport selection

### Security

1. **snow** - Noise Protocol Framework
   - Pros: Modern, simple, well-tested
   - Use for: Transport encryption

2. **ring** or **rustls** - Crypto primitives
   - Pros: Fast, audited
   - Use for: Key generation, signatures

### Utilities

1. **async-stream** - File streaming
   - Use for: Efficient file transfer

2. **backoff** - Retry logic
   - Use for: Connection resilience

## Connection Flow

### Local Network

```rust
async fn connect_local(target: DeviceId) -> Result<Connection> {
    // 1. Discover via mDNS
    let services = mdns.discover_services().await?;
    let target_service = services
        .iter()
        .find(|s| s.device_id == target)
        .ok_or("Device not found")?;
    
    // 2. Connect via QUIC
    let connection = quinn_endpoint
        .connect(target_service.addr, &target_service.name)?
        .await?;
    
    // 3. Noise handshake
    let noise = NoiseSession::initiate(&identity.private_key, None)?;
    perform_handshake(&mut connection, noise).await?;
    
    // 4. Verify device identity
    verify_remote_device(&connection, target)?;
    
    Ok(Connection::Local(connection))
}
```

### Internet via Relay

```rust
async fn connect_relay(target: DeviceId) -> Result<Connection> {
    // 1. Connect to relay server
    let relay = RelayClient::connect("relay.spacedrive.com").await?;
    
    // 2. Authenticate with relay
    relay.authenticate(&identity, auth_token).await?;
    
    // 3. Request connection to target
    let session = relay.connect_to(target).await?;
    
    // 4. Noise handshake through relay
    let noise = NoiseSession::initiate(&identity.private_key, None)?;
    perform_relayed_handshake(&relay, session, noise).await?;
    
    Ok(Connection::Relay(relay, session))
}
```

## File Transfer Protocol

Efficient file streaming over any transport:

```rust
/// File transfer header
pub struct FileHeader {
    /// File name
    pub name: String,
    
    /// Total size in bytes
    pub size: u64,
    
    /// Blake3 hash for verification
    pub hash: [u8; 32],
    
    /// Optional: Resume from offset
    pub resume_offset: Option<u64>,
}

/// Stream file over connection
async fn stream_file(
    conn: &mut dyn NetworkConnection,
    path: &Path,
) -> Result<()> {
    let file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    
    // Send header
    let header = FileHeader {
        name: path.file_name().unwrap().to_string(),
        size: metadata.len(),
        hash: calculate_hash(path).await?,
        resume_offset: None,
    };
    
    conn.send(&serialize(&header)?).await?;
    
    // Stream chunks
    let mut reader = BufReader::new(file);
    let mut buffer = vec![0u8; 1024 * 1024]; // 1MB chunks
    
    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 { break; }
        
        conn.send(&buffer[..n]).await?;
    }
    
    Ok(())
}
```

## Sync Protocol Integration

The sync protocol from the previous design runs over these connections:

```rust
impl NetworkConnection {
    /// High-level sync operations
    pub async fn sync_pull(
        &mut self,
        from_seq: u64,
        limit: Option<usize>,
    ) -> Result<PullResponse> {
        // Send request
        let request = PullRequest { from_seq, limit };
        self.send(&serialize(&request)?).await?;
        
        // Receive response
        let response_data = self.receive().await?;
        let response: PullResponse = deserialize(&response_data)?;
        
        Ok(response)
    }
}
```

## API Design

Simple, transport-agnostic API:

```rust
/// Main networking interface
pub struct Network {
    manager: Arc<ConnectionManager>,
}

impl Network {
    /// Connect to a device (auto-selects transport)
    pub async fn connect(&self, device_id: DeviceId) -> Result<DeviceConnection> {
        // Try local first
        if let Ok(conn) = self.manager.connect_local(device_id).await {
            return Ok(DeviceConnection::new(conn));
        }
        
        // Fall back to relay
        self.manager.connect_relay(device_id).await
            .map(DeviceConnection::new)
    }
    
    /// Share file with device
    pub async fn share_file(
        &self,
        device_id: DeviceId,
        file_path: &Path,
    ) -> Result<()> {
        let mut conn = self.connect(device_id).await?;
        conn.send_file(file_path).await
    }
    
    /// Sync with device
    pub async fn sync_with(
        &self,
        device_id: DeviceId,
        from_seq: u64,
    ) -> Result<Vec<SyncLogEntry>> {
        let mut conn = self.connect(device_id).await?;
        let response = conn.sync_pull(from_seq, Some(1000)).await?;
        Ok(response.changes)
    }
}
```

## Security Considerations

### Device Pairing

1Password-style pairing flow:

```rust
/// On device A (has master key)
async fn initiate_pairing() -> Result<PairingCode> {
    let secret = generate_random_bytes(32);
    let code = PairingCode::from_secret(&secret);
    
    // Display code.words to user
    println!("Pairing code: {}", code.words.join(" "));
    
    // Listen for pairing requests
    pairing_listener.register(code.clone()).await;
    
    Ok(code)
}

/// On device B (new device)
async fn complete_pairing(words: Vec<String>) -> Result<()> {
    let code = PairingCode::from_words(&words)?;
    
    // Connect to device A
    let conn = discover_and_connect_pairing_device().await?;
    
    // Exchange keys using pairing secret
    let shared_key = derive_key_from_secret(&code.secret);
    
    // Send our public key encrypted
    let encrypted_key = encrypt(&identity.public_key, &shared_key);
    conn.send(&encrypted_key).await?;
    
    // Receive encrypted master key
    let encrypted_master = conn.receive().await?;
    let master_key = decrypt(&encrypted_master, &shared_key)?;
    
    // Save master key locally
    save_master_key(master_key).await?;
    
    Ok(())
}
```

### Encryption Everywhere

- All connections use Noise Protocol XX pattern
- Forward secrecy with ephemeral keys
- No plaintext data ever transmitted
- File chunks encrypted individually

### Trust Model

- Trust on first use (TOFU) for device keys
- Optional key verification via pairing codes
- Devices can be revoked by removing from master key

## Performance Optimizations

### Connection Pooling

```rust
impl ConnectionManager {
    /// Reuse existing connections
    async fn get_or_connect(&self, device_id: DeviceId) -> Result<Connection> {
        // Check pool first
        if let Some(conn) = self.connections.read().await.get(&device_id) {
            if conn.is_connected() {
                return Ok(conn.clone());
            }
        }
        
        // Create new connection
        let conn = self.connect_new(device_id).await?;
        self.connections.write().await.insert(device_id, conn.clone());
        Ok(conn)
    }
}
```

### Adaptive Transport

```rust
/// Choose best transport based on conditions
async fn select_transport(target: DeviceId) -> Transport {
    // Same network? Use local
    if is_same_network(target).await {
        return Transport::Local;
    }
    
    // Has public IP? Try direct
    if has_public_ip(target).await {
        return Transport::Direct;
    }
    
    // Otherwise use relay
    Transport::Relay
}
```

## Future Enhancements

### WebRTC DataChannels
- For browser support
- Better NAT traversal
- Built-in STUN/TURN

### Bluetooth Support
- For mobile devices
- Low power scenarios
- Offline sync

### Tor Integration
- Anonymous connections
- Privacy-focused users
- Hidden service support

## Implementation Priority

1. **Phase 1**: Local P2P with mDNS + QUIC
2. **Phase 2**: Relay service with WebSocket
3. **Phase 3**: File transfer protocol
4. **Phase 4**: Sync protocol integration
5. **Phase 5**: Advanced features (WebRTC, etc.)

## Library Comparison Matrix

### Full Stack Solutions

| Library | Pros | Cons | Best For |
|---------|------|------|----------|
| **libp2p** | • Complete P2P stack<br>• Multiple transports<br>• DHT, gossip, etc<br>• Battle-tested | • Large & complex<br>• Opinionated design<br>• Learning curve<br>• Heavy dependencies | Full decentralized P2P |
| **iroh** | • Built for sync<br>• QUIC-based<br>• Content addressing<br>• Modern Rust | • Young project<br>• Limited docs<br>• Specific use case | Content-addressed sync |
| **Magic Wormhole** | • Simple pairing<br>• E2E encrypted<br>• No account needed | • One-time transfers<br>• Not persistent<br>• Limited protocol | Simple file sharing |

### Transport Libraries

| Library | Pros | Cons | Best For |
|---------|------|------|----------|
| **quinn** | • Pure Rust QUIC<br>• Fast & modern<br>• Multiplexing<br>• Built-in crypto | • UDP only<br>• Firewall issues<br>• Newer protocol | Local network, future |
| **tokio-tungstenite** | • WebSocket<br>• Works everywhere<br>• Simple API<br>• HTTP-based | • TCP limitations<br>• No multiplexing<br>• Text/binary only | Relay connections |
| **tarpc** | • RPC framework<br>• Multiple transports<br>• Type-safe | • RPC-focused<br>• Not streaming<br>• Overhead | Control protocol |

### Discovery Libraries

| Library | Pros | Cons | Best For |
|---------|------|------|----------|
| **mdns** | • Simple mDNS<br>• Cross-platform<br>• Lightweight | • Local only<br>• Basic features | Local discovery |
| **libp2p-mdns** | • Part of libp2p<br>• More features | • Requires libp2p<br>• Heavier | If using libp2p |
| **bonjour** | • Full Bonjour<br>• Apple native | • Platform specific<br>• Complex | macOS/iOS native |

### Security Libraries

| Library | Pros | Cons | Best For |
|---------|------|------|----------|
| **snow** | • Noise Protocol<br>• Simple API<br>• Well-tested<br>• Modern crypto | • Just crypto<br>• No networking | Our choice ✓ |
| **rustls** | • TLS in Rust<br>• Fast<br>• Audited | • Certificate based<br>• Complex setup | HTTPS/TLS needs |
| **sodiumoxide** | • libsodium wrapper<br>• Many primitives | • C dependency<br>• Lower level | Crypto primitives |

## Recommended Stack

Based on the analysis, here's the recommended combination:

### Core Stack
```toml
[dependencies]
# Transport
quinn = "0.10"              # QUIC for local/direct connections
tokio-tungstenite = "0.20"  # WebSocket for relay fallback

# Discovery  
mdns = "3.0"                # Local network discovery
if-watch = "3.0"            # Network monitoring

# Security
snow = "0.9"                # Noise Protocol encryption
ring = "0.16"               # Crypto primitives
argon2 = "0.5"              # Password derivation

# Utilities
tokio = { version = "1.0", features = ["full"] }
async-stream = "0.3"        # File streaming
backoff = "0.4"             # Retry logic
serde = "1.0"               # Serialization
bincode = "1.5"             # Efficient encoding
```

### Why This Stack?

1. **quinn + tokio-tungstenite**
   - Covers all transport needs
   - QUIC for performance, WebSocket for compatibility
   - Both well-maintained

2. **mdns**
   - Simple and sufficient for local discovery
   - No need for complex libp2p stack

3. **snow**
   - Perfect fit for our security needs
   - Simpler than TLS
   - Better than rolling our own

4. **Minimal Dependencies**
   - Each library does one thing well
   - Total control over protocol
   - Easy to understand and debug

### Alternative: libp2p-based

If we wanted a more complete solution:

```toml
[dependencies]
libp2p = { version = "0.53", features = [
    "tcp",
    "quic",
    "mdns",
    "noise",
    "yamux",
    "request-response",
    "kad",
    "gossipsub",
    "identify",
]}
```

Pros:
- Everything included
- Proven P2P patterns
- DHT for device discovery
- NAT traversal built-in

Cons:
- Much larger dependency
- Harder to customize
- More complex to debug
- Overkill for our needs

## Implementation Complexity

### Minimal Viable Implementation (2-3 weeks)
```rust
// Just local network support
struct SimpleNetwork {
    mdns: mdns::Service,
    quinn: quinn::Endpoint,
    connections: HashMap<DeviceId, quinn::Connection>,
}

// Basic operations
impl SimpleNetwork {
    async fn connect(&mut self, device_id: DeviceId) -> Result<()>;
    async fn send_file(&mut self, device_id: DeviceId, path: &Path) -> Result<()>;
}
```

### Full Implementation (6-8 weeks)
- Local P2P ✓
- Relay service ✓
- Encryption ✓
- File transfer ✓
- Sync protocol ✓
- Connection pooling ✓
- Auto-reconnect ✓

### With libp2p (4-6 weeks)
- Faster initial development
- But more time debugging/customizing
- Less control over protocol

## Conclusion

This design provides a flexible, secure networking layer that abstracts transport details from the application. By leveraging existing libraries like quinn, mdns, and snow, we minimize implementation complexity while maintaining full control over the protocol design. The transport-agnostic API ensures we can add new connection methods without changing application code.

The recommended stack balances simplicity with capability, avoiding the complexity of full P2P frameworks while still providing all needed functionality. This approach lets us ship a working solution quickly and iterate based on real usage.