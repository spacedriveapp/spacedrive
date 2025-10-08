# Library Sync Implementation Roadmap

**Current Status**: Phase 1 & 2 Complete  
**Last Updated**: 2025-10-08

---

## ✅ Completed (Phases 1 & 2)

### Phase 1: Foundation
- ✅ **LSYNC-008**: Sync Log Schema (separate DB)
- ✅ **LSYNC-007**: Syncable Trait + Registry
- ✅ **LSYNC-009**: Leader Election
- ✅ **LSYNC-006**: TransactionManager

### Phase 2: Protocol & Service  
- ✅ **LSYNC-013**: Sync Protocol Handler (push-based)
- ✅ **LSYNC-010**: Sync Service (leader & follower)

### Integration
- ✅ Library lifecycle integration
- ✅ Location model (first syncable entity)
- ✅ Zero-touch registry architecture

**Total**: ~3,000 lines, 18+ tests passing

---

## 🎯 Next Steps (Prioritized)

### Option A: Complete Core Sync (Recommended) 

Build out the **minimum viable sync** before adding more models:

#### A1. **Network Integration** (Critical Gap)
**Current State**: Protocol handler exists but isn't wired to networking  
**What's Missing**:
- SyncProtocolHandler not registered in NetworkingService
- No actual BiStream connections for sync messages
- notify_followers() and request_entries() are stubs

**Tasks**:
1. Register SyncProtocolHandler when library opens
2. Connect to SYNC_ALPN streams
3. Implement actual message sending via Iroh
4. Add connection lifecycle management

**Files**:
- `core/src/service/network/core/mod.rs` - Register sync protocol
- `core/src/library/manager.rs` - Create & register handler on open
- `core/src/service/sync/leader.rs` - Use protocol handler to push
- `core/src/service/sync/follower.rs` - Use protocol handler to pull

**Estimate**: 2-3 hours  
**Priority**: **CRITICAL** - Without this, sync doesn't actually work!

---

#### A2. **InitialSyncJob** (New Device Pairing)
**Purpose**: When a device first pairs, pull all history from leader

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct InitialSyncJob {
    library_id: Uuid,
    leader_device_id: Uuid,
    
    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    current_sequence: Option<u64>,
}

impl JobHandler for InitialSyncJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<SyncOutput> {
        // 1. Get leader's latest sequence
        // 2. Pull entries in batches (1000 at a time)
        // 3. Apply via SyncApplier
        // 4. Track progress, checkpoint for resumability
    }
}
```

**Location**: `core/src/ops/sync/initial_sync/`  
**Estimate**: 3-4 hours  
**Priority**: **HIGH** - Needed for multi-device setup

---

#### A3. **BackfillSyncJob** (Catch-Up Sync)
**Purpose**: When device reconnects after being offline

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct BackfillSyncJob {
    library_id: Uuid,
    leader_device_id: Uuid,
    from_sequence: u64,
    to_sequence: u64,
    
    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    current_sequence: Option<u64>,
}
```

**Similar to InitialSyncJob but incremental**

**Location**: `core/src/ops/sync/backfill/`  
**Estimate**: 2 hours (reuses InitialSyncJob logic)  
**Priority**: **MEDIUM** - Can be added later

---

### Option B: Add More Syncable Models

Expand sync to cover more of the data model:

#### B1. **Tag Sync** (LSYNC-002 partial)
**What**: Sync tag definitions across devices

```rust
// core/src/infra/db/entities/tag.rs
impl Syncable for tag::Model {
    async fn apply_sync_entry(...) { /* ~40 lines */ }
}
crate::register_syncable_model!(Model);
```

**Estimate**: 1 hour per model  
**Priority**: **MEDIUM** - Nice to have, not critical for MVP

---

#### B2. **Collection Sync**
Same pattern as Tag

---

#### B3. **Entry Sync** (LSYNC-012 - Complex!)
**Challenge**: 1M+ files = bulk optimization needed

**Approach**:
- Bulk operations create metadata-only sync logs
- Follower triggers own indexing jobs (doesn't replicate 1M entries)
- Special handling in TransactionManager.log_bulk()

**Estimate**: 5-6 hours (complex)  
**Priority**: **HIGH** but after network integration

---

### Option C: Production Readiness

Make sync production-ready:

#### C1. **Database Migration** (Add version fields)
```sql
ALTER TABLE locations ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tag ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE collection ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE user_metadata ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
```

**Location**: `core/src/infra/db/migration/m20250108_000001_add_sync_version_fields.rs`  
**Estimate**: 30 minutes  
**Priority**: **MEDIUM** - Currently using placeholder version=1

---

#### C2. **Conflict Resolution UI** (LSYNC-011)
**Current**: Version-based LWW (automatic)  
**Enhancement**: UI for manual conflict resolution

**Priority**: **LOW** - Automatic resolution works for metadata

---

#### C3. **Persist Leadership State**
**Current**: LeadershipManager state is in-memory only  
**Enhancement**: Persist to device's `sync_leadership` JSON field

**Estimate**: 1 hour  
**Priority**: **MEDIUM** - Leader failover works but doesn't persist

---

## 📋 Recommended Implementation Order

### Sprint 1: Make Sync Actually Work (Week 1)
**Goal**: End-to-end sync working between two devices

1. **Network Integration** (A1) - 2-3 hours ⭐ **CRITICAL**
   - Register SyncProtocolHandler
   - Wire up BiStreams
   - Actually send/receive messages

2. **Test End-to-End**
   - Create location on Device A
   - Verify it syncs to Device B
   - Debug any issues

3. **InitialSyncJob** (A2) - 3-4 hours
   - For multi-device setup
   - Pull full history

**Deliverable**: Demo syncing a location between two devices!

---

### Sprint 2: Expand Data Coverage (Week 2)
**Goal**: Sync tags, collections, and user metadata

1. **Tag Sync** (B1) - 1 hour
2. **Collection Sync** (B2) - 1 hour  
3. **UserMetadata Sync** - 1 hour
4. **Junction Tables** (user_metadata_tag) - 2 hours

**Deliverable**: Full metadata sync working!

---

### Sprint 3: Entry Sync (Week 3)
**Goal**: Sync file/folder entries with bulk optimization

1. **Entry Model Syncable** - 2 hours
2. **Bulk Optimization** (B3) - 4 hours
3. **Watcher Integration** - 3 hours

**Deliverable**: Filesystem changes sync between devices!

---

### Sprint 4: Production Polish (Week 4)
**Goal**: Production-ready sync

1. **Database Migration** (C1) - 30 min
2. **Persist Leadership** (C3) - 1 hour
3. **BackfillSyncJob** (A3) - 2 hours
4. **Error Handling & Retry Logic** - 2 hours
5. **Performance Testing** - 4 hours

**Deliverable**: Production-ready sync system!

---

## 🎯 Immediate Next Step (My Recommendation)

**A1: Network Integration** - Make sync actually work!

This is the **critical missing piece**. Everything else is built, but messages aren't actually flowing over the network.

### What to Build:

```rust
// 1. In LibraryManager::open_library()
let sync_handler = Arc::new(SyncProtocolHandler::new(
    library.id(),
    library.sync_log_db().clone(),
    context.networking.device_registry(),
    role,
));

// 2. Register with networking
context.networking
    .protocol_registry()
    .write()
    .await
    .register_handler(sync_handler)?;

// 3. In LeaderSync - actually push
let protocol = get_protocol_handler("sync")?;
protocol.notify_followers(from_seq, to_seq).await?;

// 4. In FollowerSync - actually pull
let protocol = get_protocol_handler("sync")?;
let entries = protocol.request_entries(leader_id, last_seq, 100).await?;
```

### Acceptance Criteria:
- [ ] SyncProtocolHandler registered when library opens
- [ ] Leader can send NewEntries over network
- [ ] Follower receives and applies entries
- [ ] Location syncs between two devices in real-time

**This is the capstone that makes everything work!**

---

## Alternative: Jobs First (If You Prefer)

If you want to build jobs before network integration:

**InitialSyncJob** can work with stub networking - useful for testing the job pattern.

---

## What Would You Like to Do?

**Option 1**: 🔥 **Network Integration** (make sync work end-to-end)  
**Option 2**: 📦 **InitialSyncJob** (build job pattern first)  
**Option 3**: 🏷️ **Add Tag/Collection Sync** (expand coverage)  
**Option 4**: 🗄️ **Database Migration** (add version fields)

**My recommendation**: **Option 1** - Let's make sync actually work over the network! Then we can test it and build from there.
