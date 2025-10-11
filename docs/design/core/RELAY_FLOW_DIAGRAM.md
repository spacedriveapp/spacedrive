<!--CREATED: 2025-10-11-->
# Relay Integration Flow Diagrams

## Current State: mDNS-Only Pairing

```
┌─────────────────────────────────────────────────────────────────────┐
│                        SAME NETWORK (Works)                       │
└─────────────────────────────────────────────────────────────────────┘

Initiator                                                    Joiner
─────────                                                    ──────

1. Generate pairing code
   └─> "word1 word2 ... word12"

2. Start pairing session                               3. Enter code
   └─> Broadcast session_id                               └─> Parse session_id
       via mDNS user_data
                                                         4. Listen for mDNS
3. Wait for connection          <─────────────────────     └─> Find session_id
                                     mDNS Discovery             in broadcasts
                                        (~1s)
                                                         5. Connect via direct
4. Accept connection            <─────────────────────     socket addresses
                                   QUIC Connection

5. Challenge-response handshake ←───────────────────→  6. Sign challenge
                                      Pairing

SUCCESS: Devices paired!


┌─────────────────────────────────────────────────────────────────────┐
│                    DIFFERENT NETWORKS (Fails)                     │
└─────────────────────────────────────────────────────────────────────┘

Initiator (Network A)                              Joiner (Network B)
─────────────────────                              ──────────────────

1. Generate pairing code
   └─> "word1 word2 ... word12"

2. Start pairing session                               3. Enter code
   └─> Broadcast session_id                               └─> Parse session_id
       via mDNS (local only!)
                                                         4. Listen for mDNS
3. Wait for connection          ╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳╳     └─> Timeout after 10s
                                  mDNS blocked by              (different network)
                                  network boundary
                                                         5. ERROR: Discovery failed
FAILURE: Pairing failed!


Note: Even though endpoint has RelayMode::Default configured, the relay
      is never used because pairing code doesn't include relay info!
```

## Proposed State: Dual-Path Discovery

```
┌─────────────────────────────────────────────────────────────────────┐
│              SAME NETWORK (Faster via mDNS)                       │
└─────────────────────────────────────────────────────────────────────┘

Initiator                                                    Joiner
─────────                                                    ──────

1. Generate enhanced code                              3. Enter code
   └─> Include:                                           └─> Parse:
       • session_id                                           • session_id
       • node_id                                              • node_id
       • relay_url                                            • relay_url

2. Start pairing session                               4. Parallel discovery:
   ├─> Broadcast via mDNS                                 ├─> Listen for mDNS ✅
   └─> Connected to relay                                 └─> Try relay connect
       (already happens)
                                                         5. mDNS wins race!
3. Accept connection            <─────────────────────     └─> Connect via
                                   mDNS + Direct               direct address
                                      (~1s)

SUCCESS: Fast local pairing (no change to user experience)


┌─────────────────────────────────────────────────────────────────────┐
│         DIFFERENT NETWORKS (Works via Relay)                      │
└─────────────────────────────────────────────────────────────────────┘

Initiator (Network A)                              Joiner (Network B)
─────────────────────                              ──────────────────

1. Generate enhanced code                              3. Enter code
   └─> session_id + node_id + relay_url                  └─> Parse all fields

2. Start pairing session                               4. Parallel discovery:
   ├─> Broadcast via mDNS                                 ├─> Listen for mDNS ❌
   │   (won't reach Network B)                           │   (timeout ~3s)
   └─> Home relay: use1-1.relay...                       │
                                                         └─> Try relay connect ✅
                    Relay Server                             └─> Build NodeAddr:
                  ┌──────────────┐                               NodeAddr::from_parts(
  Connected to ───┤              │                                 node_id,
  relay as home   │   n0 Relay   │                                 relay_url,
                  │              │                                 []
                  └──────────────┘                               )

3. Incoming connection via relay                       5. Connect via relay
   └─> Relay forwards encrypted ←─────────────────────    └─> Connection succeeds!
       QUIC packets                    ~2-5s                  (~2-5s)

4. Challenge-response handshake ←───────────────────→  6. Sign challenge
                                  (over relay)

5. Upgrade to direct connection                        7. Hole-punching attempt
   ├─> Iroh attempts NAT traversal                        ├─> Exchange candidates
   └─> Success rate: ~90%                                 └─> Direct path found!

SUCCESS: Devices paired via relay, then upgraded to direct!


┌─────────────────────────────────────────────────────────────────────┐
│              RECONNECTION AFTER PAIRING                              │
└─────────────────────────────────────────────────────────────────────┘

Device A                                                   Device B
────────                                                   ────────

[Device info stored]:                              [Device info stored]:
• node_id                                          • node_id
• relay_url: use1-1.relay...                      • relay_url: euc1-1.relay...
• last_seen_addresses: [10.0.0.5:8080]           • last_seen_addresses: [...]
• session_keys                                     • session_keys

RECONNECTION ATTEMPT (NodeId rule: lower ID initiates)
───────────────────────────────────────────────────────

1. Try direct addresses first
   └─> [10.0.0.5:8080] Timeout
       (device moved networks)

2. Try mDNS discovery
   └─> Wait 2s for broadcast Not found
       (not on same network)

3. Fallback to relay ✅
   └─> NodeAddr::from_parts(
         device_b_node_id,
         Some(relay_url),        ← Stored relay
         vec![]
       )

4. Connect via relay                               5. Accept connection
   └─> Relay forwards packets ───────────────────>    └─> Recognize node_id
                                    ~100ms                 as paired device

6. Restore encrypted session                       7. Session restored
   └─> Use stored session_keys                        └─> Use stored keys

8. Attempt hole-punch                              9. Coordinate NAT traversal
   └─> If successful, upgrade to direct            └─> Direct path established

SUCCESS: Reconnected via relay, upgraded to direct


┌─────────────────────────────────────────────────────────────────────┐
│                    CONNECTION LIFECYCLE                              │
└─────────────────────────────────────────────────────────────────────┘

                    ┌──────────────┐
                    │   Discovery  │
                    └───────┬──────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
        ┌─────▼──────┐            ┌──────▼─────┐
        │   mDNS     │            │   Relay    │
        │ (Local)    │            │  (Remote)  │
        └─────┬──────┘            └──────┬─────┘
              │                           │
              └─────────────┬─────────────┘
                            │
                    ┌───────▼────────┐
                    │   Connection   │ ← Whichever succeeds first
                    │   Established  │
                    └───────┬────────┘
                            │
                    ┌───────▼────────┐
                    │ Relay Transit  │ ← If via relay
                    └───────┬────────┘
                            │
                    ┌───────▼────────┐
                    │  Hole-Punch    │ ← Automatic upgrade attempt
                    │   Attempt      │    (90% success)
                    └───────┬────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
        ┌─────▼──────┐            ┌──────▼─────┐
        │   Direct   │            │   Relay    │
        │ Connection │            │ Connection │
        │  (<10ms)   │            │  (~100ms)  │
        └────────────┘            └────────────┘

        Optimal                 Fallback
           (90% of cases)            (Always works)


┌─────────────────────────────────────────────────────────────────────┐
│                    RELAY SERVER TOPOLOGY                             │
└─────────────────────────────────────────────────────────────────────┘

                    ┌────────────────────┐
                    │  Device A (EU)     │
                    │  Home: eu relay    │
                    └──────────┬─────────┘
                               │
                               │ Connects to home relay
                               │
         ┌─────────────────────┼─────────────────────┐
         │                     │                     │
   ┌─────▼──────┐      ┌──────▼──────┐      ┌──────▼──────┐
   │ NA Relay   │      │  EU Relay   │      │  AP Relay   │
   │ use1-1...  │◄────►│  euc1-1...  │◄────►│  aps1-1...  │
   └─────┬──────┘      └──────┬──────┘      └──────┬──────┘
         │                     │                     │
         └─────────────────────┼─────────────────────┘
                               │
                               │ Relay forwards
                               │ encrypted packets
                               │
                    ┌──────────▼─────────┐
                    │  Device B (NA)     │
                    │  Home: na relay    │
                    └────────────────────┘

• Devices connect to geographically closest relay (automatic)
• Relays coordinate to forward packets
• Can only see encrypted QUIC traffic
• Relays assist with hole-punching via STUN/TURN-like protocol
```

## Key Implementation Points

### 1. Enhanced Pairing Code Structure

```rust
// BEFORE (current)
PairingCode {
    entropy: [u8; 16],  // Only has session_id info
}

// AFTER (proposed)
PairingCode {
    session_id: Uuid,        // For mDNS matching
    node_id: NodeId,         // For relay discovery
    relay_url: Option<RelayUrl>,  // Initiator's home relay
}

// Encoding options:
// Option A: Extended BIP39 (24 words instead of 12)
// Option B: JSON + Base64 in QR code (not human-readable)
// Option C: Hybrid: Show QR, fallback to manual 24-word entry
```

### 2. Discovery Implementation

```rust
// core/src/service/network/core/mod.rs

pub async fn start_pairing_as_joiner(&self, code: &str) -> Result<()> {
    let pairing_code = PairingCode::from_string(code)?;

    // Create both discovery futures
    let mdns_future = self.try_mdns_discovery(pairing_code.session_id());
    let relay_future = self.try_relay_discovery(
        pairing_code.node_id(),
        pairing_code.relay_url()
    );

    // Race them - whichever succeeds first wins
    let connection = tokio::select! {
        Ok(conn) = mdns_future => {
            self.logger.info("Connected via mDNS (local network)").await;
            conn
        }
        Ok(conn) = relay_future => {
            self.logger.info("Connected via relay (remote network)").await;
            conn
        }
    };

    // Continue with pairing handshake using the established connection
    // ... existing pairing logic ...
}
```

### 3. Relay Info Storage

```rust
// core/src/service/network/device/persistence.rs

pub struct PersistedPairedDevice {
    pub device_info: DeviceInfo,
    pub session_keys: SessionKeys,
    pub paired_at: DateTime<Utc>,

    // Enhanced fields for relay support
    pub home_relay_url: Option<String>,      // ← Add this
    pub last_known_relay: Option<String>,    // ← Add this
    pub last_seen_addresses: Vec<String>,    // ← Already exists

    // Connection history
    pub last_connected_at: Option<DateTime<Utc>>,
    pub connection_attempts: u32,
    pub trust_level: TrustLevel,
}
```

---

**Implementation Timeline**

```
Week 1: Pairing code enhancement + dual-path discovery
Week 2: Testing cross-network scenarios + bug fixes
Week 3: Reconnection improvements + relay info storage
Week 4: Observability + metrics + documentation
Week 5-6: Beta testing with various network configs
Week 7: Production rollout
```

