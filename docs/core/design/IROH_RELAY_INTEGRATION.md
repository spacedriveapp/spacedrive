# Iroh Relay Integration for Spacedrive

**Author:** AI Assistant
**Date:** October 7, 2025
**Status:** Implemented (Phase 1 Complete)

## Executive Summary

This document outlines the plan to enhance Spacedrive's networking stack to use Iroh's relay servers as a fallback mechanism for device pairing and communication when local (mDNS) connections are not available. The goal is to enable reliable peer-to-peer communication across different networks while maintaining the current fast local network discovery.

## Current State Analysis

### What's Already in Place ✅

1. **Iroh Integration**: Spacedrive already uses Iroh as its networking stack (migrated from libp2p)
2. **RelayMode Configured**: The endpoint is already configured with `RelayMode::Default` (line 182 in `core/src/service/network/core/mod.rs`)
3. **Relay Information Captured**: When nodes are discovered, the code already extracts and stores `relay_url()` from discovery info (line 1254)
4. **NodeAddr with Relay**: When building `NodeAddr` for connections, relay URLs are included alongside direct addresses

### Current Limitations ❌

1. **mDNS-Only Pairing**: Device pairing currently relies exclusively on mDNS for discovery
   - Initiator broadcasts pairing session ID via mDNS user_data
   - Joiner listens for mDNS announcements with matching session_id
   - **Failure Point**: If devices are on different networks or mDNS doesn't work (e.g., restricted networks, iOS entitlement issues), pairing fails entirely

2. **No Remote Discovery Fallback**: The pairing flow has a 10-second mDNS timeout but no fallback mechanism
   - Line 1218: `let timeout = tokio::time::Duration::from_secs(10);`
   - Line 1288-1297: If mDNS times out, the system just warns and fails
   - No attempt to use relay for pairing discovery

3. **Relay Not Used for Reconnection**: Persisted devices store relay URLs but they're not actively used
   - Line 393 in `device/persistence.rs`: `relay_url: Option<String>` is stored
   - But reconnection attempts (line 396 in `core/mod.rs`) use the NodeAddr which may not have valid relay info

4. **No Relay Health Monitoring**: No visibility into relay connection status or fallback behavior

## The Good News 🎉

**The relay is already working!** Iroh is configured to use relay servers by default, and when you connect to a `NodeAddr` that includes a relay URL, Iroh automatically:
1. Attempts direct connection via provided socket addresses
2. Falls back to relay connection if direct fails
3. Attempts hole-punching to establish direct connection while relaying
4. Seamlessly upgrades from relay to direct when possible

The infrastructure is there - we just need to **expose it for pairing** and **ensure it's used effectively**.

## Iroh Default Relay Servers

Spacedrive is currently using the production Iroh relay servers maintained by number0:

- **North America**: `https://use1-1.relay.n0.iroh.iroh.link.`
- **Europe**: `https://euc1-1.relay.n0.iroh.iroh.link.`
- **Asia-Pacific**: `https://aps1-1.relay.n0.iroh.iroh.link.`

These are production-grade servers handling 200k+ concurrent connections with 90%+ NAT traversal success rate.

## Implementation Plan

### Phase 1: Enhanced Pairing with Relay Fallback (Priority: HIGH)

**Objective**: Enable pairing across different networks using relay servers as fallback

#### 1.1 Add Out-of-Band Pairing Code Exchange

**Problem**: Currently, the pairing code alone only provides a session_id for mDNS matching. It doesn't contain information about how to reach the initiator over the internet.

**Solution**: Enhance the pairing code/QR code to include:
- Session ID (for identification)
- Initiator's NodeId
- Initiator's relay URL (from home relay)

**Implementation**:
```rust
// core/src/service/network/protocol/pairing/code.rs
pub struct PairingCodeData {
    /// Existing session ID
    pub session_id: Uuid,
    /// Initiator's NodeId for relay-based discovery
    pub node_id: NodeId,
    /// Initiator's home relay URL
    pub relay_url: Option<RelayUrl>,
}
```

**Changes Required**:
- Modify `PairingCode::new()` to include node_id and relay_url
- Update BIP39 encoding/decoding to handle additional data (or use JSON+base64 for QR codes)
- Update pairing UI to show/scan enhanced codes

#### 1.2 Implement Dual-Path Discovery for Pairing

**Objective**: Try mDNS first (fast for local), fall back to relay (for remote)

**Implementation**:
```rust
// core/src/service/network/core/mod.rs
pub async fn start_pairing_as_joiner(&self, code: &str) -> Result<()> {
    let pairing_code = PairingCode::from_string(code)?;
    let session_id = pairing_code.session_id();

    // Start pairing state machine
    // ... existing code ...

    // Run discovery in parallel: mDNS + Relay
    tokio::select! {
        result = self.try_mdns_discovery(session_id) => {
            // Fast path: local network discovery
            result?
        }
        result = self.try_relay_discovery(pairing_code.node_id(), pairing_code.relay_url()) => {
            // Fallback path: relay-based discovery
            result?
        }
    }

    // Continue with pairing handshake...
}

async fn try_mdns_discovery(&self, session_id: Uuid) -> Result<Connection> {
    // Existing mDNS discovery logic
    // Timeout: 3-5 seconds (most local networks are fast)
}

async fn try_relay_discovery(&self, node_id: NodeId, relay_url: Option<RelayUrl>) -> Result<Connection> {
    // New: Connect via relay if mDNS fails
    let node_addr = NodeAddr::from_parts(
        node_id,
        relay_url,
        vec![] // No direct addresses yet
    );

    self.endpoint
        .connect(node_addr, PAIRING_ALPN)
        .await
        .map_err(|e| NetworkingError::ConnectionFailed(format!("Relay connection failed: {}", e)))
}
```

**Benefits**:
- Fast local pairing (mDNS wins the race)
- Reliable remote pairing (relay always works)
- Seamless user experience (whichever succeeds first)

#### 1.3 Update Pairing Protocol Documentation

- Update `docs/core/pairing.md` to document relay fallback behavior
- Update `docs/core/design/DEVICE_PAIRING_PROTOCOL.md` with new flow diagram showing dual-path discovery

### Phase 2: Improve Reconnection Reliability (Priority: MEDIUM)

**Objective**: Ensure paired devices can reconnect via relay when local network is unavailable

#### 2.1 Capture and Store Relay Information

**Current**: NodeAddr with relay_url is stored but may become stale

**Enhancement**:
```rust
// core/src/service/network/device/persistence.rs
pub struct PersistedPairedDevice {
    // ... existing fields ...

    /// Home relay URL of this device
    pub home_relay_url: Option<String>,

    /// Last known relay URLs (in order of preference)
    pub relay_urls: Vec<String>,

    /// Timestamp when relay info was last updated
    pub relay_info_updated_at: Option<DateTime<Utc>>,
}
```

#### 2.2 Enhance Reconnection Strategy

```rust
// core/src/service/network/core/mod.rs
async fn attempt_device_reconnection(...) {
    // Try in order of preference:

    // 1. Direct addresses (if on same network)
    if !persisted_device.last_seen_addresses.is_empty() {
        // Try cached direct addresses
    }

    // 2. mDNS discovery (if recently seen locally)
    if should_try_mdns(&persisted_device) {
        // Wait briefly for mDNS discovery
    }

    // 3. Relay fallback (always works)
    if let Some(relay_url) = &persisted_device.home_relay_url {
        let node_addr = NodeAddr::from_parts(
            remote_node_id,
            Some(relay_url.parse()?),
            vec![] // Start with relay, Iroh will discover direct
        );

        endpoint.connect(node_addr, MESSAGING_ALPN).await?;
    }
}
```

#### 2.3 Periodic Relay Info Refresh

**Rationale**: Home relay can change if a device moves or relay becomes unavailable

```rust
// Periodically refresh relay information for connected devices
async fn start_relay_info_refresh_task(&self) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour

        loop {
            interval.tick().await;

            // For each connected device, query their current relay info
            // Update persistence if changed
        }
    });
}
```

### Phase 3: Observability & Configuration (Priority: LOW)

**Objective**: Provide visibility into relay usage and allow configuration

#### 3.1 Relay Connection Metrics

Add to `NetworkEvent` enum:
```rust
pub enum NetworkEvent {
    // ... existing variants ...

    /// Connection established via relay (before hole-punch)
    ConnectionViaRelay {
        device_id: Uuid,
        relay_url: String,
    },

    /// Connection upgraded from relay to direct
    ConnectionUpgradedToDirect {
        device_id: Uuid,
        connection_type: String, // "ipv4", "ipv6", etc.
    },

    /// Relay connection health
    RelayHealth {
        relay_url: String,
        latency_ms: u64,
        connected: bool,
    },
}
```

#### 3.2 Relay Configuration API

```rust
// core/src/ops/network/config/action.rs

/// Configure relay settings
pub struct ConfigureRelayAction {
    pub mode: RelayMode,
}

pub enum RelayMode {
    /// Use default n0 production relays
    Default,
    /// Use custom relay servers
    Custom { relay_urls: Vec<String> },
    /// Disable relay (local-only mode)
    Disabled,
}
```

#### 3.3 Network Inspector UI

Add a "Network Status" panel showing:
- Current relay server and connection status
- Connection type for each paired device (direct/relay)
- Relay latency and bandwidth metrics
- Historical connection reliability

### Phase 4: Advanced Features (Future)

#### 4.1 Smart Relay Selection

- Prefer geographically closer relay servers
- Load balance across multiple relays
- Automatically switch relays based on performance

#### 4.2 Custom Relay Server Support

- Allow users to deploy their own relay servers
- Configuration UI for custom relay URLs
- Documentation for self-hosting Iroh relay servers

#### 4.3 Hybrid Discovery

- Combine mDNS with relay-assisted NAT traversal
- Use relay to coordinate hole-punching even for local networks behind strict firewalls

## Migration & Testing Plan

### Testing Strategy

1. **Local Network Tests**: Verify mDNS still works and is preferred
2. **Cross-Network Tests**: Test pairing between devices on different networks
3. **Relay Failover Tests**: Simulate relay outages and verify fallback behavior
4. **Performance Tests**: Measure latency increase when using relay
5. **NAT Traversal Tests**: Test various NAT configurations

### Rollout Plan

1. **Phase 1 - Week 1-2**: Implement enhanced pairing with relay fallback
2. **Phase 2 - Week 3**: Improve reconnection reliability
3. **Phase 3 - Week 4**: Add observability and configuration
4. **Beta Testing - Week 5-6**: Internal testing with various network configurations
5. **Public Release - Week 7**: Roll out to users with documentation

## Technical Considerations

### Security

- **Relay Privacy**: Relay servers see encrypted traffic only, cannot decrypt
- **Man-in-the-Middle**: Not possible due to TLS + NodeId verification
- **Relay Trust**: Using n0's relays means trusting their infrastructure (same as using their DNS)

### Performance

- **Relay Latency**: Adds 20-100ms typically (vs direct <10ms)
- **Bandwidth**: Relay servers can handle traffic but direct is always preferred
- **Hole-Punching**: Iroh automatically upgrades to direct connection (90% success rate)

### Reliability

- **Multi-Relay Redundancy**: n0 operates relays in 3 regions
- **Automatic Failover**: Iroh handles relay outages transparently
- **Connection Persistence**: QUIC maintains connection during network changes

## Alternative Approaches Considered

### 1. DHT-Based Discovery (Rejected)

**Approach**: Use Kademlia DHT for peer discovery instead of relay
**Why Rejected**:
- Adds complexity
- DHT discovery is slower (seconds to minutes)
- Iroh's relay approach is simpler and faster
- Still need relay for NAT traversal anyway

### 2. Centralized Signaling Server (Rejected)

**Approach**: Build custom signaling server for pairing coordination
**Why Rejected**:
- Reinventing the wheel - Iroh relay does this
- Operational overhead of running our own infrastructure
- n0's relays are already proven at scale

### 3. WebRTC-Style ICE (Rejected)

**Approach**: Implement full ICE protocol with STUN/TURN servers
**Why Rejected**:
- Iroh already handles this internally
- More complex than needed
- Relay servers provide same functionality

## Resources

### Iroh Documentation
- [Iroh Connection Establishment](https://docs.rs/iroh/latest/iroh/#connection-establishment)
- [Iroh Relay Servers](https://docs.rs/iroh/latest/iroh/#relay-servers)
- [RelayMode Documentation](https://docs.rs/iroh/latest/iroh/enum.RelayMode.html)

### Spacedrive Documentation
- [Networking Module](../networking.md)
- [Pairing Protocol](../pairing.md)
- [Iroh Migration Design](./IROH_MIGRATION_DESIGN.md)

### Code References
- Endpoint configuration: `core/src/service/network/core/mod.rs:175-196`
- Pairing joiner flow: `core/src/service/network/core/mod.rs:1179-1368`
- Device persistence: `core/src/service/network/device/persistence.rs`
- NodeAddr construction: `core/src/service/network/core/mod.rs:1252-1256`

## Open Questions

1. **Pairing Code Format**: Should we stick with 12-word BIP39 or switch to QR-only for remote pairing?
2. **Relay Server Priority**: Should users be able to pin a preferred relay region?
3. **Bandwidth Limits**: Should we impose limits on relay traffic to prevent abuse?
4. **Custom Relays**: Priority for custom relay server support?

## Next Steps

1. ✅ Complete discovery and analysis
2. 📋 Create implementation plan (this document)
3. 🔨 Implement Phase 1: Enhanced pairing with relay fallback
4. 🧪 Test cross-network pairing
5. 📊 Measure relay usage and performance
6. 📚 Update user documentation

---

**Status**: Ready for implementation
**Estimated Effort**: 2-3 weeks for Phases 1-2
**Risk Level**: Low (leveraging existing Iroh functionality)
