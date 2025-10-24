---
id: INDEX-004
title: Nested Locations Support (Entry Reuse Architecture)
status: To Do
assignee: james
priority: Medium
tags: [indexing, locations, architecture, sync]
last_updated: 2025-10-23
related_tasks: [INDEX-001, INDEX-003, CORE-001, LSYNC-010]
---

# Nested Locations Support (Entry Reuse Architecture)

## Problem Statement

Locations are currently implemented as isolated entry trees with orphan roots (`parent_id: null`). This prevents nested locations where a subdirectory of one location becomes its own location.

The design intent is for locations to be **virtual organizational concepts** that reference existing entries in the unified entry tree, not isolated silos. This would enable:

- Nested locations without entry duplication
- Flexible location hierarchies
- More granular indexing control
- Reduced storage overhead

## Current Behavior (Entry Duplication)

```
Filesystem:
/Users/jamespine/Documents/
├── Work/
│   ├── project.txt
│   └── notes.md
└── Personal/

User actions:
1. sd location add "/Users/jamespine/Documents"
2. sd location add "/Users/jamespine/Documents/Work"

Database result (BROKEN):
entries:
  1: Documents (parent: null) ← Location A root
  2: Work      (parent: 1)    ← Created by Location A indexing
  3: Personal  (parent: 1)
  4: project.txt (parent: 2)
  5: notes.md    (parent: 2)
  100: Work    (parent: null) ← Location B root (DUPLICATE!)
  101: project.txt (parent: 100) ← DUPLICATE!
  102: notes.md    (parent: 100) ← DUPLICATE!

locations:
  Location A: entry_id = 1
  Location B: entry_id = 100

Issues:
Entry duplication (Work, project.txt, notes.md exist twice)
Broken tree (Location B's root is orphaned)
Wasted storage (same data indexed twice)
Sync confusion (two UUIDs for same file)
Update conflicts (which entry to modify?)
```

## Desired Behavior (Entry Reuse)

```
Database result (CORRECT):
entries:
  1: Documents (parent: null)
  2: Work      (parent: 1)    ← Shared by both locations
  3: Personal  (parent: 1)
  4: project.txt (parent: 2)
  5: notes.md    (parent: 2)

locations:
  Location A: entry_id = 1  (points to Documents)
  Location B: entry_id = 2  (points to EXISTING Work entry)

entry_closure:
  (1, 1, 0)  # Documents → Documents (self)
  (1, 2, 1)  # Documents → Work (child)
  (1, 3, 1)  # Documents → Personal (child)
  (1, 4, 2)  # Documents → project.txt (grandchild)
  (1, 5, 2)  # Documents → notes.md (grandchild)
  (2, 2, 0)  # Work → Work (self)
  (2, 4, 1)  # Work → project.txt (child)
  (2, 5, 1)  # Work → notes.md (child)

Benefits:
Single unified entry tree (no duplication)
Work entry has correct parent (Documents)
Both locations reference same physical entries
Changes to Work/ reflected in both locations
Storage efficient
Sync consistent (one UUID per file)
```

## Architecture

### Core Concept

**Locations are views over the entry tree, not owners of entries.**

- **Entry tree**: Single source of truth for filesystem hierarchy
- **Locations**: Pointers into the tree with indexing behavior attached
- **Nesting**: Multiple locations can reference nodes in the same tree
- **Ownership**: Entries belong to the tree, locations provide indexing semantics

### Location Semantics

Each location defines:
- **Root entry**: Which node in the tree this location starts from
- **Index mode**: How deeply to process files (Shallow/Content/Deep)
- **Watching**: Whether to monitor changes in real-time
- **Rules**: Which files to include/exclude

Multiple locations can reference overlapping subtrees with different behaviors.

## Required Changes

### 1. Location Creation - Reuse Existing Entries

**File**: `core/src/location/manager.rs:100-122`

**Current**:
```rust
// Always creates new entry
let entry_model = entry::ActiveModel {
    parent_id: Set(None),  // Orphan root
    ...
};
let entry_record = entry_model.insert(&txn).await?;
```

**Needed**:
```rust
// Check if entry already exists at this path
let existing_entry = directory_paths::Entity::find()
    .filter(directory_paths::Column::Path.eq(&path_str))
    .one(&txn)
    .await?;

let entry_id = match existing_entry {
    Some(dir_path) => {
        // REUSE existing entry for nested location
        info!(
            "Reusing existing entry {} for nested location at {}",
            dir_path.entry_id,
            path_str
        );
        dir_path.entry_id
    }
    None => {
        // Create new root entry (parent location doesn't exist)
        let entry_model = entry::ActiveModel {
            parent_id: Set(None),  // Will be orphan unless we detect parent location
            ...
        };
        let entry_record = entry_model.insert(&txn).await?;

        // Create closure and directory_path entries...

        entry_record.id
    }
};

// Location points to existing or new entry
let location_model = location::ActiveModel {
    entry_id: Set(Some(entry_id)),
    ...
};
```

### 2. Skip Indexer Job for Already-Indexed Paths

**File**: `core/src/location/manager.rs:~180`

**Current**:
```rust
// Always spawns indexer job
let job = IndexerJob::from_location(location_id, sd_path, mode);
library.jobs().dispatch(job).await?;
```

**Needed**:
```rust
// Check if this entry is already indexed
let entry = entry::Entity::find_by_id(entry_id)
    .one(db)
    .await?
    .ok_or(...)?;

if entry.indexed_at.is_some() {
    info!(
        "Location root already indexed at {}, skipping indexer job",
        entry.indexed_at.unwrap()
    );

    // But we might still want to apply THIS location's index_mode
    // if it's different from the parent location's mode
    if should_reindex_with_different_mode(entry_id, mode, db).await? {
        let job = IndexerJob::from_location(location_id, sd_path, mode);
        library.jobs().dispatch(job).await?;
    }
} else {
    // Not yet indexed, spawn job as normal
    let job = IndexerJob::from_location(location_id, sd_path, mode);
    library.jobs().dispatch(job).await?;
}
```

### 3. Watcher Precedence for Nested Locations

**File**: `core/src/service/watcher/mod.rs` (new logic)

**Problem**: If both Location A and Location B watch overlapping paths, which one handles events?

**Options**:

**Option A: All watchers trigger (simple but wasteful)**
```rust
// Both Location A and B get notified for /Documents/Work/test.txt
// Both call responder
// Responder is idempotent, so duplicate processing is safe but inefficient
```

**Option B: Innermost location wins (efficient)**
```rust
// In the watcher event dispatch or routing:
async fn find_deepest_watching_location(
    &self,
    event_path: &Path,
    library_id: Uuid,
    db: &DatabaseConnection,
) -> Result<Option<Uuid>> {
    // NOTE: All locations in watched_locations are already filtered to THIS device
    // (INDEX-003 Phase 1 ensures only owned locations are watched)

    let mut candidates = Vec::new();

    for (location_id, watched_loc) in self.watched_locations.read().await.iter() {
        // Get location's entry record to check tree relationship
        let location_record = location::Entity::find()
            .filter(location::Column::Uuid.eq(*location_id))
            .one(db)
            .await?;

        if let Some(loc) = location_record {
            if let Some(root_entry_id) = loc.entry_id {
                // Check if event path is under this location's entry tree
                // Use entry_closure and directory_paths, not path string matching
                if is_path_in_entry_tree(event_path, root_entry_id, db).await? {
                    // Get depth of location's root in the overall entry tree
                    let depth = get_entry_depth(root_entry_id, db).await?;
                    candidates.push((*location_id, depth));
                }
            }
        }
    }

    // Return location with deepest (highest depth value) root entry
    // Deeper in tree = more nested = should take precedence
    Ok(candidates
        .into_iter()
        .max_by_key(|(_, depth)| *depth)
        .map(|(id, _)| id))
}

async fn is_path_in_entry_tree(
    path: &Path,
    root_entry_id: i32,
    db: &DatabaseConnection,
) -> Result<bool> {
    // Try to resolve the path within this entry tree
    let path_str = path.to_string_lossy().to_string();

    let result = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT 1
            FROM directory_paths dp
            INNER JOIN entry_closure ec ON ec.descendant_id = dp.entry_id
            WHERE dp.path = ?
              AND ec.ancestor_id = ?
            LIMIT 1
            "#,
            vec![path_str.into(), root_entry_id.into()],
        ))
        .await?;

    Ok(result.is_some())
}
```

**Device filtering note**: Since INDEX-003 Phase 1 ensures only this device's locations are loaded into `watched_locations`, we don't need additional device_id filtering here. All locations in the HashMap are guaranteed to be owned by the current device.

**Recommendation**: Start with Option A (both trigger), optimize to Option B later.

### 4. Location Deletion - Preserve Shared Entries

**File**: `core/src/location/manager.rs` (delete method)

**Problem**: Deleting Location A shouldn't delete entries used by Location B

**Solution**:
```rust
async fn delete_location(&self, location_id: Uuid, db: &DatabaseConnection) -> Result<()> {
    let location = location::Entity::find()
        .filter(location::Column::Uuid.eq(location_id))
        .one(db)
        .await?
        .ok_or(...)?;

    // Check if other locations reference this entry or its descendants
    if let Some(entry_id) = location.entry_id {
        // Get all descendants of this location's root
        let descendant_ids = entry_closure::Entity::find()
            .filter(entry_closure::Column::AncestorId.eq(entry_id))
            .all(db)
            .await?
            .into_iter()
            .map(|ec| ec.descendant_id)
            .collect::<Vec<_>>();

        // Check if any other locations reference these entries
        let other_locations = location::Entity::find()
            .filter(location::Column::EntryId.is_in(descendant_ids.clone()))
            .filter(location::Column::Id.ne(location.id))
            .count(db)
            .await?;

        if other_locations > 0 {
            warn!(
                "Location shares entries with {} other location(s), preserving entry tree",
                other_locations
            );
            // Just delete the location record, keep entries
        } else {
            // Safe to delete entire entry tree
            delete_subtree(entry_id, db).await?;
        }
    }

    // Delete location record
    location::Entity::delete_by_id(location.id)
        .exec(db)
        .await?;

    Ok(())
}
```

### 5. Sync Behavior for Nested Locations

**Challenge**: How to sync nested locations across devices?

**Scenario**:
- Device A has Location A (`/Documents`) and Location B (`/Documents/Work`)
- Device C connects and syncs

**Current sync** (no nesting support):
- Location A syncs → creates entries 1-5
- Location B syncs → creates duplicate entries 100-102 

**With nesting support**:
- Location A syncs → creates entries 1-5 
- Location B syncs → just creates location record pointing to existing entry 2 
- No entry duplication 

**Implementation**: Location sync already uses `entry_id` reference, so this works automatically! Just need to ensure receiving device doesn't re-create entries.

### 6. Entry Ownership in Nested Scenarios

**Question**: Who "owns" entry 2 (Work)?

**Answer**: The **device** owns it (through Location A's device_id), not the location itself.

```
Device A owns Location A (/Documents)
  └─ Location A owns the indexing process for entry 1 and descendants
     └─ Including entry 2 (Work)

Device A creates Location B (/Documents/Work)
  └─ Location B is just a VIEW into entry 2's subtree
  └─ Still owned by Device A
  └─ No ownership conflict
```

**Implication**: Nested locations must be on the same device as their parent location's device.

**Validation needed**:
```rust
// When creating nested location, verify it's under a location on THIS device
if let Some(parent_location) = find_parent_location(&path, db).await? {
    if parent_location.device_id != current_device_id {
        return Err(LocationError::CannotNestAcrossDevices {
            path: path.to_string(),
            parent_location: parent_location.uuid,
            parent_device: parent_location.device_id,
        });
    }
}
```

## Implementation Plan

### Phase 1: Entry Reuse (2-3 days)

**Files**:
- `core/src/location/manager.rs`

**Tasks**:
1. Modify `add_location()` to check for existing entries at path
2. Reuse entry if found, create if not
3. Add validation to prevent cross-device nesting
4. Update `directory_paths` only if entry was created
5. Update `entry_closure` only if entry was created

### Phase 2: Skip Redundant Indexing (1 day)

**Files**:
- `core/src/location/manager.rs`

**Tasks**:
1. Check if entry is already indexed before spawning job
2. Consider index_mode differences (might need re-index)
3. Add logic to determine if re-indexing needed

### Phase 3: Watcher Precedence (2 days)

**Files**:
- `core/src/service/watcher/mod.rs`
- `core/src/service/watcher/worker.rs`

**Tasks**:
1. Implement `find_deepest_watching_location()` helper
2. Route events to innermost location only
3. Handle edge cases (multiple watchers at same depth)
4. Add metrics for event routing decisions

### Phase 4: Location Deletion Safety (1 day)

**Files**:
- `core/src/ops/locations/delete/action.rs` (or manager)

**Tasks**:
1. Check for other location references before deleting entries
2. Preserve shared entry trees
3. Only delete location record if entries are shared
4. Add tombstone for location (not entries) if nested

### Phase 5: Sync Validation (1 day)

**Files**:
- `core/src/infra/db/entities/location.rs`

**Tasks**:
1. Ensure location sync doesn't duplicate entries
2. Validate nested location references exist on receiving device
3. Handle case where parent location hasn't synced yet (defer)

### Phase 6: Testing (2 days)

**File**: `core/tests/nested_locations_test.rs` (new)

```rust
#[tokio::test]
async fn test_nested_location_reuses_entries() {
    let device = setup_test_device().await;

    // Create parent location
    let location_a = create_location(device, "/Documents").await;
    wait_for_index().await;

    // Verify Work entry exists
    let work_entry = find_entry_by_path("/Documents/Work").await.unwrap();
    assert_eq!(work_entry.parent_id, Some(documents_entry_id));

    // Create nested location at Work
    let location_b = create_location(device, "/Documents/Work").await;

    // Verify NO new entry created
    let work_entry_after = find_entry_by_path("/Documents/Work").await.unwrap();
    assert_eq!(work_entry.id, work_entry_after.id); // Same entry!

    // Verify Location B points to existing entry
    assert_eq!(location_b.entry_id, Some(work_entry.id));

    // Verify no duplicate entries
    let all_work_entries = entry::Entity::find()
        .filter(entry::Column::Name.eq("Work"))
        .all(db)
        .await?;
    assert_eq!(all_work_entries.len(), 1); // Only ONE Work entry
}

#[tokio::test]
async fn test_nested_location_watcher_precedence() {
    let device = setup_test_device().await;

    let location_a = create_location(device, "/Documents").await;
    let location_b = create_location(device, "/Documents/Work").await;

    // Create file in nested location
    create_file("/Documents/Work/test.txt").await;

    // Verify only Location B's worker processed it (innermost wins)
    let worker_metrics_a = get_worker_metrics(location_a.id).await;
    let worker_metrics_b = get_worker_metrics(location_b.id).await;

    assert_eq!(worker_metrics_a.events_processed.load(), 0);
    assert_eq!(worker_metrics_b.events_processed.load(), 1);
}

#[tokio::test]
async fn test_delete_parent_preserves_nested_location() {
    let device = setup_test_device().await;

    let location_a = create_location(device, "/Documents").await;
    let location_b = create_location(device, "/Documents/Work").await;
    wait_for_index().await;

    let work_entry_id = location_b.entry_id.unwrap();

    // Delete parent location
    delete_location(location_a.id).await.unwrap();

    // Verify Work entry still exists (referenced by Location B)
    let work_entry = entry::Entity::find_by_id(work_entry_id)
        .one(db)
        .await?;
    assert!(work_entry.is_some());

    // Verify Location B still works
    let location_b_after = location::Entity::find_by_id(location_b.id)
        .one(db)
        .await?;
    assert!(location_b_after.is_some());
}

#[tokio::test]
async fn test_nested_location_sync() {
    let (device_a, device_b) = setup_paired_devices().await;

    // Device A creates nested locations
    let location_a = create_location(device_a, "/Documents").await;
    wait_for_index().await;
    let location_b = create_location(device_a, "/Documents/Work").await;

    // Sync to Device B
    wait_for_sync().await;

    // Verify Device B has both locations
    let synced_location_a = find_location(device_b, location_a.uuid).await.unwrap();
    let synced_location_b = find_location(device_b, location_b.uuid).await.unwrap();

    // Verify they reference the same entry tree (no duplication)
    let work_entries = entry::Entity::find()
        .filter(entry::Column::Name.eq("Work"))
        .all(device_b.db())
        .await?;
    assert_eq!(work_entries.len(), 1); // Only ONE Work entry

    // Verify entry_id relationships preserved
    assert_eq!(synced_location_b.entry_id, Some(work_entries[0].id));
}

#[tokio::test]
async fn test_cannot_nest_across_devices() {
    let (device_a, device_b) = setup_paired_devices().await;

    // Device A creates location
    let location_a = create_location(device_a, "/Documents").await;
    wait_for_sync().await;

    // Device B tries to create nested location under Device A's location
    let result = create_location(device_b, "/Documents/Work").await;

    // Should fail - can't nest under another device's location
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LocationError::CannotNestAcrossDevices { .. }));
}
```

## Edge Cases & Solutions

### Edge Case 1: Parent Location Deleted, Nested Remains

**Scenario**:
- Location A (`/Documents`) deleted
- Location B (`/Documents/Work`) still exists
- Entry 2 (Work) now has orphan parent or needs reparenting

**Solution**:
```rust
// When deleting Location A:
// - Keep entry tree intact (Location B references it)
// - Entry 2's parent_id still points to entry 1
// - Entry 1 no longer has a location pointing to it
// - This is fine! Entry 1 exists as an unreferenced node
// - Or: Set entry 1's parent_id based on filesystem parent
```

**Alternative**: Prevent deleting parent locations if nested locations exist:
```rust
// Check for child locations before allowing deletion
let child_locations = find_locations_under_entry_subtree(entry_id, db).await?;
if !child_locations.is_empty() {
    return Err(LocationError::HasNestedLocations {
        location_id,
        nested: child_locations,
    });
}
```

### Edge Case 2: Moving Nested Location

**Scenario**:
```bash
# Move Work directory to Personal
mv /Documents/Work /Documents/Personal/Work
```

**Current behavior**:
- Location A's watcher detects rename
- Updates entry 2's parent from entry 1 to entry 3 (Personal)
- Location B's `entry_id` still points to entry 2 
- Location B's path is now wrong 

**Solution**: Update location path when root entry moves:
```rust
// After moving entry via responder:
// Check if any locations reference this entry
let locations_using_entry = location::Entity::find()
    .filter(location::Column::EntryId.eq(moved_entry_id))
    .all(db)
    .await?;

for location in locations_using_entry {
    // Rebuild location path from entry's new path
    let new_path = PathResolver::get_full_path(db, moved_entry_id).await?;

    location::ActiveModel {
        id: Set(location.id),
        // Update any path-related fields...
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    }.update(db).await?;
}
```

### Edge Case 3: Index Mode Conflicts

**Scenario**:
- Location A (`/Documents`) has `mode: Shallow`
- Location B (`/Documents/Work`) has `mode: Deep`
- Which mode applies to `/Documents/Work/test.pdf`?

**Solution**: Innermost location's mode wins:
```rust
// When indexing or processing:
fn get_effective_index_mode(path: &Path, db: &DatabaseConnection) -> IndexMode {
    let all_containing_locations = find_locations_containing_path(path, db).await?;

    // Find deepest location
    let deepest = all_containing_locations
        .into_iter()
        .max_by_key(|loc| count_path_components(&loc.path));

    deepest.map(|loc| loc.index_mode).unwrap_or(IndexMode::Shallow)
}
```

### Edge Case 4: Sync Order Dependencies

**Problem**: Location B references entry 2, but what if Location A hasn't synced yet?

**Current sync order** (from docs):
1. Shared resources (tags, etc.)
2. Devices
3. Locations
4. Volumes
5. Entries

**With nesting**:
- Location B syncs → `entry_id: 2`
- Entry 2 might not exist yet on receiving device!
- Foreign key constraint violation 

**Solution**: Defer nested location sync until parent location syncs:
```rust
// In location::Model::apply_state_change()
if let Some(entry_id) = location_data.entry_id {
    // Check if the entry exists
    if entry::Entity::find_by_id(entry_id).one(db).await?.is_none() {
        // Entry doesn't exist yet - parent location hasn't synced
        // Defer this location until later
        tracing::debug!(
            "Deferring nested location {} - entry {} not yet synced",
            location_uuid,
            entry_id
        );
        return Ok(()); // Skip for now, will retry on next sync
    }
}
```

Or better: Use the existing dependency system to ensure entries sync before locations that reference them.

## Database Schema Changes

**No schema changes needed!** The current schema already supports this:

```sql
CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    uuid TEXT UNIQUE,
    device_id INTEGER,
    entry_id INTEGER,  ← Can point to ANY entry (not just orphans)
    ...
);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER,  ← Can be null (root) or reference parent
    ...
);
```

The flexibility is already built in!

## Acceptance Criteria

- [ ] Can create nested location pointing to existing entry
- [ ] No entry duplication when nesting locations
- [ ] Entry tree maintains correct parent/child relationships
- [ ] Nested location inherits entry tree from parent location
- [ ] Innermost watcher handles events (or both handle idempotently)
- [ ] Deleting parent location preserves entries used by nested location
- [ ] Moving nested location's root entry updates location reference
- [ ] Nested locations sync correctly (defer if entry not yet synced)
- [ ] Cannot create nested location across devices
- [ ] Index mode of innermost location applies
- [ ] All tests pass
- [ ] Documentation updated

## Migration Strategy

**Breaking change**: No

**Backwards compatibility**: Yes - existing non-nested locations continue to work

**Rollout**:
1. Implement entry reuse in location creation (Phase 1)
2. Test with simple 1-level nesting
3. Add watcher precedence (Phase 3)
4. Add deletion safety (Phase 4)
5. Test multi-level nesting (3+ levels)
6. Document and release

## Performance Considerations

**Benefits**:
- Reduced storage (no duplicate entries)
- Faster indexing (skip already-indexed paths)
- Less sync traffic (entries synced once)

**Costs**:
- Checking for existing entries on location creation (+1 query)
- Watcher precedence logic (path comparison overhead)
- Location deletion checks (query for other location references)

**Net impact**: Positive for users with many nested locations, neutral for simple use cases.

## UI/UX Implications

**Location list view**:
```
Documents (/Users/jamespine/Documents)
  └─ Work (/Users/jamespine/Documents/Work) [nested]

Photos (/Users/jamespine/Pictures)
```

**Considerations**:
- Show nesting visually in UI
- Warn before deleting parent location
- Indicate which location is actively watching a path
- Show index mode inheritance chain

## References

- [Location Watcher Service](../../core/src/service/watcher/mod.rs)
- [Location Manager](../../core/src/location/manager.rs)
- [Entry-Centric Model](./CORE-001-entry-centric-model.md)
- [INDEX-003](./INDEX-003-watcher-device-ownership-violation.md) - Related device ownership work

## Implementation Files

**Modified files**:
- `core/src/location/manager.rs`
- `core/src/service/watcher/mod.rs`
- `core/src/service/watcher/worker.rs`
- `core/src/ops/locations/delete/action.rs`

**New files**:
- `core/tests/nested_locations_test.rs`
- `core/src/location/nesting.rs` (helper functions)

## Future Enhancements

- **Virtual locations**: Locations that don't correspond to filesystem paths (e.g., "All PDFs")
- **Dynamic nesting**: Auto-detect and suggest nested locations
- **Cross-device virtual views**: Read-only "nested" views of remote device locations
- **Location templates**: Predefined nesting structures for common use cases
