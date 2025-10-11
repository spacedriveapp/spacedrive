---
id: LSYNC-010
title: Peer Sync Service (Leaderless)
status: In Progress
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, replication, service, peer-to-peer, leaderless]
depends_on: [LSYNC-006, LSYNC-014, LSYNC-015, LSYNC-016, LSYNC-013]
design_doc: core/src/infra/sync/NEW_SYNC.md
---

## Description

Implement the peer sync service using the new leaderless hybrid model. All devices are equals that broadcast changes to peers using two strategies:

1. **State-based** for device-owned data (locations, entries, volumes)
2. **Log-based with HLC** for shared resources (tags, albums, metadata)

**Architecture**: Peer-to-peer broadcast with no leader/follower roles.

## Implementation Steps

### Core Service
1. Create `SyncService` struct (no role field!)
2. Initialize service when library opens
3. Integrate with SyncProtocolHandler for messaging
4. Query `sync_partners` table for peer list

### State-Based Sync (Device-Owned Data)
5. Subscribe to database change events (locations, entries, volumes)
6. Implement `broadcast_state_change()` - sends to all peers
7. Implement `on_state_change_received()` - applies peer's state
8. Batch state changes for efficiency (100ms window)
9. Handle incremental state sync (use timestamps)

### Log-Based Sync (Shared Resources)
10. Subscribe to shared change events (tags, albums, user_metadata)
11. Implement `broadcast_shared_change()` - with HLC ordering
12. Implement `on_shared_change_received()` - applies with conflict resolution
13. Track peer HLCs for causality
14. Implement ACK mechanism for log pruning
15. Prune `shared_changes.db` when all peers ack

### Connection Management
16. Broadcast to all peers in `sync_partners`
17. Handle offline peers (queue changes)
18. Reconnect and sync on peer online
19. Track per-peer sync state

## Technical Details

- Location: `core/src/service/sync/`
  - `mod.rs` - SyncService (no leader/follower split!)
  - `state.rs` - State-based sync logic
  - `shared.rs` - Log-based sync with HLC
  - `hlc.rs` - HLC generator
- Push-based: All peers broadcast changes
- No roles: Every device is equal
- Error handling: Retry with exponential backoff

## Complete Flow

### Device-Owned Data (State-Based)
```
Device A creates location:
  1. INSERT INTO locations (device_id=A, ...)
  2. Emit: LocationCreated event
  3. SyncService.on_location_created()
  4. Broadcast StateChange to all sync_partners
  5. Done! (no log)

Peers (B, C):
  1. Receive StateChange
  2. INSERT INTO locations (device_id=A, ...)
  3. Emit event → UI updates
```

### Shared Resources (Log-Based)
```
Device A creates tag:
  1. Generate HLC(1000,A)
  2. INSERT INTO tags (...)
  3. INSERT INTO shared_changes (hlc, ...)
  4. Broadcast SharedChange to all sync_partners

Peers (B, C):
  1. Receive SharedChange
  2. Update local HLC
  3. INSERT INTO tags (...) with merge
  4. Send ACK to Device A

Device A:
  1. Receive ACKs from all
  2. DELETE FROM shared_changes WHERE hlc <= 1000
  3. Log stays small!
```

## Service Structure

```rust
pub struct SyncService {
    library_id: Uuid,
    // No role field!
    protocol_handler: Arc<SyncProtocolHandler>,
    event_bus: Arc<EventBus>,

    // HLC generator for shared changes
    hlc_generator: Arc<Mutex<HLCGenerator>>,

    // Shared changes log (per-device, small)
    shared_changes_db: Arc<SharedChangesDb>,

    // Track peer states
    peer_states: Arc<RwLock<HashMap<Uuid, PeerSyncState>>>,

    // Pending state broadcasts (batched)
    pending_states: Arc<Mutex<Vec<StateChange>>>,
}

impl SyncService {
    /// Create and start sync service
    pub async fn start(
        library_id: Uuid,
        protocol_handler: Arc<SyncProtocolHandler>,
    ) -> Result<Self, SyncError>;

    /// Broadcast device-owned state change
    async fn broadcast_state_change(&self, change: StateChange);

    /// Broadcast shared resource change (with HLC)
    async fn broadcast_shared_change(&self, change: SharedChange);

    /// Handle received state change
    async fn on_state_change(&self, change: StateChange);

    /// Handle received shared change
    async fn on_shared_change(&self, change: SharedChange);
}
```

## Acceptance Criteria

### State-Based Sync
- [x] State changes broadcast to all peers ✅
- [x] Received state applied idempotently ✅
- [ ] Batch optimization (100ms window) (pending)
- [ ] Incremental sync via timestamps (pending)
- [x] No sync log for device-owned data ✅

### Log-Based Sync
- [x] Shared changes written to per-device log ✅
- [x] HLC generated for each change ✅
- [x] Changes broadcast with HLC ✅
- [x] Peers apply in HLC order ✅
- [x] ACK mechanism works ✅
- [ ] Log pruning keeps it small (<1000 entries) (partial - ACK tracking works, pruning implemented)

### Peer Management
- [x] Works with any number of peers (no leader/follower) ✅
- [ ] Offline peers handled (changes queue) (TODO comments added)
- [ ] Reconnect triggers sync (pending)
- [ ] New device backfill works (pending)

### Integration
- [ ] Service starts when library opens (pending)
- [ ] Integration tests validate peer-to-peer sync (pending)
- [ ] Multi-peer scenario tested (3+ devices) (pending)
- [ ] Conflict resolution via HLC verified (pending)

## Implementation Progress (Oct 9, 2025)

Successfully implemented in `core/src/service/sync/peer.rs`:

**Broadcast Improvements**:
- ✅ Parallel sends using `futures::join_all` (was sequential)
- ✅ Proper error propagation (removed `.unwrap_or_default()`)
- ✅ 30-second timeouts per send operation
- ✅ Structured logging with tracing
- ✅ Ready for retry queue integration (TODO comments added)

**State-Based Sync**:
- ✅ `broadcast_state_change()` sends to all peers in parallel
- ✅ `on_state_change_received()` applies via registry
- ✅ Buffering during backfill phase

**Log-Based Sync**:
- ✅ `broadcast_shared_change()` generates HLC and sends to all peers
- ✅ `on_shared_change_received()` applies with conflict resolution
- ✅ `on_ack_received()` tracks peer ACKs for pruning
- ✅ Peer log append before broadcast

**Next Steps**:
- [ ] Implement backfill for new devices
- [ ] Add retry queue for failed sends
- [ ] Connection state tracking
- [ ] Integration testing

## Performance Benefits

- **No Bottleneck**: Any device can change anytime
- **Offline First**: Changes queue locally
- **Simpler**: No leader election overhead
- **Resilient**: No single point of failure

## References

- `core/src/infra/sync/NEW_SYNC.md` - Leaderless architecture
- HLC: LSYNC-014
- Protocol: LSYNC-013
- TransactionManager: LSYNC-006
