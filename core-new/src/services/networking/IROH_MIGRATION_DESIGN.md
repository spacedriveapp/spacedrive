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

## Implementation Order

1. **Core Module**: Replace NetworkingService with Iroh
2. **Protocols**: Update pairing, file transfer, messaging
3. **Tests**: Update all integration tests
4. **Documentation**: Update networking docs

## Conclusion

Replacing libp2p with Iroh will significantly improve Spacedrive's networking reliability while reducing code complexity. The direct replacement approach allows us to immediately benefit from Iroh's superior connectivity and simpler API.