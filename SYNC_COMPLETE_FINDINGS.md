# Sync System Deep Dive - Complete Findings

**Date**: November 14, 2025
**Investigation**: Library sync watermark issues at 10k+ entries
**Status**: Bug confirmed, fix planned, critical TODOs identified

---

## Executive Summary

Your intuition was **100% correct**. The library sync system uses **global watermarks** (one per device) instead of **per-resource watermarks**, causing data loss after 10k+ entries.

Additionally, the investigation uncovered **3 critical TODOs** in the sync system that prevent watermark-based incremental sync from working at all.

---

## The Watermark Bug

### Root Cause

The sync system stores **two global watermarks** per device:

```rust
// database.db - devices table
last_state_watermark: Option<DateTime>  // For ALL device-owned resources
last_shared_watermark: Option<String>   // For ALL shared resources (HLC)
```

When Device B syncs from Device A:

1. Syncs 10,000 entries → watermark advances to T+30
2. Disconnects before syncing locations (timestamps T+0 to T+10)
3. Reconnects, queries: `WHERE updated_at >= T+30`
4. **Locations permanently lost** (T+0 < T+30)

### Proof

**Unit test** (`core/tests/sync_watermark_unit_test.rs`) **proves the bug**:

```
BUG CONFIRMED!
   We have 10 locations in the database with timestamps T+0 to T+10,
   but query with watermark T+25 returns 0 locations.
```

Run: `cargo test -p sd-core --test sync_watermark_unit_test -- --nocapture`

### Why It Manifests at 10k

- Default batch size: 10,000 records
- < 10k entries: Everything fits in one batch, no issue
- \> 10k entries: Multiple batches, watermark advances mid-sync
- If disconnection happens between batches, earlier resources lost

---

## Critical TODOs Discovered

### 1. Incremental State Catchup Doesn't Trigger

**Location**: `core/src/service/sync/peer.rs:518`

```rust
if we_need_state_catchup {
    // TODO: Implement incremental state request
    warn!("Incremental state catch-up not yet implemented, will use backfill");
}
```

**Impact**: Watermark exchange detects divergence but **doesn't trigger catchup**! Always does full backfill instead.

**Fix Required**: Wire `on_watermark_exchange_response()` to call `BackfillManager::catch_up_from_peer()`

---

### 2. Checkpoint Persistence Not Implemented

**Location**: `core/src/service/sync/state.rs:251-260`

```rust
pub async fn save(&self) -> Result<(), std::io::Error> {
    // TODO: Persist to disk for crash recovery
    Ok(())
}
```

**Impact**: If daemon crashes during backfill, **must restart from beginning**.

**Fix Required**: Store checkpoints in `sync.db` for resumability

---

### 3. HLC Incremental Sync Disabled

**Location**: `core/src/service/sync/backfill.rs:209`

```rust
// TODO: Parse HLC from string watermark when HLC implements FromStr
let max_shared_hlc = self.backfill_shared_resources(peer).await?;
// ^^^ Always does FULL sync, not incremental!
```

**Impact**: Shared resources (tags, collections) **always do full sync** instead of incremental.

**Fix Required**: Implement `FromStr` for HLC

---

## The Fix

### Architecture Change

Move watermarks from `database.db` to `sync.db` with per-resource tracking:

```sql
-- sync.db (NEW table)
CREATE TABLE device_resource_watermarks (
    device_uuid TEXT NOT NULL,
    peer_device_uuid TEXT NOT NULL,
    resource_type TEXT NOT NULL,      -- "location", "entry", "volume"
    last_watermark TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
);

-- database.db (REMOVE from devices table)
ALTER TABLE devices DROP COLUMN last_state_watermark;
ALTER TABLE devices DROP COLUMN last_shared_watermark;
```

### Benefits

1. **Correctness**: Each resource syncs independently
2. **Clean separation**: sync.db = coordination, database.db = domain
3. **No foreign keys**: UUID-based, simple lookups
4. **Breaking change OK**: Unreleased software, fresh start
5. **Consistent design**: Matches `shared_changes` and `peer_acks` pattern

---

## Implementation Plan

### Week 1: Critical Fixes (P0)

**Day 1-2**: Sync DB Schema

- Create `device_resource_watermarks` table
- Create `backfill_checkpoints` table
- Create `ResourceWatermarkStore` helper
- Remove old watermark columns from devices

**Day 3-5**: Core Logic

- Update `PeerSync` to use per-resource watermarks
- Implement checkpoint save/load
- Wire up incremental state catchup trigger
- Implement `HLC::from_str()`

**Day 5-7**: Testing

- Fix/update all existing sync tests
- Verify watermark unit test passes
- Integration testing
- Manual testing with real data

### Week 2: Important Fixes (P1-P2)

- Multi-peer backfill
- Peer disconnection recovery
- Protocol handler cleanup

### Week 3+: Polish (P3)

- Metrics persistence
- Progress calculation
- Latency measurements
- Legacy code removal

---

## Files Affected

### New Files

- `core/src/infra/sync/watermarks.rs` - Per-resource watermark store
- `core/src/infra/sync/checkpoints.rs` - Checkpoint persistence

### Modified Files

- `core/src/infra/sync/db.rs` - Init new tables
- `core/src/service/sync/peer.rs` - Use per-resource watermarks, trigger catchup
- `core/src/service/sync/backfill.rs` - Load/save checkpoints, parse HLC
- `core/src/service/sync/mod.rs` - Pass sync_db, wire backfill_manager
- `core/src/service/sync/state.rs` - Implement checkpoint persistence
- `core/src/infra/sync/hlc.rs` - Add FromStr
- `core/src/infra/db/migration/m20251115_000001_remove_watermarks.rs` - Drop old columns
- `core/src/infra/db/entities/device.rs` - Remove watermark fields
- `core/tests/sync_integration_test.rs` - Update tests
- `core/tests/sync_watermark_unit_test.rs` - Should pass after fix

---

## Test Suite

### Existing Tests (should still pass)

- `test_sync_location_device_owned_state_based`
- `test_sync_tag_shared_hlc_based`
- `test_sync_backfill_includes_pre_sync_data`
- `test_watermark_reconnection_sync`
- `test_connection_state_tracking`
- All other sync integration tests

### New Tests (prove bug + verify fix)

- `test_global_watermark_filters_earlier_resources` - Proves bug
- `test_per_resource_watermark_solution` - Documents fix
- `test_incremental_state_catchup_triggered` - Verifies TODO #1 fixed
- `test_checkpoint_persistence_and_resume` - Verifies TODO #2 fixed
- `test_shared_resource_incremental_catchup` - Verifies TODO #3 fixed

---

## Risk Assessment

### With Current System (Unpatched)

**Data Loss Risk**: HIGH

- Any disconnection during backfill of 10k+ entries
- Affects: locations, volumes, any device-owned resource with earlier timestamps
- Probability: Increases with library size

**Reliability Risk**: MEDIUM

- Daemon crashes lose backfill progress
- Always full sync instead of incremental (performance impact)
- No automatic recovery from peer disconnections

### With Proposed Fix

**Data Loss Risk**: ELIMINATED

- Per-resource watermarks prevent cross-contamination
- Each resource resumes from its own checkpoint

**Reliability Risk**: SIGNIFICANTLY REDUCED

- Checkpoint persistence enables crash recovery
- Incremental sync reduces data transfer
- Automatic peer switching (Week 2 addition)

---

## Breaking Changes

Since this is unreleased software:

1. **Watermark storage moved** to sync.db
2. **Old watermark columns removed** from devices table
3. **First sync after update** = full backfill (expected, acceptable)
4. **sync.db schema version bumped**

Users will see: "Sync protocol updated, performing initial sync..."

---

## Documentation Updates Needed

After implementation:

1. Update `/docs/core/library-sync.mdx`:

   - Document per-resource watermarks
   - Update watermark section examples
   - Add troubleshooting for sync.db

2. Add `/docs/core/sync-troubleshooting.mdx`:

   - How to inspect sync.db
   - Common issues and solutions
   - Manual recovery procedures

3. Update workspace rules:
   - sync.db contains all coordination metadata
   - Never mix sync metadata in database.db

---

## Summary

### What We Found

1. **Watermark bug confirmed** - Global watermarks cause data loss
2. **3 critical TODOs** - Prevent incremental sync from working
3. **Test created** - Proves bug exists (will verify fix works)
4. **Fix designed** - Per-resource watermarks in sync.db
5. **Implementation plan** - 2 weeks for complete resolution

### What's Next

**Immediate** (This PR):

- Implement per-resource watermarks
- Fix critical TODOs (#1, #2, #3)
- Update all tests
- Clean separation of concerns

**Follow-up** (Next PR):

- Multi-peer backfill
- Peer disconnection recovery
- Performance optimizations

**Future**:

- Metrics persistence
- Code cleanup
- Advanced features

### Deliverables

**Analysis Documents**:

- `SYNC_WATERMARK_ANALYSIS.md` - Technical deep dive
- `SYNC_WATERMARK_ISSUE_SUMMARY.md` - Visual diagrams
- `SYNC_WATERMARK_FIX_PLAN.md` - Implementation plan
- `SYNC_WATERMARK_TEST_FINDINGS.md` - Test results
- `SYNC_TODO_RESOLUTION.md` - TODO tracking (this file)

**Tests**:

- `core/tests/sync_watermark_unit_test.rs` - **Proves bug**
- `core/tests/sync_watermark_bug_test.rs` - Integration test

**Ready to Implement**:

- All code examples written
- Schema migrations defined
- Test cases specified
- Timeline estimated

The sync system will be **production-ready** after these fixes!
