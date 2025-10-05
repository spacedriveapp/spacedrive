# Device Disconnection & Reconnection Fix

## Problem Summary

When an iOS device was paired with the CLI, it would randomly appear online and offline, and when offline it wouldn't show in the paired devices query. The logs showed:

1. **Connection instability**: Device repeatedly connecting and disconnecting
2. **Error on reconnection**: `"Device must be paired before connecting"` even though device was paired
3. **Disappearing from device list**: `sd network devices` would show "No paired devices" after disconnection

## Root Causes

### 1. **Missing Session Keys in Disconnected State**
The `Disconnected` device state didn't preserve `session_keys`, which are required to re-establish a connection. When a device disconnected, it would lose its session keys and couldn't reconnect.

**Location**: `core/src/service/network/device/mod.rs:82-87`

```rust
// BEFORE:
Disconnected {
    info: DeviceInfo,
    last_seen: DateTime<Utc>,
    reason: DisconnectionReason,
}

// AFTER:
Disconnected {
    info: DeviceInfo,
    session_keys: SessionKeys,  // ← Added this
    last_seen: DateTime<Utc>,
    reason: DisconnectionReason,
}
```

### 2. **`set_device_connected` Didn't Handle Disconnected State**
When a device tried to reconnect from the `Disconnected` state, the `set_device_connected` method would fall through to an error case: "Device must be paired before connecting".

**Location**: `core/src/service/network/device/registry.rs:516-626`

**Fix**: Added a new match arm to handle `Disconnected` state and transition it back to `Connected`.

### 3. **Device Query Excluded Disconnected Devices**
The devices query only returned devices in `Paired` and `Connected` states, so disconnected devices would disappear from `sd network devices`.

**Location**: `core/src/ops/network/devices/query.rs:54-62`

**Fix**: Added `Disconnected` state to the query results.

## Changes Made

### 1. **Updated `DeviceState::Disconnected` Definition**
- File: `core/src/service/network/device/mod.rs`
- Added `session_keys: SessionKeys` field to preserve encryption keys

### 2. **Updated `mark_disconnected` Method**
- File: `core/src/service/network/device/registry.rs:276-308`
- Now extracts and preserves `session_keys` from `Connected` or `Paired` states
- Stores them in the new `Disconnected` state

### 3. **Updated `mark_connected` Method**
- File: `core/src/service/network/device/registry.rs:214-237`
- Now handles `Disconnected` state by extracting `session_keys` from it
- Allows reconnection from disconnected state

### 4. **Updated `set_device_connected` Method**
- File: `core/src/service/network/device/registry.rs:616-649`
- Added new match arm for `Disconnected` state
- Transitions from `Disconnected` → `Connected` using preserved session keys
- Updates persistence with new connection addresses

### 5. **Updated Device Query**
- File: `core/src/ops/network/devices/query.rs:60`
- Added `DeviceState::Disconnected { info, .. }` to query results
- Disconnected devices now appear in `sd network devices` with status "Disconnected"

## Expected Behavior After Fix

1. ✅ **Devices stay in registry**: Paired devices remain visible in `sd network devices` even when disconnected
2. ✅ **Seamless reconnection**: When a device comes back online, it transitions from `Disconnected` → `Connected` using preserved session keys
3. ✅ **No more "must be paired" error**: The system recognizes disconnected devices as previously paired
4. ✅ **Stable connection state**: Connection status reflects actual device state without random transitions

## Testing

To test the fix:

1. **Pair with iOS device**: `sd network pair generate` (CLI) + join from iOS
2. **Verify pairing**: `sd network devices` should show the paired device
3. **Force disconnect**: Close iOS app or put device to sleep
4. **Check visibility**: `sd network devices` should still show device (as disconnected)
5. **Reconnect**: Reopen iOS app
6. **Verify reconnection**: Device should transition to connected without errors
7. **Check logs**: No more "Device must be paired before connecting" errors

## Related Files

- `core/src/service/network/device/mod.rs` - Device state definitions
- `core/src/service/network/device/registry.rs` - Device state management
- `core/src/ops/network/devices/query.rs` - Device listing query
- `core/src/service/network/core/event_loop.rs` - Connection event handling

## Migration Notes

This change is **backward compatible** in terms of code structure (no breaking API changes), but devices that were in `Disconnected` state before this fix may not have session keys stored. They will need to be re-paired once if they cannot reconnect.

For persistent storage, the `DevicePersistence` layer already stores session keys separately, so reconnection from persisted devices will continue to work.

