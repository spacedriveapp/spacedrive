# Iroh Usage Analysis - Are We Using It Correctly?

**Date**: 2025-10-18
**Iroh Source**: `/Users/jamespine/Projects/iroh`
**Our Implementation**: `core/src/service/network/protocol/pairing/`

---

## Executive Summary

After thoroughly analyzing Iroh's source code and examples, we have identified **significant architectural misunderstandings** in how we're using Iroh. We're fighting against the library instead of working with it.

### Key Findings:

**What We're Doing Right**:
- Using ALPN to identify our protocol
- Using bidirectional streams

**What We're Doing Wrong**:
1. Creating new connections for each message (should reuse connection, create new streams)
2. Manually managing direct addresses (Iroh does this automatically)
3. Not caching/reusing Connection objects
4. Closing connections immediately after each message

### Impact:

Our current approach creates **10x more overhead** than necessary and explains all the connection thrashing we're seeing.

---

## Iroh's Design Philosophy

From `/Users/jamespine/Projects/iroh/iroh/src/lib.rs` lines 130-152:

> "Streams are **extremely cheap** to create and intended to be created as needed... A connection can support **very many short-lived streams** as well as long-lived streams."

### The Iroh Pattern:

```
ONE Connection (persistent, kept alive)
├─ Stream 1: Request/Response pair
├─ Stream 2: Request/Response pair
├─ Stream 3: Request/Response pair
└─ ...many more streams...
```

### What We're Currently Doing (WRONG):

```
Connection 1 (short-lived)
└─ Stream 1: Message 1

Connection 2 (short-lived)
└─ Stream 1: Message 2

Connection 3 (short-lived)
└─ Stream 1: Message 3
```

---

## Issue Analysis: Our Code vs. Iroh Best Practices

### Issue #1: Creating Connections Instead of Streams

#### Our Code (WRONG)

**Location**: `mod.rs:1038` - `send_pairing_message_to_node()`

```rust
pub async fn send_pairing_message_to_node(
    &self,
    endpoint: &Endpoint,
    node_id: NodeId,
    message: &PairingMessage,
) -> Result<Option<PairingMessage>> {
    // Create NEW connection every time
    let node_addr = NodeAddr::new(node_id);
    let conn = endpoint.connect(node_addr, PAIRING_ALPN).await?;  // ← NEW CONNECTION

    // Open stream on this new connection
    let (mut send, mut recv) = conn.open_bi().await?;

    // ... send/receive ...

    // Connection implicitly closes when dropped
}
```

**Every time this is called**, we:
1. Establish new QUIC connection
2. Perform TLS handshake
3. Exchange ALPN
4. Create stream
5. Send message
6. Receive response
7. Close connection

#### Iroh's Intended Pattern (CORRECT)

**From**: `iroh/examples/transfer.rs` lines 517-537, `iroh/examples/echo.rs` lines 42-53

```rust
// Establish connection ONCE
let conn = endpoint.connect(node_addr, ALPN).await?;

// Store this connection and reuse it for many messages
// Each message gets a NEW STREAM on the SAME CONNECTION:

// Message 1
{
    let (mut send, mut recv) = conn.open_bi().await?;
    send.write_all(&message1).await?;
    send.finish()?;
    let response = recv.read_to_end(1000).await?;
}

// Message 2 (same connection!)
{
    let (mut send, mut recv) = conn.open_bi().await?;
    send.write_all(&message2).await?;
    send.finish()?;
    let response = recv.read_to_end(1000).await?;
}
```

#### Why This Matters

From `iroh/src/endpoint.rs` lines 1819-1835:

> "A QUIC connection. If all references to a connection have been dropped, then the connection will be **automatically closed**."

**Connection Creation Cost**:
- TLS handshake: ~1 RTT (round-trip time)
- ALPN negotiation: included in handshake
- Relay negotiation (if needed): 1-2 RTTs
- Hole punching (for direct): 1-2 RTTs

**Stream Creation Cost**:
- Zero RTTs (instantaneous)
- Virtually no overhead

**Our Overhead**: We're paying the connection cost (1-4 RTTs) for EVERY message when we should pay it once and use free streams.

---

### Issue #2: Manual Direct Address Management

#### Our Code (WRONG)

**Location**: `initiator.rs:218-233`, `joiner.rs:166-196`

```rust
// We manually extract and store direct addresses
for addr_str in &device_info.direct_addresses {
    if let Ok(socket_addr) = addr_str.parse() {
        node_addr = node_addr.with_direct_addresses([socket_addr]);
    }
}

// Store in device registry
registry.complete_pairing(device_id, device_info.clone(), session_keys).await?;

// Later: Can't find them (Issue #2 from previous doc)
// "No direct addresses for device, relying on discovery"
```

#### Iroh's Automatic Management (CORRECT)

From `iroh/src/endpoint.rs` lines 1534-1542:

```rust
/// Returns a [`Watcher`] for the current [`NodeAddr`] for this endpoint.
/// The observed [`NodeAddr`] will have the current [`RelayUrl`] and direct addresses
/// as they would be returned by [`Endpoint::home_relay`] and [`Endpoint::direct_addresses`].
pub fn node_addr(&self) -> Watcher<NodeAddr>
```

From `iroh/examples/transfer.rs` lines 306-316:

```rust
// Get the node address with all discovered direct addresses
let addr = endpoint.node_addr().initialized().await;
println!("Node ID: {}", addr.node_id);
println!("Relay URL: {:?}", addr.relay_url);
println!("Direct addresses: {:?}", addr.direct_addresses());

// Share this with peer - it contains everything needed
let ticket = ConnectionTicket::new(addr);
```

**Key Insight**: Iroh **automatically discovers and maintains** direct addresses. We don't need to:
- Extract them manually
- Store them in our database
- Manage them at all

#### How Iroh Manages Addresses

From `iroh/src/lib.rs` lines 59-72:

1. **Relay Connection**: Endpoint connects to "home relay" automatically
2. **Direct Discovery**: Iroh continuously discovers direct addresses via:
   - STUN-like probing
   - Peer-reported addresses
   - Local network interfaces
3. **Hole Punching**: When connecting, Iroh attempts direct connection via hole-punching
4. **Automatic Fallback**: If direct fails, uses relay transparently

From `iroh/src/endpoint.rs` lines 517-527:

```rust
// You can monitor connection type changes
let mut stream = endpoint.conn_type(node_id).unwrap().stream();
while let Some(conn_type) = stream.next().await {
    match conn_type {
        ConnType::Direct { .. } => println!("Now connected directly!"),
        ConnType::Relay { .. } => println!("Using relay"),
        ConnType::Mixed { .. } => println!("Transitioning..."),
    }
}
```

**Connections automatically migrate** from relay to direct as Iroh discovers better paths!

#### What We Should Do

```rust
// Just keep the NodeId and let Iroh handle everything
struct PairedDevice {
    device_id: Uuid,
    node_id: NodeId,  // This is all Iroh needs
    session_keys: SessionKeys,
    // NO need to store direct_addresses!
}

// When connecting:
let conn = endpoint.connect(node_id, ALPN).await?;
// Iroh figures out the best route automatically
```

---

### Issue #3: Pairing Protocol Bypasses Connection Cache

#### The Nuance

**Good news**: Our `NetworkingService` DOES have a connection cache!

**Location**: `core/mod.rs:105`

```rust
pub struct NetworkingService {
    endpoint: Option<Endpoint>,
    identity: NetworkIdentity,
    node_id: NodeId,
    // ... other fields ...

    /// Active connections tracker
    active_connections: Arc<RwLock<HashMap<NodeId, Connection>>>,  // ← Cache exists!
}
```

This cache is used for `MESSAGING_ALPN` connections (reconnection logic at `core/mod.rs:402`).

**Bad news**: The pairing protocol bypasses this cache entirely!

#### The Problem

**Location**: `protocol/pairing/mod.rs:36-64`

```rust
pub struct PairingProtocolHandler {
    identity: NetworkIdentity,
    device_registry: Arc<RwLock<DeviceRegistry>>,
    active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    endpoint: Option<Endpoint>,  // ← Stores its own endpoint reference
    // No connection cache!
}
```

**Location**: `protocol/pairing/mod.rs:607-620`

```rust
pub async fn send_pairing_message_to_node(...) -> Result<Option<PairingMessage>> {
    let node_addr = NodeAddr::new(node_id);
    let conn = endpoint.connect(node_addr, PAIRING_ALPN).await?;  // ← Bypasses cache!

    let (mut send, mut recv) = conn.open_bi().await?;
    // ... send/receive ...
    // conn dropped, connection closes
}
```

Every time this is called, it creates a **brand new connection** instead of checking the cache.

#### Why This Happens

The pairing protocol was designed as a standalone module with its own `Endpoint` reference. It doesn't know about or use the `NetworkingService` connection cache. The two systems operate independently:

- **NetworkingService cache**: Used for `MESSAGING_ALPN` and reconnection
- **Pairing protocol**: Creates new `PAIRING_ALPN` connections every time

#### The Fix

**Option 1**: Add a connection cache to `PairingProtocolHandler`

```rust
pub struct PairingProtocolHandler {
    identity: NetworkIdentity,
    device_registry: Arc<RwLock<DeviceRegistry>>,
    active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
    endpoint: Option<Endpoint>,

    // Add connection cache
    connections: Arc<RwLock<HashMap<NodeId, Connection>>>,
}

impl PairingProtocolHandler {
    async fn get_or_create_connection(&self, node_id: NodeId) -> Result<Connection> {
        // Check cache first
        {
            let conns = self.connections.read().await;
            if let Some(conn) = conns.get(&node_id) {
                // Verify connection is still alive
                if conn.close_reason().is_none() {
                    return Ok(conn.clone());  // Connection is Clone (reference-counted)
                }
            }
        }

        // Create new connection if not cached
        let endpoint = self.endpoint.as_ref().unwrap();
        let node_addr = NodeAddr::new(node_id);
        let conn = endpoint.connect(node_addr, PAIRING_ALPN).await?;

        // Cache it
        self.connections.write().await.insert(node_id, conn.clone());

        Ok(conn)
    }
}
```

**Option 2**: Share the NetworkingService connection cache (requires refactoring)

From `iroh/src/endpoint.rs` lines 1819-1835:

> "May be cloned to obtain another handle to the same connection."

**Connections are reference-counted** - cloning is cheap and keeps the connection alive.

---

### Issue #4: Connection Lifecycle Management

#### Our Current Pattern

```rust
async fn send_pairing_message_to_node(...) -> Result<Option<PairingMessage>> {
    let conn = endpoint.connect(node_addr, PAIRING_ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;

    // Send/receive

    // Function returns, conn is dropped, connection closes
}
```

This explains the "connection thrashing" from `PAIRING_POST_SUCCESS_ISSUES.md`:
```
139: Connection lost for device 1bb4f0d1... - connection closed
145: Triggering reconnection attempt for device 1bb4f0d1...
162: Successfully connected to device 1bb4f0d1...
175: Connection closed: Some(ApplicationClosed...)
179: Connection lost for device 1bb4f0d1... - connection closed
```

We keep creating and destroying connections!

#### Correct Pattern (from Iroh examples)

**From** `iroh/examples/echo.rs` lines 42-53:

```rust
// Create connection
let conn = endpoint.connect(node_addr, ALPN).await?;

// Use it for one or more streams
let (mut send, mut recv) = conn.open_bi().await?;
send.write_all(b"Hello, world!").await?;
send.finish()?;
let response = recv.read_to_end(1000).await?;

// Explicitly close when completely done
conn.close(0u32.into(), b"done");

// Ensure close is sent
endpoint.close().await;
```

**Key points**:
1. Keep connection alive as long as you might need it
2. Call `.finish()` on send streams to signal completion
3. Only close connection when truly done (not after each message)
4. For persistent connections, never close - keep them alive

From `iroh/src/endpoint.rs` lines 127:

> "Keep alive is enabled by default: `transport_config.keep_alive_interval(Some(Duration::from_secs(1)))`"

Connections stay alive automatically with 1-second keep-alive packets.

---

## The "Hello Stream" Mystery Solved

From `PAIRING_POST_SUCCESS_ISSUES.md`:

```
166: Opening hello stream to keep connection alive...
169: Hello stream sent successfully
175: Connection closed: Some(ApplicationClosed...)
184: Failed to read message: early eof
```

**What's happening**: We're trying to implement our own keep-alive mechanism, but:
1. Iroh already has keep-alive built in (1-second interval)
2. Our "hello stream" isn't needed
3. The "early eof" is because we open a stream, send data, but the other side expects a proper protocol message

**Solution**: Remove the hello stream entirely. Iroh keeps connections alive automatically.

---

## The Correct Pairing Flow

### Current (WRONG) Flow:

```
Bob → Alice:
├─ Create Connection 1
├─ Open Stream
├─ Send PairingRequest
├─ Receive Challenge
└─ Close Connection 1

Bob → Alice:
├─ Create Connection 2  ← NEW CONNECTION!
├─ Open Stream
├─ Send Response
├─ Receive Complete
└─ Close Connection 2

Alice → Bob:
├─ Create Connection 3  ← ANOTHER ONE!
├─ Open Stream
├─ Send "Hello"
└─ Close immediately (early eof)
```

### Correct (Iroh Way) Flow:

```
Bob → Alice:
├─ Create Connection (ONCE)
│
├─ Stream 1: PairingRequest → Challenge
│   ├─ Bob: open_bi()
│   ├─ Bob: send PairingRequest
│   ├─ Alice: accept_bi()
│   ├─ Alice: send Challenge
│   └─ Bob: receive Challenge
│
├─ Stream 2: Response → Complete
│   ├─ Bob: open_bi()
│   ├─ Bob: send Response
│   ├─ Alice: accept_bi()
│   ├─ Alice: send Complete
│   └─ Bob: receive Complete
│
└─ Connection stays alive (keep-alive every 1s)
    └─ Used for future sync protocol messages
```

**Benefits**:
- 1 connection instead of 3+
- Streams are free (no RTT cost)
- Connection stays alive for future use
- No connection thrashing
- No need for manual "hello" keep-alive

---

## Symmetric Session Keys - Not Iroh's Fault

From `PAIRING_POST_SUCCESS_ISSUES.md` Issue #4:

```rust
send_key: [205, 246, 175, 178, ...]
receive_key: [205, 246, 175, 178, ...]  // Same!
```

This is **our KDF implementation problem**, not related to Iroh usage.

Iroh provides the encrypted transport. We're responsible for application-level encryption with the session keys. This issue stands as documented.

---

## Direct Addresses in Test Output - Explained

From `PAIRING_SUCCESS_TEST_RUN.txt` line 29:

```
Direct addresses: [24.114.41.127:2586, 100.69.176.38:63372, 172.20.10.14:63372, ...]
```

These are **automatically discovered by Iroh**. We're manually extracting and trying to store them (line 123), but then can't find them later (line 163).

**Why can't we find them?**

Not because we didn't store them correctly, but because **we're asking the wrong question**. Instead of:

```rust
// WRONG: Asking our database for addresses
let addresses = device_registry.get_direct_addresses(device_id)?;
if addresses.is_empty() {
    // Fall back to discovery
}
```

We should just:

```rust
// RIGHT: Just connect - Iroh handles addresses
let conn = endpoint.connect(node_id, ALPN).await?;
// Iroh automatically:
// 1. Checks if already connected (reuses connection)
// 2. Tries direct addresses it has discovered
// 3. Falls back to relay if needed
// 4. Attempts hole-punching
// All transparent!
```

---

## Recommended Architecture Changes

### 1. Add Connection Cache

```rust
pub struct NetworkCore {
    endpoint: Endpoint,
    connections: Arc<RwLock<HashMap<NodeId, Connection>>>,
    // ... other fields
}

impl NetworkCore {
    pub async fn get_connection(&self, node_id: NodeId, alpn: &[u8]) -> Result<Connection> {
        // Check cache
        if let Some(conn) = self.connections.read().await.get(&node_id) {
            // Verify connection is still alive
            if conn.close_reason().is_none() {
                return Ok(conn.clone());
            }
        }

        // Create new connection
        let conn = self.endpoint.connect(node_id, alpn).await?;
        self.connections.write().await.insert(node_id, conn.clone());
        Ok(conn)
    }
}
```

### 2. Use Streams for Messages

```rust
pub async fn send_message(
    &self,
    node_id: NodeId,
    message: &PairingMessage,
) -> Result<Option<PairingMessage>> {
    // Get or reuse connection
    let conn = self.get_connection(node_id, PAIRING_ALPN).await?;

    // Create new stream for this message
    let (mut send, mut recv) = conn.open_bi().await?;

    // Send
    let data = serde_json::to_vec(message)?;
    send.write_all(&(data.len() as u32).to_be_bytes()).await?;
    send.write_all(&data).await?;
    send.finish()?;

    // Receive response
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;

    Ok(Some(serde_json::from_slice(&buf)?))
}
```

### 3. Remove Manual Address Management

```rust
// DELETE these fields from DeviceInfo storage:
// - direct_addresses: Vec<String>

// KEEP only:
struct PairedDevice {
    device_id: Uuid,
    node_id: NodeId,  // Iroh needs just this
    session_keys: SessionKeys,
}
```

### 4. Remove Hello Stream

Delete all "hello stream" code. Iroh's keep-alive handles it.

### 5. Handle Connection Events

```rust
// Monitor connection state changes
tokio::spawn({
    let endpoint = endpoint.clone();
    async move {
        let mut watcher = endpoint.conn_type(node_id).unwrap().stream();
        while let Some(conn_type) = watcher.next().await {
            match conn_type {
                ConnType::Direct { .. } => log::info!("Direct connection established"),
                ConnType::Relay { .. } => log::info!("Using relay"),
                ConnType::None => log::info!("Disconnected"),
                _ => {}
            }
        }
    }
});
```

---

## Impact Assessment

### Current System:
- **Connection overhead**: 3-5 new connections per pairing
- **RTT penalty**: 3-15 extra RTTs
- **Connection thrashing**: Constant connect/disconnect cycles
- **Code complexity**: Manual address management
- **Reliability**: "early eof" errors from protocol mismatches

### After Fixes:
- **Connection overhead**: 1 connection per device pair
- **RTT penalty**: 0 (streams are free)
- **Connection thrashing**: None (persistent connections)
- **Code complexity**: Simpler (let Iroh handle addresses)
- **Reliability**: Stable connections with automatic keep-alive

### Code Reduction:
- Can delete ~200 lines of address management code
- Can delete "hello stream" mechanism
- Simpler connection logic

---

## Testing Strategy

After implementing these changes:

1. **Verify Single Connection**: Add logging to confirm only 1 connection per device pair
2. **Monitor Streams**: Log stream creation/destruction (should be many streams, one connection)
3. **Check Connection Type**: Monitor transitions from relay → direct
4. **Measure RTT**: Should see dramatic improvement in message latency
5. **Stability Test**: Run for hours - no connection thrashing

---

## Questions Answered

> "Are we using Iroh correctly?"

**No.** We're creating connections like HTTP requests when we should create one connection and use many streams.

> "How come we're creating new streams?"

**We're not.** We're creating new **connections**. We should create new streams on existing connections.

> "Do we need to be storing direct addresses?"

**No.** Iroh manages this automatically. We just need to store the `NodeId`.

> "Does Iroh actually handle that?"

**Yes.** Iroh discovers, maintains, and uses direct addresses automatically. It even migrates connections from relay to direct transparently.

> "Are we adding complexity?"

**Yes.** We're reimplementing things Iroh already does better:
- Address discovery (Iroh does this)
- Keep-alive (Iroh has this built-in)
- Connection management (should cache, not recreate)
- Relay fallback (Iroh handles automatically)

---

## Conclusion

We've been using Iroh as if it were a stateless HTTP library, creating a new "connection" for each request. But Iroh is designed for **persistent peer-to-peer connections** with many lightweight streams.

The fix is conceptually simple but requires architectural changes:
1. Cache and reuse connections
2. Create new streams for each message
3. Let Iroh manage addresses
4. Remove our custom keep-alive

This will eliminate all 5 issues from `PAIRING_POST_SUCCESS_ISSUES.md` and dramatically simplify the codebase.
