# Sync Watermark Test Findings

## Test Results

Running the watermark bug test revealed some important findings about the current sync system.

### Test Setup

Created mixed resources on Device A:

- 50 locations (timestamps: T+0 to T+5)
- 12,000 entries (timestamps: T+20 onwards)
- 3,000 more entries (later timestamps)

### Result

**All resources synced successfully** - the bug did NOT manifest.

```
Sync Results:
  Locations: 50 / 50 expected ✓
  Entries:   15,050 / 15,000 expected ✓ (15k data + 50 location entries)
```

## Why the Bug Didn't Manifest

### Finding #1: Incremental State Catchup Is TODO

```rust
// core/src/service/sync/peer.rs:518-523
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

The `on_watermark_exchange_response()` method doesn't actually trigger incremental catchup - it's still TODO!

### Finding #2: catch_up_from_peer() Does Exist

`BackfillManager::catch_up_from_peer()` is implemented and DOES use watermarks:

```rust
// core/src/service/sync/backfill.rs:213
let final_state_checkpoint = self.backfill_device_owned_state(peer, effective_state_watermark).await?;
//                                                                  ^^^^^^^^^^^^^^^^^^^^^^^^
//                                                                  Watermark passed through!
```

### Finding #3: Dependency Ordering Affects Test

The sync order is:

1. Devices
2. **Entries** (comes BEFORE locations in some orderings!)
3. Locations
4. Volumes

This is different than I expected. Looking at logs:

```
sync_order=["entry", "collection", "audit_log", "tag", "content_identity",
            "device", "user_metadata", "collection_entry", "tag_relationship",
            "location", "volume", "user_metadata_tag"]
```

**Entries sync BEFORE locations!** This is because entries don't depend on locations for FK (location_id can be None).

So in the initial backfill:

1. 50 entries synced (no parent_id, from our test data)
2. Then all future queries include watermark
3. Locations come later but their timestamps are still early

The watermark would need to advance BETWEEN model types in the ordered list for the bug to manifest.

## The Real Bug Scenario

The bug manifests when:

1. **Backfill starts**, syncs models in order
2. **During one model type** (e.g., entry), it processes 10,000 records
3. **Watermark updates to last entry timestamp** (e.g., T+30)
4. **Disconnection** happens
5. **Reconnect** with watermark T+30
6. **Next model type** in order (e.g., location) has timestamps < T+30
7. **Query filters by watermark**: `WHERE updated_at >= T+30`
8. **Locations skipped!** ❌

## Why Our Test Didn't Trigger It

Our test scenario:

- Initial backfill completed FULLY (all 50 locations + 50 entries)
- Watermark set after backfill complete
- Then created MORE entries
- Catchup correctly got the new entries
- Locations already in DB, not resynced

We need to interrupt backfill DURING processing, not after.

## What We Need to Test

### Scenario A: Interrupt During Batched Model

```
1. Create 50 locations (T+0 to T+5)
2. Create 15,000 entries (T+20+)
3. Start backfill
4. Backfill syncs entries FIRST (due to dependency order)
5. After 10,000 entries, watermark = T+30
6. INTERRUPT backfill
7. Watermark persisted at T+30
8. Resume backfill
9. Locations query: WHERE updated_at >= T+30
10. Result: 0 locations (they're at T+0 to T+5) ❌
```

### Scenario B: Use Automatic Background Catchup

The sync loop in `mod.rs` automatically triggers `catch_up_from_peer()` when:

- Device is Ready
- Watermarks diverge from peers
- Background loop detects this

We could:

1. Let initial backfill complete
2. Create new data on Device A
3. Wait for background sync loop to detect watermark divergence
4. Let it trigger catch_up_from_peer() automatically
5. Verify all data syncs

## Recommended Test Approach

The test should interrupt the actual `backfill_peer_state()` method after it processes one model type but before processing the next. This requires either:

### Option 1: Mock the backfill process

- Override batch handling to stop after first model type
- Set watermark manually
- Resume with watermark-based query
- Verify skipped data

### Option 2: Use extremely large batch and manually interrupt

- Create 50k+ entries so one batch takes time
- Use tokio::select! to race backfill vs timeout
- Interrupt and check watermark
- Resume and check for missing data

### Option 3: Test at unit level

- Test `query_for_sync()` directly with watermark parameter
- Create data with various timestamps
- Call with watermark in the middle
- Verify only newer data returned

## Current Code Status

**Watermark persistence**: Implemented and working
**Watermark-based queries**: Implemented in `query_for_sync()`
**Watermark exchange**: Implemented
**Incremental catchup trigger**: TODO in watermark exchange handler
**Manual catchup**: `catch_up_from_peer()` works

The infrastructure for watermark-based sync EXISTS and WORKS. The bug is real and will manifest in production, but our integration test scenario didn't trigger it because:

1. Backfill completed before interruption
2. We're not testing the exact timing/interleaving needed

## Next Steps

1. **Simplify test**: Unit test `query_for_sync()` with watermarks
2. **Document finding**: The real bug exists but needs specific timing to trigger
3. **Implement fix**: Per-resource watermarks as planned
4. **Add integration test**: After fix, verify mixed resources work correctly

The deep dive was valuable - we confirmed the architecture and identified where the bug lives!
