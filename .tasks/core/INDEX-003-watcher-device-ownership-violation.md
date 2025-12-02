---
id: INDEX-003
title: Fix Watcher Device Ownership Violation (CRITICAL)
status: To Do
assignee: james
priority: Critical
tags: [watcher, sync, bug, security]
last_updated: 2025-10-23
related_tasks: [INDEX-001, LSYNC-010]
---

# Fix Watcher Device Ownership Violation (CRITICAL)

## Problem Statement

**CRITICAL BUG**: The location watcher violates device ownership by watching and modifying locations owned by other devices.

### Bug Scenario

1. Device A creates location `/Users/jamespine/Desktop` → `device_id = A`
2. Location syncs to Device B's database
3. Device B also has `/Users/jamespine/Desktop` on its local filesystem
4. Device B's watcher **incorrectly** starts watching Device A's location
5. When files change on Device B's desktop, the watcher triggers indexer
6. Device B modifies Device A's entries → **OWNERSHIP VIOLATION** ❌

###Current Code (Bug)

`core/src/service/watcher/mod.rs:~493`
```rust
// Load all locations from database (NO DEVICE CHECK!)
let locations = entities::location::Entity::find()
    .all(db)  // <-- Gets ALL devices' locations
    .await?;

for location in locations {
    let path = PathResolver::get_full_path(db, entry_id).await?;

    // If path exists locally, start watching
    if path.exists() {
        self.add_location(watched_location).await?;  // BUG!
    }
}
```

## Impact

**Severity**: CRITICAL - Data corruption / sync integrity violation

**Consequences**:
- Device B can modify Device A's location metadata
- Entries get incorrectly attributed to wrong devices
- Sync state becomes corrupted
- Cannot determine authoritative source of changes
- Breaks the fundamental device-owned sync model

**Reproduction**:
1. Have two devices with same username (common: `/Users/jamespine/`)
2. Add Desktop location on Device A
3. Sync to Device B
4. Create a file on Device B's desktop
5. **Bug**: Device B's watcher modifies Device A's location

## Root Cause

**TWO separate bugs:**

### Bug 1: Watcher loads all devices' locations

The watcher's `load_locations_from_database()` method (line ~493) queries **all locations** without filtering by device ownership:

```rust
let locations = entities::location::Entity::find()
    .all(db)  // Gets locations from ALL devices
    .await?;
```

It then checks if the path exists locally and starts watching if it does, **regardless of which device owns the location**.

### Bug 2: Responder looks up parents by path without location scoping

The responder's `create_entry()` looks up parent entries by path string:

```rust
// entry.rs:234-236
entities::directory_paths::Entity::find()
    .filter(entities::directory_paths::Column::Path.eq(&parent_path_str))
    .one(ctx.library_db())
```

If Device A and Device B both have `/Users/jamespine/Desktop` locations, this query could return **EITHER** device's entry (whichever is first in the table).

**Impact**: Even with Bug 1 fixed, if `location_id` isn't used to scope parent lookup, entries could be created under the wrong location's tree.

## Solution

### Fix 1: Filter Locations by Device (DONE ✅)

**File**: `core/src/service/watcher/mod.rs:487-520`

Only watch locations owned by the current device:

```rust
// Get current device UUID
let current_device_uuid = crate::device::get_current_device_id();

// Get device's integer ID
let current_device = device::Entity::find()
    .filter(device::Column::Uuid.eq(current_device_uuid))
    .one(db)
    .await?;

// Filter locations by current device
let locations = location::Entity::find()
    .filter(location::Column::DeviceId.eq(current_device.id))
    .all(db)
    .await?;
```

### Fix 2: Add Safety Check in add_location() (DONE ✅)

**File**: `core/src/service/watcher/mod.rs:356-388`

Add runtime ownership validation before watching:

```rust
pub async fn add_location(&self, location: WatchedLocation) -> Result<()> {
    // Verify this device owns the location
    let location_record = entities::location::Entity::find()
        .filter(entities::location::Column::Uuid.eq(location.id))
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("Location not found"))?;

    let current_device_id = self.context.device().id();

    if location_record.device_id != current_device_id {
        return Err(anyhow!(
            "Cannot watch location {} owned by device {} (current device: {})",
            location.id,
            location_record.device_id,
            current_device_id
        ));
    }

    // ... rest of add_location logic
}
```

### Fix 3: Add Integration Test

```rust
#[tokio::test]
async fn test_watcher_respects_device_ownership() {
    let (device_a, device_b) = setup_paired_devices().await;

    // Device A creates location
    let location_a = create_location(device_a, "/Users/test/Desktop").await;

    // Sync to device B
    wait_for_sync().await;

    // Device B should NOT watch device A's location
    let watched = device_b.watcher().get_watched_locations().await;
    assert!(!watched.iter().any(|l| l.id == location_a.uuid));

    // Device B creates its own location with same path
    let location_b = create_location(device_b, "/Users/test/Desktop").await;

    // Device B SHOULD watch its own location
    let watched = device_b.watcher().get_watched_locations().await;
    assert!(watched.iter().any(|l| l.id == location_b.uuid));
}
```

## Implementation Plan

### Phase 1: Watcher Device Filtering (DONE ✅)

**Files**: `core/src/service/watcher/mod.rs`

- [x] Filter locations by device_id in `load_locations_from_database()`
- [x] Add runtime ownership check in `add_location()`
- [x] Add necessary SeaORM imports

### Phase 2: Scope Responder Path Lookups (TODO - CRITICAL)

**Files**: `core/src/ops/indexing/responder.rs`

Required changes:
1. Thread `location_id` through all handlers:
   - `handle_modify(ctx, path, location_id, ...) `
   - `handle_remove(ctx, path, location_id, ...)`
   - `handle_rename(ctx, from, to, location_id, ...)`

2. Look up location root entry ID at start of each handler:
   ```rust
   let location_record = location::Entity::find()
       .filter(location::Column::Uuid.eq(location_id))
       .one(db).await?;
   let location_root_entry_id = location_record.entry_id.unwrap();
   ```

3. Update resolve functions to accept and use `location_root_entry_id`:
   - `resolve_directory_entry_id(ctx, path, location_root_entry_id)`
   - `resolve_file_entry_id(ctx, path, location_root_entry_id)`

4. Add `entry_closure` JOIN to scope queries:
   ```sql
   SELECT dp.entry_id
   FROM directory_paths dp
   INNER JOIN entry_closure ec ON ec.descendant_id = dp.entry_id
   WHERE dp.path = ? AND ec.ancestor_id = ?
   ```

5. Update `entry.rs` parent lookup (line 234) with same pattern

**Estimated time**: 2-3 hours

### Phase 3: Testing (1-2 hours)

**File**: `core/tests/indexing_multi_device_test.rs` (new)

```rust
#[tokio::test]
async fn test_responder_scopes_to_correct_location() {
    // Setup: Two devices, both with /Users/test/Desktop
    let (device_a, device_b) = setup_paired_devices().await;

    let location_a = create_location(device_a, "/Users/test/Desktop").await;
    let location_b = create_location(device_b, "/Users/test/Desktop").await;

    // Both create test.txt on their respective desktops
    create_file(device_a, "/Users/test/Desktop/test.txt").await;
    create_file(device_b, "/Users/test/Desktop/test.txt").await;

    // Verify each device's watcher only modified its own location's entries
    let entries_a = get_entries_for_location(location_a.id).await;
    let entries_b = get_entries_for_location(location_b.id).await;

    assert_eq!(entries_a.len(), 2); // Desktop + test.txt
    assert_eq!(entries_b.len(), 2); // Desktop + test.txt

    // Verify no cross-contamination
    assert!(entries_a.iter().all(|e| is_descendant_of(e.id, location_a.entry_id)));
    assert!(entries_b.iter().all(|e| is_descendant_of(e.id, location_b.entry_id)));
}
```

### Phase 4: Audit (30 minutes)

Check for similar path-based queries elsewhere:
- `PathResolver::get_full_path()` - Does it need scoping?
- File operations (copy/move/delete) - Do they scope by location?
- Any other `directory_paths WHERE path = ?` queries

## Acceptance Criteria

### Phase 1 (Watcher) - COMPLETED ✅
- [x] Watcher filters locations by current device ID
- [x] `add_location()` validates device ownership
- [x] Builds successfully
- [x] Device A's location not watched on Device B

### Phase 2 (Responder) - TODO
- [ ] All handlers receive `location_id` parameter
- [ ] `resolve_directory_entry_id()` scoped by location using `entry_closure` JOIN
- [ ] `resolve_file_entry_id()` scoped by location using `entry_closure` JOIN
- [ ] `entry.rs` parent lookup scoped by location
- [ ] Integration test passes for multi-device same-path scenario
- [ ] Existing tests still pass
- [ ] Both devices can have same path without cross-contamination

## Testing Strategy

### Manual Test
```bash
# On Device A
sd location add "/Users/jamespine/Desktop"

# On Device B (after sync)
sd sync wait  # Wait for location to sync

# Verify Device B is NOT watching Device A's location
sd watcher status
# Should show 0 watched locations (or only Device B's own locations)

# Create a file on Device B's desktop
touch "/Users/jamespine/Desktop/test.txt"

# Wait a few seconds for watcher
sleep 5

# Query Device A's location entries from Device B
sd --instance jam query entries --location <device-a-location-id>
# Should NOT include test.txt (Device B didn't modify Device A's location)
```

### Integration Test
Run test suite with device ownership checks enabled:
```bash
cargo test --lib watcher::test_watcher_respects_device_ownership
```

## Migration Notes

**Breaking Change**: If any users have been affected by this bug, their databases may contain corrupted entries where Device B modified Device A's location.

**Cleanup Strategy** (optional future work):
1. Query for entries where `location.device_id != device_that_created_entry`
2. Mark these as "orphaned" or "corrupted"
3. Allow user to reassign to correct device or delete

### Fix 3: Scope Responder Path Lookups by Location (TODO - CRITICAL)

**Problem**: The responder's resolve functions query entries by path alone, without location scoping:

**Vulnerable functions** (`core/src/ops/indexing/responder.rs`):
- `resolve_directory_entry_id()` (line 397) - Used by modify/remove/rename
- `resolve_file_entry_id()` (line 415) - Used by modify/remove/rename
- Parent lookup in `entry.rs` (line 234) - Used by create

**Current behavior (BROKEN)**:
```rust
// Queries by path only - could match ANY device's entry!
directory_paths::Entity::find()
    .filter(directory_paths::Column::Path.eq(path_str))
    .one(db)
```

**Correct approach (like the indexer)**:
```rust
// Scope to location's entry tree using entry_closure
async fn resolve_directory_entry_id_scoped(
    ctx: &impl IndexingCtx,
    abs_path: &Path,
    location_root_entry_id: i32,  // <-- Add this
) -> Result<Option<i32>> {
    let path_str = abs_path.to_string_lossy().to_string();

    // Query directory_paths and JOIN with entry_closure to scope by location
    let result = ctx.library_db()
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT dp.entry_id
            FROM directory_paths dp
            INNER JOIN entry_closure ec ON ec.descendant_id = dp.entry_id
            WHERE dp.path = ?
              AND ec.ancestor_id = ?
            "#,
            vec![path_str.into(), location_root_entry_id.into()],
        ))
        .await?;

    Ok(result.map(|row| row.try_get::<i32>("", "entry_id").ok()).flatten())
}
```

**Implementation plan**:
1. Thread `location_id` through all responder handlers
2. Look up `location_root_entry_id` once at start of each handler
3. Pass to all resolve functions
4. Add JOIN with `entry_closure` to scope queries
5. Apply same pattern to `entry.rs` parent lookup

**Why this is better than cache**:
- Works across all operations (create/modify/remove/rename)
- Survives state resets and restarts
- Database-backed correctness (not in-memory heuristic)
- Matches proven indexer pattern
- Prevents cross-device contamination definitively

**Status**: TODO - requires refactoring responder signatures

## Comparison: Indexer vs Responder

| Aspect | Indexer | Responder (Current) | Responder (After Fix) |
|--------|---------|---------------------|----------------------|
| Has location_id? | Yes | Yes (but unused) | Yes (used) |
| Scoping method | `entry_closure` JOIN | None | `entry_closure` JOIN |
| Cache seeding | Yes (line 61-63) | Yes (my fix) | Yes (keep as optimization) |
| Path queries scoped? | Yes | No | Yes |
| Safe for multi-device? | Yes | No | Yes |

## Related Issues

- Entry device ownership filtering during sync (separate concern)
- Sync integrity validation
- Location transfer ownership on volume move

## References

- [Location Watcher Service](../../core/src/service/watcher/mod.rs)
- [LSYNC-010](./LSYNC-010-sync-service.md) - Device-owned sync model
- [INDEX-001](./INDEX-001-location-watcher-service.md) - Watcher architecture
