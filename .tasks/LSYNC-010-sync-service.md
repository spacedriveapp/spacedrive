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
last_updated: 2025-10-14
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

### Service Lifecycle (BLOCKING)
- [ ] PeerSync added to Services struct
- [ ] Service starts when library opens
- [ ] Service stops gracefully on library close
- [ ] Flush pending changes on shutdown
- [ ] Service config supports enable/disable

### State-Based Sync (Core)
- [x] State changes broadcast to all peers ✅
- [x] Received state applied idempotently ✅
- [x] No sync log for device-owned data ✅
- [x] Parallel sends with timeout ✅
- [x] Retry queue for failed sends ✅
- [ ] Batch optimization (100ms window)
- [ ] Incremental sync via timestamps

### Log-Based Sync (Core)
- [x] Shared changes written to per-device log ✅
- [x] HLC generated for each change ✅
- [x] Changes broadcast with HLC ✅
- [x] Peers apply in HLC order ✅
- [x] ACK mechanism works ✅
- [x] Periodic log pruning background task ✅
- [x] Log pruning keeps it small (<1000 entries) ✅

### Connection Management (BLOCKING)
- [ ] Track peer online/offline state
- [ ] on_peer_connected() event handler
- [ ] on_peer_disconnected() event handler
- [ ] Queue changes for offline peers (persistent)
- [ ] Detect stale connections

### Startup/Reconnection Sync (BLOCKING)
- [ ] Watermark tracking per peer
- [ ] Persist watermarks to database
- [ ] Compare watermarks on startup
- [ ] Trigger catch-up if diverged
- [ ] "Sync on reconnect" event handler
- [ ] Incremental catch-up (not just full backfill)

### Backfill Protocol
- [x] Backfill state machine (Uninitialized → Backfilling → CatchingUp → Ready) ✅
- [x] Buffer queue for updates during backfill ✅
- [x] transition_to_ready() processes buffer ✅
- [ ] request_state_batch() wired to network
- [ ] request_shared_changes() wired to network
- [ ] Handle StateResponse messages
- [ ] Handle SharedChangeResponse messages
- [ ] Checkpoint persistence for crash recovery
- [ ] Detect new device and trigger backfill
- [ ] Peer selection logic

### Heartbeat & Monitoring
- [x] Heartbeat message handler ✅
- [ ] Periodic heartbeat sender
- [ ] Health check metrics
- [ ] Watermark exchange in heartbeat

### Integration Testing
- [ ] Service lifecycle test
- [ ] Two-peer state sync test
- [ ] Conflict resolution via HLC test
- [ ] Multi-peer scenario (3+ devices)
- [ ] Offline peer handling test
- [ ] Reconnection sync test
- [ ] New device backfill test

## Implementation Progress (Oct 9, 2025)

Successfully implemented in `core/src/service/sync/peer.rs`:

**Broadcast Improvements**:
- Parallel sends using `futures::join_all` (was sequential)
- Proper error propagation (removed `.unwrap_or_default()`)
- 30-second timeouts per send operation
- Structured logging with tracing
- Ready for retry queue integration (TODO comments added)

**State-Based Sync**:
- `broadcast_state_change()` sends to all peers in parallel
- `on_state_change_received()` applies via registry
- Buffering during backfill phase

**Log-Based Sync**:
- `broadcast_shared_change()` generates HLC and sends to all peers
- `on_shared_change_received()` applies with conflict resolution
- `on_ack_received()` tracks peer ACKs for pruning
- Peer log append before broadcast

**Completion Estimate**: ~40% (core broadcast works, but lifecycle missing)

## Missing Lifecycle Components (Oct 14, 2025)

Detailed gap analysis to ensure nothing gets lost:

### CRITICAL (Blocking) ️

**1. Service Lifecycle Integration**
- Location: Not in `core/src/service/mod.rs` Services struct
- Problem: PeerSync.start() exists but never called during library open
- Impact: Sync doesn't work at all - service never runs
- Files: core/src/service/mod.rs:29-47, core/src/library/manager.rs

**2. Connection State Management**
- Location: No peer connection tracking anywhere
- Problem: Can't detect when peers go online/offline
- Missing:
  - `on_peer_connected()` event handler
  - `on_peer_disconnected()` event handler (exists in backfill.rs:258 but never called)
  - Persistent peer state tracking (online/offline/last_seen)
  - Change queueing for offline peers (TODO comments only)
- Impact: Can't handle offline peers or reconnections
- Reference: peer.rs:447, 559 (TODO comments for retry queue)

**3. Startup Sync / Reconnection Logic**
- Location: Missing entirely
- Problem: No catch-up after device restarts or comes back online
- Missing:
  - Watermark comparison on startup (state_watermark always None: peer.rs:119)
  - Incremental catch-up mechanism (only full backfill exists)
  - "Sync on reconnect" trigger
- Impact: Devices drift out of sync after being offline
- Reference: peer.rs:116-125 (get_watermarks always returns None)

### MAJOR (Functional Gaps)

**4. Backfill Network Integration**
- Location: core/src/service/sync/backfill.rs
- Problem: BackfillManager can't actually request data
- Stubs:
  - `request_state_batch()` (line 220-238) - always returns empty
  - `request_shared_changes()` (line 240-255) - always returns empty
- Missing:
  - Wire requests through NetworkTransport
  - Handle StateRequest/SharedChangeRequest responses
  - Resume from checkpoint on failure
- Impact: New devices can't backfill initial state

**5. Watermark Tracking**
- Location: peer.rs:116-125
- Problem: Can't determine what needs syncing
- Missing:
  - Track last synced timestamp per model type
  - Persist watermarks to database
  - Compare watermarks on reconnect
- Impact: Can't do incremental sync, only full state transfer

**6. Batching Optimization**
- Location: peer.rs (broadcast methods)
- Problem: State changes sent one-at-a-time
- Missing:
  - 100ms batching window (marked "pending" in task)
  - Coalescing multiple changes to same record
  - Batch send with StateBatch/SharedChangeBatch
- Impact: High network overhead, chatty protocol

### MINOR (Nice to Have)

**7. Checkpoint Persistence**
- Location: state.rs:186-195
- Problem: Backfill can't resume after crash
- Stub: save() and load() are no-ops
- Impact: Must restart backfill from beginning if interrupted

**8. Initial Backfill Trigger**
- Location: Missing entirely
- Problem: No code to detect new device and start backfill
- Questions:
  - When does device transition Uninitialized → Backfilling?
  - How are available peers discovered?
  - Who calls BackfillManager::start_backfill()?

**9. Heartbeat Health Monitoring**
- Location: handler.rs:275-301 (receive only)
- Problem: Heartbeat handler exists but no sender
- Missing:
  - Periodic heartbeat background task
  - Stale connection detection
  - Health check metrics

**10. Incremental State Sync**
- Location: protocol_handler.rs:116-160
- Problem: Only supports full backfill
- Note: query_state() supports `since` param but never used with actual timestamps

## Complete Lifecycle Flow

Here's the full sync lifecycle with gaps marked:

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Library Open                                        │
│    PeerSync.start() never called                          │
│    Not in Services struct                                 │
│    No integration with library manager                    │
│    → BLOCKS: Everything else                                 │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Initial Backfill (New Device)                       │
│    No trigger to detect new device                        │
│    request_state_batch() is stub                          │
│    request_shared_changes() is stub                       │
│    Checkpoint save/load not implemented                   │
│    PeerSync.transition_to_ready() works                   │
│    Buffer processing works                                │
│    → BLOCKS: New devices joining library                     │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: Ready State (Normal Operation)                      │
│    Broadcast works (parallel sends, timeouts)             │
│    Receive works (via registry)                           │
│    ACK mechanism works                                    │
│    Retry queue works (background processor)               │
│    Log pruning works (periodic background task)           │
│    No batching (100ms window)                             │
│    State watermark always None                            │
│    → WORKS: Happy path with 2+ always-online devices         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 4: Peer Disconnection                                  │
│    No connection state tracking                           │
│    on_peer_disconnected() exists but never called         │
│    Changes not queued persistently for offline peers      │
│    Retry queue handles temporary failures                 │
│    → BLOCKS: Offline peer support                            │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 5: Reconnection / Startup Sync                         │
│    No watermark comparison                                │
│    No incremental catch-up (only full backfill)           │
│    No "sync on reconnect" event handler                   │
│    No divergence detection                                │
│    → BLOCKS: Devices staying in sync after offline periods   │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 6: Library Close                                       │
│    No graceful shutdown in Services.stop_all()            │
│    No flush of pending changes                            │
│    → MINOR: Might lose in-flight changes on shutdown         │
└─────────────────────────────────────────────────────────────┘
```

**Reality Check**: Implementation is ~75% complete for **Phase 3 only** (always-online happy path), but 0% complete for **Phases 1, 4, 5, 6** (lifecycle management).

## Updated Next Steps (Prioritized)

### Priority 1: Service Lifecycle (BLOCKING) ️
1. Add PeerSync to Services struct (core/src/service/mod.rs)
2. Create init_sync() and start_sync() methods
3. Call PeerSync.start() during library open
4. Add graceful shutdown to Services.stop_all()
5. Add sync service to service config

**Unblocks**: Everything else - sync can actually run

### Priority 2: Connection State Management (BLOCKING) ️
1. Add peer connection/disconnection event handlers
2. Track peer online/offline state in database
3. Implement change queueing for offline peers
4. Call on_peer_disconnected() on network events

**Unblocks**: Offline peer support, reconnection

### Priority 3: Startup/Reconnection Sync (BLOCKING) ️
1. Implement watermark tracking per peer
2. Persist watermarks to database
3. Compare watermarks on startup/reconnect
4. Trigger incremental catch-up if diverged
5. Add "sync on reconnect" handler

**Unblocks**: Devices staying in sync after offline periods

### Priority 4: Backfill Network Integration
1. Wire request_state_batch() through NetworkTransport
2. Wire request_shared_changes() through NetworkTransport
3. Handle response messages properly
4. Add checkpoint persistence for crash recovery
5. Implement peer selection logic trigger

**Unblocks**: New devices joining library

### Priority 5: Optimizations
1. Implement 100ms batching window
2. Add state watermark tracking (timestamps)
3. Implement incremental state sync
4. Add heartbeat sender background task
5. Add health check metrics

**Unblocks**: Better performance and monitoring

### Priority 6: Testing
1. Integration test: Service lifecycle
2. Integration test: Two-peer sync
3. Integration test: Offline peer handling
4. Integration test: Reconnection sync
5. Integration test: New device backfill

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
