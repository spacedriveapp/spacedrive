# Sync System TODO Resolution Plan

## TODOs Found During Watermark Investigation

### Critical (Must Fix - Correctness Issues)

#### 1. CRITICAL: Incremental State Catchup Not Implemented

**Location**: `core/src/service/sync/peer.rs:518-523`

**Current Code**:

```rust
if we_need_state_catchup {
    info!(peer = %peer_id, "Requesting incremental state catch-up");
    // TODO: Implement incremental state request
    // For now, log that full backfill will occur
    warn!(
        peer = %peer_id,
        "Incremental state catch-up not yet implemented, will use backfill"
    );
}
```

**Problem**:

- Watermark exchange detects divergence but doesn't trigger catchup
- Device falls back to full backfill instead of incremental sync
- The `BackfillManager::catch_up_from_peer()` EXISTS but isn't called!

**Fix**:

```rust
if we_need_state_catchup {
    info!(peer = %peer_id, "Requesting incremental state catch-up");

    // Trigger incremental catchup using watermarks
    let backfill_manager = self.backfill_manager
        .upgrade()
        .ok_or_else(|| anyhow::anyhow!("Backfill manager not available"))?;

    tokio::spawn({
        let backfill_manager = backfill_manager.clone();
        let peer_id = peer_id;
        let my_state_watermark = my_state_watermark;
        let my_shared_watermark = my_shared_watermark;
        async move {
            if let Err(e) = backfill_manager.catch_up_from_peer(
                peer_id,
                my_state_watermark,
                my_shared_watermark.map(|hlc| serde_json::to_string(&hlc).ok()).flatten(),
            ).await {
                warn!("Incremental catch-up failed: {}", e);
            }
        }
    });
}
```

**Impact**: HIGH - This is why watermark-based incremental sync doesn't work!

---

#### 2. CRITICAL: Backfill Checkpoint Persistence Not Implemented

**Location**: `core/src/service/sync/state.rs:251-260`

**Current Code**:

```rust
/// Save checkpoint to disk (TODO: implement persistence)
pub async fn save(&self) -> Result<(), std::io::Error> {
    // TODO: Persist to disk for crash recovery
    Ok(())
}

/// Load checkpoint from disk (TODO: implement persistence)
pub async fn load() -> Result<Option<Self>, std::io::Error> {
    // TODO: Load from disk
    Ok(None)
}
```

**Problem**:

- If daemon crashes during backfill, must restart from beginning
- With 100k+ entries, this could mean hours of lost work

**Fix**: Store in `sync.db`:

```sql
-- Add to sync.db
CREATE TABLE backfill_checkpoints (
    peer_device_uuid TEXT PRIMARY KEY,
    resume_token TEXT,
    completed_models TEXT,  -- JSON array
    progress REAL,
    updated_at TEXT
);
```

```rust
pub async fn save(&self, sync_db: &rusqlite::Connection) -> Result<(), std::io::Error> {
    use rusqlite::params;

    sync_db.execute(
        "INSERT OR REPLACE INTO backfill_checkpoints
         (peer_device_uuid, resume_token, completed_models, progress, updated_at)
         VALUES (?, ?, ?, ?, ?)",
        params![
            self.peer.to_string(),
            self.resume_token,
            serde_json::to_string(&self.completed_models)?,
            self.progress,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}
```

**Impact**: HIGH - Backfill crashes lose all progress

---

### Important (Affects Performance/Features)

#### 3. ️ HLC Parsing from String Watermark

**Location**: `core/src/service/sync/backfill.rs:209`

**Current**:

```rust
// For now, just do full backfill of shared resources
// TODO: Parse HLC from string watermark when HLC implements FromStr
let max_shared_hlc = self.backfill_shared_resources(peer).await?;
```

**Fix**: Implement `FromStr` for HLC:

```rust
// core/src/infra/sync/hlc.rs
impl std::str::FromStr for HLC {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

// Then in backfill.rs:
let shared_hlc = shared_watermark
    .and_then(|s| s.parse::<HLC>().ok());
let max_shared_hlc = self.backfill_shared_resources_since(peer, shared_hlc).await?;
```

**Impact**: MEDIUM - Shared resources always do full sync instead of incremental

---

#### 4. ️ Multi-Peer Backfill Not Implemented

**Location**: `core/src/service/sync/backfill.rs:258-260`

**Current**:

```rust
// TODO: Get list of all peers, not just primary
// For now, just backfill from primary peer
let checkpoint = self
    .backfill_peer_state(primary_peer, model_types.clone(), None, since_watermark)
    .await?;
```

**Problem**:

- Only syncs from one peer
- If that peer goes offline mid-backfill, sync stalls
- Could parallelize by syncing different resources from different peers

**Fix**:

```rust
// Get device ownership map: resource_type -> owning_peer
let mut peer_assignments: HashMap<String, Uuid> = HashMap::new();

for model_type in &model_types {
    // For device-owned models, find which peer owns them
    let owner = self.find_resource_owner(&model_type, &available_peers).await?;
    peer_assignments.insert(model_type.clone(), owner);
}

// Group by peer and backfill in parallel
let mut backfill_tasks = vec![];
for (peer, models) in group_by_peer(peer_assignments) {
    let task = self.backfill_peer_state(peer, models, None, since_watermark);
    backfill_tasks.push(task);
}

futures::future::try_join_all(backfill_tasks).await?;
```

**Impact**: MEDIUM - Single point of failure, missed parallelization opportunity

---

#### 5. ️ Protocol Handler Uses Raw SQL

**Location**: `core/src/service/sync/protocol_handler.rs:220-227`

**Current**:

```rust
for row in rows {
    // TODO: Proper serialization per model type via registry
    let uuid_str: String = row.try_get("", "uuid")?;
    let uuid = Uuid::parse_str(&uuid_str)?;

    records.push(StateRecord {
        uuid,
        data: serde_json::json!({}), // TODO: Serialize row via Syncable trait
        timestamp: Utc::now(),
    });
}
```

**Problem**:

- Returns empty JSON instead of actual data
- This handler appears to be unused/deprecated (covered by registry)

**Fix**: Either:

1. Remove dead code, or
2. Route through registry:

```rust
for row in rows {
    let uuid_str: String = row.try_get("", "uuid")?;
    let uuid = Uuid::parse_str(&uuid_str)?;

    // Serialize through registry
    let data = crate::infra::sync::registry::serialize_row(
        &model_type,
        row,
        self.db.clone(),
    ).await?;

    records.push(StateRecord { uuid, data, timestamp });
}
```

**Impact**: LOW - Appears to be dead code, but should be cleaned up

---

#### 6. ️ Peer Disconnection During Backfill

**Location**: `core/src/service/sync/backfill.rs:555-557`

**Current**:

```rust
pub async fn on_peer_disconnected(&self, peer_id: Uuid) -> Result<()> {
    let state = self.peer_sync.state().await;

    if let DeviceSyncState::Backfilling { peer, .. } = state {
        if peer == peer_id {
            warn!(
                peer_id = %peer_id,
                "Backfill peer disconnected, need to switch"
            );

            // TODO: Save checkpoint, select new peer, resume
            // For now, just log
        }
    }

    Ok(())
}
```

**Problem**:

- Backfill stalls if peer disconnects
- No automatic peer switching or resume logic

**Fix**:

```rust
pub async fn on_peer_disconnected(&self, peer_id: Uuid) -> Result<()> {
    let state = self.peer_sync.state().await;

    if let DeviceSyncState::Backfilling { peer, .. } = state {
        if peer == peer_id {
            warn!(peer_id = %peer_id, "Backfill peer disconnected, attempting recovery");

            // Get current checkpoint
            let checkpoint = self.get_current_checkpoint().await?;

            // Save checkpoint
            checkpoint.save(self.sync_db()).await?;

            // Get list of remaining online peers
            let remaining_peers = self.peer_sync
                .network()
                .get_connected_sync_partners(self.library_id, self.peer_sync.db())
                .await?;

            if remaining_peers.is_empty() {
                // Transition to paused, will resume when peer reconnects
                self.peer_sync.transition_to_paused().await?;
                info!("No peers available, pausing until reconnection");
            } else {
                // Select new peer and resume
                let new_peer = select_backfill_peer(convert_to_peer_info(remaining_peers))?;
                info!(new_peer = %new_peer, "Switching to new backfill peer");

                // Resume from checkpoint with new peer
                self.resume_backfill(new_peer, checkpoint).await?;
            }
        }
    }

    Ok(())
}
```

**Impact**: MEDIUM - Affects reliability during network issues

---

### Minor (Technical Debt)

#### 7. Hardcoded Latency Metrics

**Location**: `core/src/service/sync/mod.rs:216`

**Fix**: Measure actual network latency using ping/pong or request timing

**Impact**: LOW - Affects peer selection quality

---

#### 8. Hardcoded Progress Calculation

**Location**: `core/src/service/sync/backfill.rs:345`

**Fix**: Calculate based on records synced vs total expected

**Impact**: LOW - Progress bar inaccurate

---

#### 9. Metrics Persistence Not Implemented

**Location**: `core/src/service/sync/metrics/persistence.rs:23, 41, 59`

**Fix**: Store in sync.db:

```sql
CREATE TABLE sync_metrics_snapshots (
    timestamp TEXT PRIMARY KEY,
    snapshot_json TEXT NOT NULL
);
```

**Impact**: LOW - Metrics lost on restart, but not critical

---

#### 10. Legacy sync_sequence in TransactionManager

**Location**: `core/src/infra/sync/transaction.rs:70, 251`

**Problem**: Old leader-based sequence tracking still present

**Fix**: Remove entirely (we use HLC now):

```rust
// DELETE these fields and methods:
sync_sequence: Arc<Mutex<HashMap<Uuid, u64>>>,  // Not used in leaderless arch
```

**Impact**: LOW - Dead code cleanup

---

## Resolution Plan

### Phase 1: Critical Fixes (With Watermark Refactor)

These MUST be fixed alongside the watermark refactor:

1. **Implement incremental state catchup trigger** (peer.rs)

   - Wire up `on_watermark_exchange_response()` to call `catch_up_from_peer()`
   - Add backfill_manager reference to PeerSync

2. **Implement checkpoint persistence** (state.rs)

   - Add `backfill_checkpoints` table to sync.db
   - Implement save/load using rusqlite
   - Resume backfill from checkpoint on daemon restart

3. **Implement HLC parsing** (backfill.rs)
   - Add `FromStr` impl for HLC
   - Enable incremental shared resource catchup

**Timeline**: Same as watermark fix (Week 1)

### Phase 2: Important Fixes (Week 2)

4. **Multi-peer backfill** (backfill.rs)

   - Implement peer switching on disconnection
   - Optional: Parallel backfill from multiple peers

5. **Clean up protocol handler** (protocol_handler.rs)
   - Remove dead SQL code or route through registry

**Timeline**: 3-4 days

### Phase 3: Cleanup (Week 3)

6. **Latency metrics** - Use actual measurements
7. **Progress calculation** - Calculate from actual data
8. **Metrics persistence** - Store in sync.db
9. **Remove legacy sequence tracking** - Delete dead code

**Timeline**: 2-3 days

## Implementation Order

### Combined with Watermark Fix:

```rust
// sync.db schema (all at once):
CREATE TABLE device_resource_watermarks (
    device_uuid TEXT NOT NULL,
    peer_device_uuid TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    last_watermark TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
);

CREATE TABLE backfill_checkpoints (
    peer_device_uuid TEXT PRIMARY KEY,
    resume_token TEXT,
    completed_models TEXT,  -- JSON array
    progress REAL,
    updated_at TEXT
);

CREATE TABLE sync_metrics_snapshots (
    timestamp TEXT PRIMARY KEY,
    library_id TEXT NOT NULL,
    snapshot_json TEXT NOT NULL
);
```

### Files to Modify

**Critical Fixes**:

1. `core/src/infra/sync/watermarks.rs` (NEW - per-resource watermarks)
2. `core/src/service/sync/peer.rs` (wire up incremental catchup, add sync_db)
3. `core/src/service/sync/state.rs` (implement checkpoint save/load)
4. `core/src/infra/sync/hlc.rs` (add FromStr impl)
5. `core/src/service/sync/backfill.rs` (use parsed HLC for shared catchup)
6. `core/src/service/sync/mod.rs` (pass backfill_manager to PeerSync)

**Important Fixes**: 7. `core/src/service/sync/backfill.rs` (peer disconnection handler) 8. `core/src/service/sync/protocol_handler.rs` (remove dead code or fix)

**Cleanup**: 9. `core/src/service/sync/metrics/persistence.rs` (implement storage) 10. `core/src/infra/sync/transaction.rs` (remove legacy sequence)

## Testing Strategy

### For Critical Fixes

**Test 1**: Incremental state catchup

```rust
#[tokio::test]
async fn test_incremental_state_catchup_triggered() {
    // Device A creates 100 locations
    // Device B syncs them
    // Device A creates 10 more locations
    // Device B disconnects
    // Device B reconnects
    // watermark_exchange_and_catchup() should trigger catch_up_from_peer()
    // Verify: Only 10 new locations synced (not all 110)
}
```

**Test 2**: Checkpoint persistence

```rust
#[tokio::test]
async fn test_backfill_resumes_from_checkpoint() {
    // Device B starts backfill
    // Sync 5,000 entries
    // Simulate crash (drop backfill_manager)
    // Create new backfill_manager
    // BackfillCheckpoint::load() should return saved checkpoint
    // Resume should continue from entry 5,001
}
```

**Test 3**: HLC incremental sync

```rust
#[tokio::test]
async fn test_shared_resource_incremental_catchup() {
    // Create 50 tags on Device A
    // Device B syncs them (HLC watermark set)
    // Create 10 more tags on Device A
    // Device B catchup with watermark
    // Verify: Only 10 new tags synced
}
```

## Estimated Effort

**Critical TODOs**: 4-5 days (combined with watermark fix)

- Incremental catchup trigger: 4 hours
- Checkpoint persistence: 1 day
- HLC parsing: 2 hours
- Testing: 2 days

**Important TODOs**: 3-4 days

- Multi-peer backfill: 2 days
- Protocol handler cleanup: 1 day
- Testing: 1 day

**Cleanup TODOs**: 2-3 days

- Metrics persistence: 1 day
- Progress calculation: 4 hours
- Latency metrics: 4 hours
- Remove legacy code: 4 hours

**Total**: 2 weeks for complete TODO resolution + watermark fix

## Priority Matrix

| TODO                       | Severity  | Effort | Priority         |
| -------------------------- | --------- | ------ | ---------------- |
| Incremental state catchup  | Critical  | Low    | P0 - Fix now     |
| Checkpoint persistence     | Critical  | Medium | P0 - Fix now     |
| Per-resource watermarks    | Critical  | Medium | P0 - Fix now     |
| HLC parsing                | Important | Low    | P1 - Fix now     |
| Multi-peer backfill        | Important | High   | P2 - Next sprint |
| Protocol handler cleanup   | Important | Low    | P2 - Next sprint |
| Peer disconnection handler | Important | Medium | P2 - Next sprint |
| Metrics persistence        | Minor     | Medium | P3 - Future      |
| Progress calculation       | Minor     | Low    | P3 - Future      |
| Latency metrics            | Minor     | Medium | P3 - Future      |
| Legacy sequence cleanup    | Minor     | Low    | P3 - Future      |

## Recommended Approach

### Week 1: Core Sync Correctness

- Per-resource watermarks in sync.db
- Checkpoint persistence in sync.db
- Incremental state catchup trigger
- HLC FromStr implementation
- All unit + integration tests
- Update documentation

### Week 2: Reliability & Cleanup

- ️ Peer disconnection recovery
- ️ Protocol handler cleanup
- ️ Multi-peer backfill (nice to have)

### Week 3+: Polish

- Metrics persistence
- Accurate progress
- Real latency measurements
- Remove legacy code

## Success Criteria

After Week 1 (critical fixes):

- Per-resource watermarks prevent data loss
- Backfill survives daemon crashes
- Incremental sync works for both state and shared resources
- All sync integration tests pass
- No data loss scenarios
- Clear separation: sync.db = all sync metadata

## References

- Watermark fix plan: `SYNC_WATERMARK_FIX_PLAN.md`
- Bug analysis: `SYNC_WATERMARK_ANALYSIS.md`
- Test findings: `SYNC_WATERMARK_TEST_FINDINGS.md`
- Failing test: `core/tests/sync_watermark_unit_test.rs`
