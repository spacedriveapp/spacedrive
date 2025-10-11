# Iroh Relay Integration - Quick Summary

## TL;DR

**Good News**: Spacedrive already uses Iroh with relay servers configured! The relay infrastructure is working - we just need to expose it for pairing and ensure it's used effectively.

**Key Finding**: Your relay is already set to `RelayMode::Default` (line 182 in `core/src/service/network/core/mod.rs`), which means paired devices can already connect via relay. The main gap is **pairing discovery** which currently only uses mDNS.

## Current Architecture

```
Device A (Same Network)          Device B
    |                                |
    |-------- mDNS Discovery ------->| Works great!
    |<------- Connection ----------->|
    |                                |

Device A (Different Network)     Device B
    |                                |
    |-------- mDNS Discovery ------->| Times out (10s)
    |                                | Pairing fails
    X                                X
```

## Proposed Architecture

```
Device A (Any Network)           Device B (Any Network)
    |                                |
    ├------- mDNS Discovery -------->| Fast path (local)
    |                                |
    └------- Relay Discovery ------->| Fallback (remote)
    |        (via n0 relays)         |
    |                                |
    |<======= Connection ===========>| Always works!
         (direct or via relay)
```

## What's Already Working

1. **Iroh Integration**: Using Iroh instead of libp2p
2. **Relay Configured**: `RelayMode::Default` set
3. **Default Relays**: Using n0's production servers (NA, EU, AP)
4. **Relay in NodeAddr**: Relay URLs stored when available
5. **Automatic Fallback**: Iroh handles relay direct transitions

## Current Limitations

### 1. Pairing Discovery (Main Issue)

**File**: `core/src/service/network/core/mod.rs:1179-1368`

```rust
pub async fn start_pairing_as_joiner(&self, code: &str) -> Result<()> {
    // Only uses mDNS discovery
    let mut discovery_stream = endpoint.discovery_stream();
    let timeout = Duration::from_secs(10);  // Fails after 10s

    // No fallback to relay!
}
```

**Impact**: Devices on different networks cannot pair.

### 2. Reconnection Strategy

**File**: `core/src/service/network/core/mod.rs:300-446`

Reconnection uses stored NodeAddr but doesn't actively refresh relay info.

### 3. No Visibility

No events/metrics for relay usage, fallback behavior, or connection types.

## Implementation Priority

### Phase 1: Pairing Fallback (MUST HAVE) 

**Effort**: 1-2 weeks
**Impact**: HIGH - Enables cross-network pairing

1. Enhance pairing code to include initiator's NodeId + relay URL
2. Implement dual-path discovery (mDNS + relay)
3. Update pairing UI for enhanced codes

### Phase 2: Reconnection (SHOULD HAVE) 

**Effort**: 1 week
**Impact**: MEDIUM - Improves reliability

1. Store and refresh relay information
2. Enhance reconnection strategy with relay fallback
3. Periodic relay info updates

### Phase 3: Observability (NICE TO HAVE) 

**Effort**: 1 week
**Impact**: LOW - Developer visibility

1. Add relay metrics and events
2. Network inspector UI
3. Connection type indicators

## Key Code Locations

### Networking Core
- **Endpoint Setup**: `core/src/service/network/core/mod.rs:159-203`
- **Pairing Joiner**: `core/src/service/network/core/mod.rs:1179-1368`
- **Reconnection**: `core/src/service/network/core/mod.rs:300-446`

### Pairing Protocol
- **Pairing Code**: `core/src/service/network/protocol/pairing/code.rs`
- **NodeAddr Serialization**: `core/src/service/network/protocol/pairing/types.rs:385-437`

### Device Persistence
- **Storage**: `core/src/service/network/device/persistence.rs:19-29`
- **Registry**: `core/src/service/network/device/registry.rs`

### Iroh Configuration (Reference)
- **RelayMode**: `iroh/src/endpoint.rs:2206-2229`
- **Defaults**: `iroh/src/defaults.rs:20-121`

## Relay Servers (Already Configured)

```rust
// Production servers (from iroh/src/defaults.rs)
NA: https://use1-1.relay.n0.iroh.iroh.link.
EU: https://euc1-1.relay.n0.iroh.iroh.link.
AP: https://aps1-1.relay.n0.iroh.iroh.link.
```

These are production-grade, handling 200k+ concurrent connections.

## Testing Checklist

- [ ] Local pairing (mDNS) still fast and preferred
- [ ] Cross-network pairing works via relay
- [ ] Connection upgrades from relay to direct
- [ ] Reconnection works across networks
- [ ] Relay failover (simulate outage)
- [ ] Various NAT configurations
- [ ] iOS devices (mDNS entitlement issues)

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|---------|------------|
| Relay downtime | Low | High | Multi-region redundancy + automatic failover |
| Increased latency | High | Low | Automatic upgrade to direct (90% success) |
| Privacy concerns | Low | Medium | Relay only sees encrypted traffic |
| Implementation bugs | Medium | Medium | Comprehensive testing + gradual rollout |

## Performance Expectations

| Metric | Local (mDNS) | Remote (Relay) | Remote → Direct |
|--------|-------------|----------------|-----------------|
| Discovery time | <1s | 2-5s | N/A |
| Connection latency | <10ms | 20-100ms | <10ms |
| Hole-punch success | N/A | N/A | ~90% |
| Bandwidth overhead | None | Minimal | None |

## Questions for Discussion

1. **Pairing Code Format**: Keep 12-word BIP39 or switch to QR-only for remote pairing?
   - *BIP39 is human-readable but limited data capacity*
   - *QR codes can hold more data (NodeId + relay URL)*

2. **Custom Relays**: How important is self-hosting capability?
   - *Some users may want private relay servers*
   - *Adds operational complexity*

3. **Relay Selection**: Should users choose relay region?
   - *Lower latency for specific regions*
   - *More configuration complexity*

4. **Bandwidth Limits**: Should we limit relay traffic?
   - *Prevent abuse of n0's free relays*
   - *May impact legitimate use cases*

## Next Actions

1. Review this plan and provide feedback
2. Decide on pairing code format (BIP39 vs QR)
3. Implement Phase 1 (pairing fallback)
4. Test cross-network scenarios
5. Document for users

---

**See detailed plan**: [IROH_RELAY_INTEGRATION.md](./IROH_RELAY_INTEGRATION.md)

