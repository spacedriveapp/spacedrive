---
id: LSYNC-013
title: Sync Protocol Handler (Message-based)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, networking, protocol, push]
depends_on: [LSYNC-008, LSYNC-009]
---

## Description

Create a dedicated sync protocol handler for the networking layer that enables push-based sync via `SyncMessage` enum. This replaces polling with efficient message-passing between leader and follower devices.

## Architecture Decision

**Before**: Follower polls leader every 5 seconds (`sync_iteration()`)
- High latency (up to 5s)
- Wasted bandwidth (empty polls)
- Battery drain on mobile

**After**: Push-based messaging via dedicated protocol
- Instant updates (pushed when changes happen)
- No empty polls
- Bi-directional: Leader pushes, follower can request

## SyncMessage Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    // Leader → Follower: New entries available
    NewEntries {
        library_id: Uuid,
        from_sequence: u64,
        to_sequence: u64,
        entry_count: usize,
    },

    // Follower → Leader: Request entries
    FetchEntries {
        library_id: Uuid,
        since_sequence: u64,
        limit: usize,
    },

    // Leader → Follower: Response with entries
    EntriesResponse {
        library_id: Uuid,
        entries: Vec<SyncLogEntry>,
        has_more: bool,
    },

    // Follower → Leader: Acknowledge received
    Acknowledge {
        library_id: Uuid,
        up_to_sequence: u64,
    },

    // Bi-directional: Heartbeat
    Heartbeat {
        library_id: Uuid,
        current_sequence: u64,
        role: SyncRole,
    },

    // Leader → Follower: You're behind, full sync needed
    SyncRequired {
        library_id: Uuid,
        reason: String,
    },
}
```

## Implementation Steps

1. Create `core/src/service/network/protocol/sync/` directory
2. Create `sync/mod.rs` - Main protocol handler
3. Create `sync/messages.rs` - SyncMessage enum
4. Create `sync/leader.rs` - Leader-side message handling
5. Create `sync/follower.rs` - Follower-side message handling
6. Register protocol with ALPN: `/spacedrive/sync/1.0.0`
7. Integrate with DeviceRegistry for connection lookup
8. Add connection lifecycle management

## Protocol Handler Structure

```rust
// core/src/service/network/protocol/sync/mod.rs
pub struct SyncProtocolHandler {
    library_id: Uuid,
    sync_log_db: Arc<SyncLogDb>,
    device_registry: Arc<RwLock<DeviceRegistry>>,
    event_bus: Arc<EventBus>,
    role: SyncRole,
}

impl ProtocolHandler for SyncProtocolHandler {
    const ALPN: &'static [u8] = b"/spacedrive/sync/1.0.0";

    async fn handle_connection(
        &self,
        stream: BiStream,
        peer_device_id: Uuid,
    ) -> Result<(), NetworkingError>;
}

impl SyncProtocolHandler {
    // Leader: Push notification when new entries created
    pub async fn notify_new_entries(
        &self,
        from_seq: u64,
        to_seq: u64,
    ) -> Result<(), SyncError>;

    // Follower: Request entries from leader
    pub async fn request_entries(
        &self,
        since_seq: u64,
    ) -> Result<Vec<SyncLogEntry>, SyncError>;

    // Handle incoming message
    async fn handle_message(
        &self,
        msg: SyncMessage,
        stream: &mut BiStream,
    ) -> Result<(), SyncError>;
}
```

## Connection Management

- Protocol uses Iroh BiStreams for bi-directional communication
- Leader maintains open connections to all follower devices
- Follower connects to leader on library open
- Heartbeat every 30 seconds to detect disconnections
- Auto-reconnect on connection loss

## Acceptance Criteria

- [ ] SyncProtocolHandler implemented
- [ ] SyncMessage enum defined
- [ ] Leader can push NewEntries notifications
- [ ] Follower can request entries
- [ ] BiStream communication working
- [ ] Protocol registered with correct ALPN
- [ ] Connection lifecycle managed
- [ ] Integration tests validate message flow

## References

- Existing protocols: `core/src/service/network/protocol/pairing/`
- Protocol registry: `core/src/service/network/protocol/registry.rs`
