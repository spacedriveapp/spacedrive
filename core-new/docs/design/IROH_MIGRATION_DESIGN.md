# Spacedrive Networking: libp2p to Iroh Migration Design

## Executive Summary

This document outlines the complete replacement of libp2p with Iroh for Spacedrive's networking module. Iroh offers significant advantages including:
- 90%+ NAT traversal success (vs libp2p's 70%)
- Simpler API with less configuration
- Built-in QUIC transport with encryption/multiplexing
- Production-proven with 200k+ concurrent connections
- Native mobile support (iOS/Android/ESP32)

## Current Architecture (libp2p)

```
Core → NetworkingService → Swarm<UnifiedBehaviour> → Protocols
                               ├── Kademlia DHT
                               ├── mDNS
                               └── Request/Response
```

## Target Architecture (Iroh)

```
Core → NetworkingService → iroh::Endpoint → Protocols
                               ├── Built-in Discovery
                               ├── QUIC Connections
                               └── Stream-based messaging
```

## Component Mapping

### Core Components

| libp2p Component | Iroh Replacement | Notes |
|-----------------|------------------|--------|
| `Swarm<UnifiedBehaviour>` | `iroh::Endpoint` | Single endpoint manages all connections |
| `PeerId` | `iroh::NodeId` | Ed25519-based identity |
| `Multiaddr` | `iroh::NodeAddr` | Simpler addressing scheme |
| `NetworkIdentity` | `iroh::SecretKey` | Direct key management |
| Kademlia DHT | Iroh discovery | Built-in peer discovery |
| mDNS | Iroh local discovery | Automatic local network discovery |
| TCP+Noise+Yamux | QUIC | All-in-one transport |
| Request/Response | Iroh streams | Bi/uni-directional streams |

### Protocol Migration

#### Pairing Protocol
- **Keep**: BIP39 word codes, challenge-response flow
- **Replace**: libp2p request/response → Iroh ALPN + streams
- **New**: Use Iroh's relay for better connectivity during pairing

#### File Transfer Protocol
- **Keep**: Chunking logic, encryption approach, progress tracking
- **Replace**: libp2p streams → Iroh QUIC streams
- **New**: Optional iroh-blobs for content-addressed storage

#### Messaging Protocol
- **Keep**: Message types and serialization
- **Replace**: libp2p messaging → iroh-gossip for pub/sub patterns
- **New**: Real-time capabilities with lower latency

## Implementation Plan

### Phase 1: Core Infrastructure

1. **Replace core networking module**
   ```
   src/services/networking/
   ├── core/           # Replace entirely with Iroh
   │   ├── mod.rs      # NetworkingService with iroh::Endpoint
   │   ├── discovery.rs # Iroh discovery
   │   ├── event_loop.rs # Simplified event handling
   │   └── behavior.rs  # Remove (not needed with Iroh)
   ```

2. **Update `NetworkingService`**
   ```rust
   pub struct NetworkingService {
       endpoint: iroh::Endpoint,
       identity: iroh::SecretKey,
       device_registry: Arc<RwLock<DeviceRegistry>>,
       protocol_registry: Arc<RwLock<ProtocolRegistry>>,
   }
   ```

3. **Port device identity**
   - Convert Ed25519 keypairs to Iroh format
   - Update device IDs to use NodeId

### Phase 2: Protocol Migration

1. **Pairing Protocol**
   - Replace libp2p request/response with Iroh streams
   - Use ALPN for protocol negotiation
   - Keep existing pairing flow logic

2. **File Transfer Protocol**
   - Replace libp2p streams with QUIC streams
   - Leverage Iroh's built-in progress tracking
   - Keep chunking and encryption logic

3. **Messaging Protocol**
   - Use iroh-gossip for pub/sub patterns
   - Maintain existing message types

### Phase 3: Testing & Validation

1. **Update Integration Tests**
   - Replace libp2p setup with Iroh
   - Test pairing flow end-to-end
   - Verify file transfer functionality

2. **Connection Management**
   - Port device state tracking
   - Implement Iroh-based reconnection
   - Add connection metrics

### Phase 4: Relay Configuration

1. **Spacedrive Cloud Relays**
   - Configure default relay URLs
   - Add custom relay support
   - Implement relay health checks

2. **Future Enhancements**
   - Browser support via WASM
   - Mobile optimizations
   - iroh-blobs integration

## Migration Strategy

### Direct Replacement
- Remove all libp2p dependencies and code
- Replace with Iroh implementation directly
- No feature flags or parallel implementations

### API Compatibility
The public `Core` API remains unchanged:
```rust
impl Core {
    pub async fn init_networking(&mut self) -> Result<()>
    pub async fn start_pairing_as_initiator(&self) -> Result<(String, u32)>
    pub async fn share_with_device(&mut self, ...) -> Result<Vec<TransferId>>
}
```

### Relay Configuration
```rust
pub struct NetworkingConfig {
    /// Spacedrive Cloud relay URLs
    pub default_relays: Vec<String>,
    
    /// User-configured custom relays
    pub custom_relays: Vec<String>,
    
    /// Run local relay for LAN-only setups
    pub enable_local_relay: bool,
}
```

## Benefits Post-Migration

1. **Improved Connectivity**: >90% connection success rate
2. **Simplified Codebase**: ~40% less networking code
3. **Better Performance**: QUIC reduces latency and overhead
4. **Platform Support**: Native mobile and browser support
5. **Future-Proof**: Active development and growing ecosystem

## Risk Mitigation

1. **Testing**: Update all integration tests to use Iroh
2. **Protocol Compatibility**: Keep message formats unchanged
3. **Identity Migration**: Preserve device IDs during conversion
4. **Documentation**: Update all networking docs

## Success Metrics

- Connection success rate: >90% (up from 70%)
- Time to first connection: <2s (from 3-5s)
- Code complexity: 40% reduction in LoC
- Test coverage: Maintain >80%
- User feedback: Improved reliability scores

## Detailed Implementation Plan

### Phase 1: Endpoint Migration (Foundation)

The first step is replacing the libp2p Swarm with Iroh's Endpoint. This is the foundation everything else builds on.

#### 1.1 Update NetworkingService Structure

**Current libp2p structure:**
```rust
// src/services/networking/core/mod.rs
pub struct NetworkingService {
    identity: NetworkIdentity,
    swarm: Swarm<UnifiedBehaviour>,
    protocol_registry: Arc<RwLock<ProtocolRegistry>>,
    device_registry: Arc<RwLock<DeviceRegistry>>,
    // ... channels
}
```

**New Iroh structure:**
```rust
// Replace the entire NetworkingService with Iroh-based implementation
pub struct NetworkingService {
    endpoint: iroh::Endpoint,
    identity: iroh::SecretKey,
    node_id: iroh::NodeId,
    protocol_registry: Arc<RwLock<ProtocolRegistry>>,
    device_registry: Arc<RwLock<DeviceRegistry>>,
    // ... simplified channels
}

impl NetworkingService {
    pub async fn new(
        identity: NetworkIdentity,
        device_manager: Arc<DeviceManager>,
    ) -> Result<Self> {
        // Convert existing Ed25519 keypair to Iroh format
        let secret_key = iroh::SecretKey::from_bytes(&identity.keypair_bytes())?;
        let node_id = secret_key.public();
        
        // Create Iroh endpoint with discovery and relay configuration
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key.clone())
            .alpns(vec![
                PAIRING_ALPN.to_vec(),
                FILE_TRANSFER_ALPN.to_vec(),
                MESSAGING_ALPN.to_vec(),
            ])
            .relay_mode(iroh::RelayMode::Default)
            .bind(0)
            .await?;
            
        // Start discovery (replaces mDNS + Kademlia)
        endpoint.discovery().add_discovery(Box::new(
            iroh::discovery::pkarr::PkarrPublisher::default()
        ));
        
        Ok(Self {
            endpoint,
            identity: secret_key,
            node_id,
            // ... rest of initialization
        })
    }
}
```

#### 1.2 Remove libp2p-specific files

These files can be completely deleted:
- `src/services/networking/core/behavior.rs` - No longer needed with Iroh
- `src/services/networking/core/swarm.rs` - Iroh handles transport internally
- `src/services/networking/core/discovery.rs` - Replaced by Iroh's discovery

#### 1.3 Simplify Event Loop

The event loop becomes much simpler with Iroh since it handles many things internally:

```rust
// src/services/networking/core/event_loop.rs
impl NetworkingEventLoop {
    pub async fn run(mut self) {
        loop {
            select! {
                // Handle incoming connections
                Some(conn) = self.endpoint.accept() => {
                    let conn = match conn.await {
                        Ok(c) => c,
                        Err(e) => {
                            warn!("Failed to accept connection: {}", e);
                            continue;
                        }
                    };
                    
                    // Route based on ALPN protocol
                    match conn.alpn() {
                        PAIRING_ALPN => self.handle_pairing_connection(conn).await,
                        FILE_TRANSFER_ALPN => self.handle_file_transfer(conn).await,
                        MESSAGING_ALPN => self.handle_messaging(conn).await,
                        _ => warn!("Unknown ALPN: {:?}", conn.alpn()),
                    }
                }
                
                // Handle commands from main thread
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                
                // Shutdown signal
                _ = self.shutdown_rx.recv() => {
                    info!("Shutting down networking");
                    break;
                }
            }
        }
    }
}
```

### Phase 2: Pairing Protocol (Critical Path)

This is the most complex protocol and will exercise the full Iroh API. Getting this right makes everything else straightforward.

#### 2.1 Define Pairing as Iroh Protocol

```rust
// src/services/networking/protocols/pairing/mod.rs

// Define ALPN for pairing protocol
pub const PAIRING_ALPN: &[u8] = b"spacedrive/pairing/1";

// The pairing handler now works with Iroh connections
impl PairingProtocolHandler {
    pub async fn handle_connection(&self, conn: iroh::Connection) {
        // Accept a bidirectional stream for pairing messages
        let (send, recv) = match conn.accept_bi().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to accept pairing stream: {}", e);
                return;
            }
        };
        
        // The existing pairing logic remains the same, just using Iroh streams
        self.handle_pairing_stream(send, recv, conn.remote_node_id()).await;
    }
    
    pub async fn initiate_pairing(&self, node_addr: NodeAddr) -> Result<()> {
        // Connect to the remote peer
        let conn = self.endpoint.connect(node_addr, PAIRING_ALPN).await?;
        
        // Open a bidirectional stream
        let (send, recv) = conn.open_bi().await?;
        
        // Run the pairing flow (existing logic)
        self.run_pairing_initiator(send, recv).await
    }
}
```

#### 2.2 Replace DHT Discovery with Iroh Discovery

The pairing discovery mechanism changes from Kademlia DHT to Iroh's discovery:

```rust
// src/services/networking/protocols/pairing/initiator.rs

impl PairingInitiator {
    pub async fn publish_pairing_session(&self, session: &PairingSession) -> Result<()> {
        // Create a discovery item for this pairing session
        let discovery_info = DiscoveryInfo {
            node_id: self.node_id,
            session_id: session.id,
            device_info: self.device_info.clone(),
            // Include relay info for better connectivity
            addresses: self.endpoint.node_addr().await?,
        };
        
        // Publish to Iroh's discovery system (replaces DHT PUT)
        self.endpoint
            .discovery()
            .publish(session.pairing_code.as_bytes(), &discovery_info)
            .await?;
            
        Ok(())
    }
}

// src/services/networking/protocols/pairing/joiner.rs
impl PairingJoiner {
    pub async fn discover_pairing_session(&self, code: &str) -> Result<NodeAddr> {
        // Query Iroh's discovery (replaces DHT GET)
        let discoveries = self.endpoint
            .discovery()
            .resolve(code.as_bytes())
            .await?;
            
        // Return the first valid discovery
        discoveries.into_iter().next()
            .ok_or(PairingError::SessionNotFound)
    }
}
```

#### 2.3 Update Pairing Messages for Streams

The pairing messages stay the same, but we send them over Iroh streams:

```rust
// src/services/networking/protocols/pairing/messages.rs

impl PairingMessage {
    /// Send a pairing message over an Iroh stream
    pub async fn send(&self, stream: &mut iroh::SendStream) -> Result<()> {
        let bytes = serde_cbor::to_vec(&self)?;
        let len = bytes.len() as u32;
        
        // Write length prefix
        stream.write_all(&len.to_be_bytes()).await?;
        // Write message
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        
        Ok(())
    }
    
    /// Receive a pairing message from an Iroh stream
    pub async fn recv(stream: &mut iroh::RecvStream) -> Result<Self> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Read message
        let mut bytes = vec![0u8; len];
        stream.read_exact(&mut bytes).await?;
        
        Ok(serde_cbor::from_slice(&bytes)?)
    }
}
```

### Phase 3: Update Device Management

#### 3.1 Replace PeerId with NodeId

```rust
// src/services/networking/device/mod.rs

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: Uuid,
    pub name: String,
    pub platform: Platform,
    pub node_id: iroh::NodeId,  // Was: peer_id: PeerId
    pub version: String,
}

// src/services/networking/device/registry.rs
pub struct DeviceRegistry {
    devices: HashMap<Uuid, DeviceEntry>,
    node_to_device: HashMap<iroh::NodeId, Uuid>,  // Was: PeerId -> Uuid
    // ... rest stays the same
}
```

### Phase 4: File Transfer Protocol

File transfer becomes simpler with Iroh's QUIC streams:

```rust
// src/services/networking/protocols/file_transfer.rs

pub const FILE_TRANSFER_ALPN: &[u8] = b"spacedrive/filetransfer/1";

impl FileTransferProtocolHandler {
    pub async fn send_file(
        &self,
        node_addr: NodeAddr,
        file_path: &Path,
        transfer_id: Uuid,
    ) -> Result<()> {
        // Connect with file transfer ALPN
        let conn = self.endpoint
            .connect(node_addr, FILE_TRANSFER_ALPN)
            .await?;
            
        // Open a unidirectional stream for data
        let mut send = conn.open_uni().await?;
        
        // Send transfer metadata first
        let metadata = TransferMetadata {
            id: transfer_id,
            filename: file_path.file_name().unwrap().to_string_lossy().to_string(),
            size: file_path.metadata()?.len(),
            // ... other metadata
        };
        metadata.send(&mut send).await?;
        
        // Stream the file data (existing chunking logic)
        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        
        while let Ok(n) = file.read(&mut buffer).await {
            if n == 0 { break; }
            
            // Encrypt chunk (existing logic)
            let encrypted = self.encrypt_chunk(&buffer[..n], &session_key)?;
            
            // Send over QUIC stream
            send.write_all(&encrypted).await?;
        }
        
        send.finish().await?;
        Ok(())
    }
}
```

### Phase 5: Update Identity Management

```rust
// src/services/networking/utils/identity.rs

pub struct NetworkIdentity {
    secret_key: iroh::SecretKey,
    node_id: iroh::NodeId,
    device_id: Uuid,  // Deterministic from key
}

impl NetworkIdentity {
    pub fn from_master_key(master_key: &MasterKey) -> Result<Self> {
        // Derive networking key from master (same as before)
        let key_bytes = derive_network_key(master_key);
        
        // Create Iroh identity
        let secret_key = iroh::SecretKey::from_bytes(&key_bytes)?;
        let node_id = secret_key.public();
        
        // Keep deterministic device ID generation
        let device_id = generate_device_id(&secret_key);
        
        Ok(Self {
            secret_key,
            node_id,
            device_id,
        })
    }
}
```

### Phase 6: Integration Testing

Update the integration tests to use Iroh:

```rust
// tests/test_core_pairing.rs

async fn spawn_test_node(name: &str) -> (Core, NodeAddr) {
    let mut core = create_test_core(name).await;
    core.init_networking().await.unwrap();
    
    // Get our node address for others to connect
    let node_addr = core.networking
        .as_ref()
        .unwrap()
        .endpoint
        .node_addr()
        .await
        .unwrap();
        
    (core, node_addr)
}
```

## Key Implementation Notes

1. **ALPN Protocol Negotiation**: Iroh uses ALPN (like HTTP/3) to negotiate protocols. Each protocol gets its own ALPN identifier.

2. **Stream Types**: Iroh provides both bidirectional and unidirectional streams. Use bi-streams for request/response patterns, uni-streams for one-way data transfer.

3. **Discovery**: Iroh's discovery system is pluggable. We can use the default Pkarr discovery or implement custom discovery.

4. **Relay Configuration**: Iroh automatically uses relays when direct connections fail. Configure Spacedrive relays for better control.

5. **Error Handling**: Iroh errors are more specific than libp2p's. Update error types accordingly.

6. **Testing**: Iroh works great in tests - no need for complex libp2p test setups.

## Conclusion

Replacing libp2p with Iroh will significantly improve Spacedrive's networking reliability while reducing code complexity. The direct replacement approach allows us to immediately benefit from Iroh's superior connectivity and simpler API.