---
id: LSYNC-023
title: Rebuild Closure Tables After Sync
status: Done
assignee: jamiepine
priority: High
parent: LSYNC-000
tags: [sync, database, bug, closure-table]
last_updated: 2025-10-23
related_tasks: [LSYNC-010, INDEX-003, CORE-004]
---

# Rebuild Closure Tables After Sync

## Problem Statement

**CRITICAL BUG**: Closure tables (`entry_closure` and `tag_closure`) are NOT rebuilt when entries/tags are synced from other devices, leaving them with only self-references.

### Current Broken Behavior

```sql
-- Device A (source) has full closure table:
SELECT * FROM entry_closure WHERE ancestor_id = 1;
(1, 1, 0)  -- Desktop → Desktop (self)
(1, 2, 1)  -- Desktop → Desk (child)
(1, 3, 1)  -- Desktop → .localized (child)
(1, 4, 2)  -- Desktop → file.txt (grandchild)
... 79 total relationships

-- Device B (after sync) has BROKEN closure table:
SELECT * FROM entry_closure;
(1, 1, 0)  -- Desktop → Desktop (self only!)
(2, 2, 0)  -- Desk → Desk (self only!)
(3, 3, 0)  -- .localized → .localized (self only!)
... NO parent-child relationships!
```

### Impact

**Severity**: CRITICAL - Breaks core functionality

**Consequences**:

- Cannot query descendants (`WHERE ancestor_id = X`) returns nothing
- Cannot delete subtrees (delete only deletes single entry)
- INDEX-003 fix doesn't work (relies on `entry_closure` JOIN)
- Location scoping broken (can't find entries in location's tree)
- Change detection broken (can't find existing entries in subtree)
- Path resolution ambiguity can't be fixed without closure table

**Evidence**:

```
Jam instance (synced):
- Entries: 1,987
- entry_closure records: 27 (all self-references)
- Missing: ~1,960 parent-child closure relationships
```

## Root Cause

`entry::Model::apply_state_change()` (line 329-499) inserts/updates entry records but **does NOT rebuild `entry_closure`**.

When locally indexing, `EntryProcessor::create_entry_in_conn()` populates closure:

```rust
// line 289-309 in entry.rs
let self_closure = entry_closure::ActiveModel {
    ancestor_id: Set(result.id),
    descendant_id: Set(result.id),
    depth: Set(0),
};
out_self_closures.push(self_closure);

// Copy parent's ancestors
if let Some(parent_id) = parent_id {
    conn.execute_unprepared(&format!(
        "INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
         SELECT ancestor_id, {}, depth + 1
         FROM entry_closure
         WHERE descendant_id = {}",
        result.id, parent_id
    ))
    ...
}
```

But `apply_state_change()` doesn't do this!

## Solution

### Option 1: Rebuild Per-Entry During Sync (Real-time)

**File**: `core/src/infra/db/entities/entry.rs:~497`

Add closure table population to `apply_state_change()`:

```rust
pub async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    // ... existing upsert logic ...

    let entry_id = if let Some(existing_entry) = existing {
        // Update ...
        existing_entry.id
    } else {
        // Insert ...
        inserted.id
    };

    // Rebuild entry_closure for this entry
    rebuild_entry_closure(entry_id, parent_id, db).await?;

    // If directory, update directory_paths ...

    Ok(())
}

async fn rebuild_entry_closure(
    entry_id: i32,
    parent_id: Option<i32>,
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, Set};

    // Delete existing closure records for this entry
    entry_closure::Entity::delete_many()
        .filter(entry_closure::Column::DescendantId.eq(entry_id))
        .exec(db)
        .await?;

    // Insert self-reference
    let self_closure = entry_closure::ActiveModel {
        ancestor_id: Set(entry_id),
        descendant_id: Set(entry_id),
        depth: Set(0),
    };
    self_closure.insert(db).await?;

    // If there's a parent, copy all parent's ancestors
    if let Some(parent_id) = parent_id {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
            SELECT ancestor_id, ?, depth + 1
            FROM entry_closure
            WHERE descendant_id = ?
            "#,
            vec![entry_id.into(), parent_id.into()],
        ))
        .await?;
    }

    Ok(())
}
```

**Pros**:

- Closure table always correct in real-time
- No batch rebuild needed
- Works for both backfill and real-time sync

**Cons**:

- Adds overhead to every entry sync
- Parent must exist before child (dependency ordering)

### Option 2: Bulk Rebuild After Backfill (Batch)

**File**: `core/src/service/sync/backfill.rs:~140`

Add closure rebuild after backfill completes:

```rust
// After Phase 3: backfill_device_owned_state
info!("Rebuilding closure tables after backfill...");
rebuild_all_entry_closures(db).await?;
rebuild_all_tag_closures(db).await?;

// Phase 4: Transition to ready
```

**Implementation**:

```rust
async fn rebuild_all_entry_closures(db: &DatabaseConnection) -> Result<()> {
    use sea_orm::ConnectionTrait;

    // Clear existing closure table
    entry_closure::Entity::delete_many().exec(db).await?;

    // 1. Insert all self-references
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
        SELECT id, id, 0 FROM entries
        "#,
        vec![],
    ))
    .await?;

    // 2. Recursively build parent-child relationships
    // Keep inserting until no new relationships found
    let mut iteration = 0;
    loop {
        let result = db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            INSERT OR IGNORE INTO entry_closure (ancestor_id, descendant_id, depth)
            SELECT ec.ancestor_id, e.id, ec.depth + 1
            FROM entries e
            INNER JOIN entry_closure ec ON ec.descendant_id = e.parent_id
            WHERE e.parent_id IS NOT NULL
              AND NOT EXISTS (
                SELECT 1 FROM entry_closure
                WHERE ancestor_id = ec.ancestor_id
                  AND descendant_id = e.id
              )
            "#,
            vec![],
        ))
        .await?;

        iteration += 1;
        let rows_affected = result.rows_affected();

        tracing::debug!(
            iteration = iteration,
            rows_inserted = rows_affected,
            "entry_closure rebuild iteration"
        );

        if rows_affected == 0 {
            break; // No more relationships to add
        }

        if iteration > 100 {
            return Err(anyhow::anyhow!("entry_closure rebuild exceeded max iterations - possible cycle"));
        }
    }

    info!("Rebuilt entry_closure table in {} iterations", iteration);
    Ok(())
}
```

**Pros**:

- Simple - one batch operation
- No per-entry overhead during sync
- Handles out-of-order syncing (parent after child)

**Cons**:

- Closure table incomplete during backfill
- Queries fail until rebuild completes

### Option 3: Hybrid (Recommended)

Combine both approaches:

1. **Real-time rebuild** for small syncs (< 100 entries)
2. **Batch rebuild** after large backfill operations

## Recommendation

**Implement Option 1 (real-time rebuild)** because:

- Ensures closure table is always correct
- Works for both initial backfill and incremental sync
- Required for INDEX-003 Phase 2 to work on synced entries
- Matches the pattern used during local indexing

Then add **Option 2 as a safety measure** to run after backfill in case of any missed entries.

## Implementation Plan

### Phase 1: Add Real-time Closure Rebuild (2-3 hours)

**File**: `core/src/infra/db/entities/entry.rs`

1. Create `rebuild_entry_closure()` function
2. Call it in `apply_state_change()` after upsert
3. Handle both insert and update cases
4. Add error handling and logging

### Phase 2: Add Bulk Rebuild After Backfill (1-2 hours)

**File**: `core/src/service/sync/backfill.rs`

1. Create `rebuild_all_entry_closures()` function
2. Call after `backfill_device_owned_state()` completes
3. Add progress logging
4. Handle large datasets efficiently

### Phase 3: Add Tag Closure Rebuild (1 hour)

**File**: `core/src/infra/db/entities/tag.rs` (or sync code)

1. Check if tags have same issue
2. Implement rebuild if needed
3. Call after shared resource backfill

### Phase 4: Testing (1 hour)

**File**: `core/tests/sync_closure_rebuild_test.rs` (new)

```rust
#[tokio::test]
async fn test_entry_closure_rebuilt_during_sync() {
    let (device_a, device_b) = setup_paired_devices().await;

    // Device A creates location with nested entries
    create_location(device_a, "/Test").await;
    create_file(device_a, "/Test/folder/subfolder/file.txt").await;

    // Verify Device A has full closure table
    let closure_a = count_closure_records(device_a).await;
    assert!(closure_a > 10); // Self-refs + parent-child rels

    // Sync to Device B
    wait_for_sync().await;

    // Verify Device B has FULL closure table (not just self-refs)
    let closure_b = count_closure_records(device_b).await;
    assert_eq!(closure_a, closure_b); // Should match!

    // Verify can query descendants on Device B
    let descendants = query_descendants(device_b, root_entry_id).await;
    assert!(descendants.len() > 1); // Should find children!
}
```

## Acceptance Criteria

- [ ] `entry_closure` rebuilt in `apply_state_change()`
- [ ] Bulk rebuild runs after backfill completes
- [ ] Tag closure also rebuilt if needed
- [ ] Synced entries have full closure records (not just self-refs)
- [ ] Can query descendants of synced entries
- [ ] INDEX-003 location scoping works on synced entries
- [ ] Delete subtree works on synced entries
- [ ] Test suite passes

## References

- [Entry Entity](../../core/src/infra/db/entities/entry.rs)
- [Backfill Manager](../../core/src/service/sync/backfill.rs)
- [CORE-004](./CORE-004-closure-table.md) - Closure table architecture
- [INDEX-003](./INDEX-003-watcher-device-ownership-violation.md) - Depends on closure table
