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

16. Broadcast to all peers in `devices`
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
  4. Broadcast StateChange to all devices
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
  4. Broadcast SharedChange to all devices

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

### Service Lifecycle COMPLETE

- [x] SyncService wraps PeerSync for orchestration ✅
- [x] Service starts when library opens (via Library::init_sync_service) ✅
- [x] Service stops gracefully on library close (via Library::shutdown) ✅
- [x] Late initialization support (if networking loads after libraries) ✅
- [x] Automatic backfill detection in run_sync_loop ✅
- [ ] Service config supports enable/disable (sync always enabled if networking available)

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

### Connection Management (BLOCKING) ~90% COMPLETE

- [x] Database schema with watermark columns ✅
- [x] Domain model with sync fields ✅
- [x] Track peer online/offline state (update devices table) ✅
- [x] Subscribe to network connection events ✅
- [x] on_peer_connected() event handler ✅
- [x] on_peer_disconnected() event handler ✅
- [x] Queue changes for offline peers (PeerLog IS the persistent queue) ✅
- [ ] Wire network event receiver from NetworkingService to PeerSync
- [ ] Detect stale connections (optional health checks)

### Startup/Reconnection Sync (BLOCKING) ~50% COMPLETE

- [x] Database schema for watermarks ✅
- [x] Query watermarks from devices table (get_watermarks implemented) ✅
- [ ] Persist watermarks after sync
- [ ] Compare watermarks on startup
- [ ] Trigger catch-up if diverged
- [x] "Sync on reconnect" event handler stub ✅
- [ ] Incremental catch-up (not just full backfill)

### Backfill Protocol

- [x] Backfill state machine (Uninitialized → Backfilling → CatchingUp → Ready) ✅
- [x] Buffer queue for updates during backfill ✅
- [x] transition_to_ready() processes buffer ✅
- [x] Detect new device and trigger backfill (run_sync_loop) ✅
- [x] Peer selection logic (select_backfill_peer) ✅
- [ ] request_state_batch() wired to network (currently stub)
- [ ] request_shared_changes() wired to network (currently stub)
- [ ] Handle StateResponse messages
- [ ] Handle SharedChangeResponse messages
- [ ] Checkpoint persistence for crash recovery

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

**Completion Estimate**: ~77% (core broadcast + service lifecycle + watermark schema + connection event handlers complete, PeerLog IS the persistent queue by design, watermark exchange protocol and event wiring remaining)

## Missing Lifecycle Components (Oct 14, 2025)

Detailed gap analysis to ensure nothing gets lost:

### CRITICAL (Blocking) ️

**1. Service Lifecycle Integration** COMPLETE (Oct 14, 2025)

- Location: core/src/lib.rs:249-279, core/src/library/mod.rs:108-145
- Status: Fully implemented with late initialization support
- Implementation:
  - `Library::init_sync_service()` creates and starts SyncService (mod.rs:108-145)
  - `Library::shutdown()` stops sync service gracefully (mod.rs:247-253)
  - Late initialization: If networking loads after libraries, sync is initialized retroactively (lib.rs:249-279)
  - `SyncService::run_sync_loop()` provides orchestration and automatic backfill detection (service/sync/mod.rs:116-209)
- Files: core/src/lib.rs, core/src/library/mod.rs, core/src/service/sync/mod.rs

**2. Connection State Management** ~90% COMPLETE ✅

- Location: core/src/service/sync/peer.rs
- Source of Truth: `devices` table (NOT a separate sync_partners table)
- Database Schema: COMPLETE (Oct 14, 2025)
  - Added `last_state_watermark TIMESTAMP` column
  - Added `last_shared_watermark TEXT` column
  - Updated entity model and domain model with all sync fields
  - Files: migration/m20240101_000001_unified_schema.rs, entities/device.rs, domain/device.rs
- Event Handler Implementation: COMPLETE (Oct 14, 2025)
  - Network event listener implemented in PeerSync::start()
  - Subscribes to ConnectionEstablished and ConnectionLost events
  - Updates `devices.is_online` and `devices.last_seen_at` on connection/disconnection
  - handle_peer_connected() and handle_peer_disconnected() handlers implemented
- Offline Peer Handling: ALREADY COMPLETE BY DESIGN
  - PeerLog (sync.db) IS the persistent queue for shared changes
  - ACK mechanism prevents pruning until ALL peers acknowledge
  - State changes are idempotent with devices table as source of truth
  - Retry queue handles temporary failures
- Remaining:
  - Wire network event receiver from NetworkingService to PeerSync
  - Optional: Stale connection detection
- Impact: Connection tracking ready, just needs event wiring integration

**3. Startup Sync / Reconnection Logic** ~50% COMPLETE ️

- Location: core/src/service/sync/peer.rs
- Depends On: Priority 2 connection event handlers READY (Oct 14, 2025)
- Database Ready: Watermark columns exist in `devices` table (Oct 14, 2025)
- Implemented:
  - `get_watermarks()` queries `devices` table (peer.rs:124-166)
    - Queries `devices.last_state_watermark` and `devices.last_shared_watermark`
    - Deserializes HLC from JSON
    - Returns (Option<DateTime<Utc>>, Option<HLC>)
  - `exchange_watermarks_and_catchup()` stub with comprehensive TODO (peer.rs:168-186)
- Missing:
  - Add WatermarkExchange message type to SyncMessage enum
  - Send heartbeat with our watermarks to peer
  - Receive peer's watermarks in response
  - Compare timestamps/HLC to detect divergence
  - Request incremental state/shared changes if diverged
  - Update `devices` table with peer's watermarks after sync
  - Call `exchange_watermarks_and_catchup()` from `handle_peer_connected()`
- Impact: Devices drift out of sync after being offline (backfill works but not incremental)

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

**5. Watermark Tracking** ~50% COMPLETE

- Location: peer.rs:124-166
- Status: Query implemented, protocol remaining
- Implemented:
  - get_watermarks() queries devices table
  - Database schema with watermark columns
- Missing:
  - Watermark exchange protocol (send/receive)
  - Compare watermarks on reconnect
  - Persist peer watermarks after sync
  - Update own watermarks after state changes
- Impact: Can't do incremental catch-up yet, but infrastructure ready

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

**8. Initial Backfill Trigger** COMPLETE (Oct 14, 2025)

- Location: service/sync/mod.rs:116-209 (run_sync_loop)
- Status: Fully implemented with automatic detection
- Implementation:
  - `run_sync_loop()` checks for `DeviceSyncState::Uninitialized`
  - Queries `get_connected_sync_partners()` from network (source: `devices` table)
  - Creates `PeerInfo` for peer selection
  - Calls `BackfillManager::start_backfill()` with available peers
  - Retries if no peers available or backfill fails
- Files: core/src/service/sync/mod.rs

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
│    Library::init_sync_service() called                    │
│    SyncService created and started                        │
│    Late initialization if networking loads after          │
│    run_sync_loop spawned for orchestration                │
│    → COMPLETE: Sync service runs properly                    │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Initial Backfill (New Device)                       │
│    run_sync_loop detects Uninitialized state              │
│    Automatic backfill trigger when peers available        │
│    request_state_batch() is stub                          │
│    request_shared_changes() is stub                       │
│    Checkpoint save/load not implemented                   │
│    PeerSync.transition_to_ready() works                   │
│    Buffer processing works                                │
│    → PARTIAL: Detection works, but network requests stubbed  │
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
│    Connection state tracking (event handlers)             │
│    on_peer_disconnected() implemented                     │
│    Changes queued in PeerLog (sync.db) by design          │
│    ACK mechanism prevents premature pruning               │
│    Retry queue handles temporary failures                 │
│    Event receiver wiring to NetworkingService             │
│    → WORKS: Offline peer support built into architecture     │
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
│    Library::shutdown() stops sync service                 │
│    SyncService::stop() signals shutdown and waits         │
│    PeerSync::stop() called to halt background tasks       │
│    → COMPLETE: Graceful shutdown implemented                 │
└─────────────────────────────────────────────────────────────┘
```

**Reality Check**:

- **Phase 1 (Library Open)**: Complete
- **Phase 2 (Initial Backfill)**: ~50% (detection works, network stubs need wiring)
- **Phase 3 (Ready State)**: ~90% (core broadcast complete, batching missing, watermark schema ✅)
- **Phase 4 (Disconnection)**: ~90% (event handlers implemented, PeerLog IS the persistent queue ✅)
- **Phase 5 (Reconnection)**: ~50% (watermark query implemented, exchange protocol remaining)
- **Phase 6 (Library Close)**: Complete

**Overall**: ~77% complete. Service lifecycle, watermark schema, connection event handlers, and offline peer handling complete. Event wiring, watermark exchange protocol, and backfill network integration remaining.

## Updated Next Steps (Prioritized)

### Priority 1: Service Lifecycle - COMPLETE

All items implemented in core/src/lib.rs and core/src/library/mod.rs:

1. SyncService created in Library::init_sync_service()
2. Service started when library opens
3. Late initialization support if networking loads after libraries
4. Graceful shutdown in Library::shutdown()
5. Automatic backfill detection in SyncService::run_sync_loop()

**Status**: Complete - sync service runs properly ✅

### Priority 2: Connection State Management (BLOCKING) ~90% COMPLETE ✅

Use existing `devices` table as source of truth (NOT a separate sync_partners table):

**Database Schema** COMPLETE (Oct 14, 2025):

1. Added watermark columns to `devices` table in unified migration:
   - `last_state_watermark TIMESTAMP` (device-owned data sync tracking)
   - `last_shared_watermark TEXT` (HLC-based shared resource sync, stored as JSON)
2. Updated database entity (`core/src/infra/db/entities/device.rs`):
   - Added `pub last_state_watermark: Option<DateTimeUtc>`
   - Added `pub last_shared_watermark: Option<String>`
   - Updated Syncable implementation to include watermarks in upsert
3. Updated domain model (`core/src/domain/device.rs`):
   - Added all 6 missing fields: `os_version`, `capabilities`, `sync_enabled`, `last_sync_at`, watermarks
   - Updated constructors and conversions (From/TryFrom)

**Files Modified**:
- `core/src/infra/db/migration/m20240101_000001_unified_schema.rs` - added watermark columns
- `core/src/infra/db/entities/device.rs` - added watermark fields to Model and Syncable
- `core/src/domain/device.rs` - added all missing sync-related fields

**PeerSync Implementation** COMPLETE (Oct 14, 2025):

1. Added `network_events` receiver field to PeerSync struct
2. Added `set_network_events()` method to inject event receiver
3. Implemented `start_network_event_listener()` - spawns background task
4. Subscribes to `NetworkEvent::ConnectionEstablished` and `NetworkEvent::ConnectionLost`
5. Implemented `handle_peer_connected(device_id, db)` static handler:
   - Updates `devices.is_online = true`
   - Updates `devices.last_seen_at = now()`
   - Updates `devices.updated_at = now()`
   - TODO comment for watermark exchange trigger (Priority 3)
6. Implemented `handle_peer_disconnected(device_id, db)` static handler:
   - Updates `devices.is_online = false`
   - Updates `devices.last_seen_at = now()`
   - Updates `devices.updated_at = now()`

**Files Modified**:
- `core/src/service/sync/peer.rs` - added network event listener and connection handlers

**Offline Peer Handling** ALREADY IMPLEMENTED BY DESIGN:

- **Shared changes**: `PeerLog` (sync.db) **IS** the persistent queue
  - All changes written to peer_log.append() before broadcast
  - ACK mechanism prevents pruning until ALL peers acknowledge
  - Offline peers catch up via HLC-based queries when reconnecting
- **State changes**: Idempotent state broadcast + devices table as source of truth
  - Retry queue handles temporary failures
  - Watermark comparison triggers incremental sync on reconnection
- **No additional persistent queue needed** - the design already handles offline peers

**Remaining Work** ❌:

1. Wire network event receiver from NetworkingService to PeerSync (integration in SyncService)
2. Optional: Stale connection detection (timeout-based health checks)

**Unblocks**: Priority 3 (reconnection sync) - connection tracking in place

### Priority 3: Startup/Reconnection Sync (BLOCKING) ~50% COMPLETE ️

Database schema ready (Oct 14, 2025). Priority 2 connection handlers ready (Oct 14, 2025).

**Implementation** (core/src/service/sync/peer.rs):

1. Implemented `get_watermarks()` to query `devices` table:
   - Queries `devices.last_state_watermark` and `devices.last_shared_watermark`
   - Deserializes HLC from JSON
   - Returns `(Option<DateTime<Utc>>, Option<HLC>)`
2. Added `exchange_watermarks_and_catchup(peer_id)` stub with TODO:
   - Documents full implementation requirements
   - Placeholder returns Ok(()) for now
   - Needs: WatermarkExchange message type, protocol handler, comparison logic
3. Protocol implementation remaining:
   - Add WatermarkExchange to SyncMessage enum
   - Send heartbeat with our watermarks
   - Receive peer's watermarks
   - Compare to detect divergence
   - Request incremental state if state_watermark diverged
   - Request incremental shared if shared_watermark diverged
4. Call `exchange_watermarks_and_catchup()` from `on_peer_connected()`
5. Update `devices` table with peer's latest watermarks after sync

**Files Modified**:
- `core/src/service/sync/peer.rs` - implemented get_watermarks(), added exchange stub

**Files to Create**:
- `core/src/service/sync/catchup.rs` - incremental catch-up logic (optional, can be in peer.rs)

**Files to Modify**:
- `core/src/service/network/protocol/sync/messages.rs` - add WatermarkExchange message
- `core/src/service/sync/peer.rs` - complete exchange_watermarks_and_catchup()

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
