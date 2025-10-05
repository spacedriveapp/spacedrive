# Device Pairing Connection Issue - Root Cause Analysis

## Problem Summary

After successfully pairing iOS and CLI devices, they fail to maintain a stable connection with repeated "closed by peer: 0" errors, despite both sides successfully establishing QUIC connections.

## Timeline of What We Fixed

### 1. ✅ Device Disconnection State (DEVICE_DISCONNECTION_FIX.md)
- **Issue**: `Disconnected` state didn't preserve session keys
- **Fix**: Added `session_keys` field to `Disconnected` state
- **Files**: `core/src/service/network/device/mod.rs`, `registry.rs`

### 2. ✅ Swift Type Cleanup
- **Issue**: Fake `DeviceInfoLite` type in Swift code
- **Fix**: Replaced with real `PairedDeviceInfo` from generated types
- **Files**: All Swift files using device types

### 3. ✅ Node-to-Device Mapping Restoration
- **Issue**: Mapping not restored when loading paired devices from persistence
- **Fix**: Restore `node_to_device` mapping in `load_paired_devices()`
- **Files**: `core/src/service/network/device/registry.rs:72-88`

### 4. ✅ Trust Level Issues
- **Issue**: Devices marked as `Unreliable` after 5 failed connections, excluded from auto-reconnect
- **Fix**:
  - Allow reconnection to `Unreliable` devices if attempts < 10
  - Reset `Unreliable` → `Trusted` on successful connection
- **Files**: `core/src/service/network/device/persistence.rs:293-300, 356-365`

## Current Root Cause: Bidirectional Stream Deadlock

### What's Happening

Both devices are running the same Rust core and exhibiting identical behavior:

```
[NETWORKING INFO] Successfully connected to device XXX
[NETWORKING DEBUG] Sent ping message after reconnection
[NETWORKING INFO] Incoming connection from PublicKey(YYY)
[NETWORKING INFO] Paired device connected, sending ping to establish stream
[NETWORKING DEBUG] Sent ping to paired device
[NETWORKING ERROR] Failed to accept bidirectional stream: closed by peer: 0
```

### The Ping War Problem

1. **CLI** connects to iOS via mDNS discovery
2. **CLI** opens bidirectional stream and sends Ping
3. **CLI** calls `send.finish()` → closes send half
4. **iOS** sees incoming connection from CLI
5. **iOS** also tries to open bidirectional stream and send Ping
6. **iOS** calls `send.finish()` → closes send half
7. **Both sides** try to `accept_bi()` but streams are already closed
8. **Result**: "closed by peer: 0" on both sides

### Why This Happens

**Symmetric Behavior**: Both devices:
- Try to auto-reconnect (when they're the client)
- Try to send ping on incoming connections (when they're the server)
- Close the stream immediately after sending (`send.finish()`)
- Never wait for or read responses

**QUIC Stream Model**:
- `open_bi()` creates a bidirectional stream
- `finish()` closes the send side
- If both sides finish() without reading, the stream dies

## The Fix Needed

**Only ONE side should initiate the stream**. Two approaches:

### Option A: Remove Proactive Ping (Recommended)
Don't send pings proactively. Let the protocol handler accept streams naturally:

```rust
// In attempt_device_reconnection (core/src/service/network/core/mod.rs:353-376)
// REMOVE the ping sending code - just connect and let accept_bi() handle it

match endpoint.connect(node_addr.clone(), MESSAGING_ALPN).await {
    Ok(conn) => {
        // Just signal connection established
        // Don't open streams - let the event loop accept incoming streams
        let _ = sender.send(EventLoopCommand::ConnectionEstablished {
            device_id,
            node_id,
        });
    }
}

// In handle_connection (core/src/service/network/core/event_loop.rs:208-241)
// REMOVE the proactive ping on incoming connections
// Just proceed to accept_bi() and let the protocol handler process it
```

### Option B: Designate One Side as Initiator
Use a deterministic rule (e.g., lower NodeId always initiates):

```rust
let should_initiate = my_node_id < remote_node_id;
if should_initiate {
    // Send ping
} else {
    // Just accept streams
}
```

## Files to Modify

1. `core/src/service/network/core/mod.rs:353-376`
   - Remove ping sending after successful connection

2. `core/src/service/network/core/event_loop.rs:208-241`
   - Remove proactive ping on incoming paired device connections

## Expected Behavior After Fix

```
[CLI] Successfully connected to device iOS
[CLI] Connection established, waiting for incoming stream from peer
[iOS] Incoming connection from CLI
[iOS] Paired device connected - accepting will handle protocol
[iOS] Accepted bidirectional stream from CLI
[iOS] Directing bidirectional stream to messaging handler
[CLI/iOS] Connection stable ✅
```

## Testing Steps

1. Rebuild both CLI and iOS with the fix
2. Restart CLI daemon: `sd restart`
3. Relaunch iOS app
4. Both should see each other as connected and stay connected
5. Verify with `sd network devices` - should show `Status: 🟢 Connected`

## Additional Notes

- The periodic reconnection (every 30s) may still trigger reconnection attempts
- Health checks should be added to detect stale connections
- Consider implementing proper ping/pong heartbeat after connection is stable

