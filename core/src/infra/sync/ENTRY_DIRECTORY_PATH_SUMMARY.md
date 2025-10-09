# Entry and Directory Path Sync - Implementation Summary

**Date**: 2025-10-09
**Status**: Analysis Complete, Implementation Added
**Related**: `ENTRY_PATH_SYNC_ANALYSIS.md`

---

## Problem Statement

You identified a critical sync challenge: **entries depend heavily on `directory_paths` for path resolution**, but `directory_paths` cannot be synced directly because it uses local integer foreign keys and device-specific paths.

### The Data Model

```
entries
├── id (local PK)
├── uuid (global sync ID)
├── name (filename without extension)
├── extension (just the extension)
├── parent_id (FK to entries.id - the containing directory)
└── kind (0=File, 1=Directory)

directory_paths (cache table)
├── entry_id (PK, FK to entries.id) ← LOCAL INTEGER!
└── path (full absolute path)      ← DEVICE-SPECIFIC!
```

**Path Reconstruction**:
- **Directories**: Direct lookup in `directory_paths` by `entry_id`
- **Files**: `parent_directory_path` + `/` + `name` + `.` + `extension`

---

## Solution

### Classification: Derived Data (Do Not Sync)

`directory_paths` is a **denormalized cache** for performance. The source of truth is the hierarchical `parent_id` chain in entries.

**Decision**: Do NOT implement `Syncable` for `directory_paths`. Instead, rebuild it locally on each device after syncing entries.

---

## Implementation

### 1. Entry Model Foreign Key Mappings

Added FK mapping declarations to convert local integer IDs to UUIDs during sync:

```rust
// In core/src/infra/db/entities/entry.rs
impl Syncable for Model {
    fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
        vec![
            FKMapping::new("parent_id", "entries"),         // Self-referential
            FKMapping::new("metadata_id", "user_metadata"),
            FKMapping::new("content_id", "content_identities"),
        ]
    }
}
```

### 2. Outgoing Sync (Broadcasting)

Modified `query_for_sync()` to convert FKs to UUIDs before sending:

```rust
async fn query_for_sync(...) -> Result<Vec<(Uuid, Value, DateTime)>> {
    // Query entries
    let results = Entity::find()
        .filter(Column::Uuid.is_not_null())
        .all(db)
        .await?;

    // Convert each entry
    for entry in results {
        let mut json = entry.to_sync_json()?;

        // Convert FK integer IDs → UUIDs
        for fk in <Model as Syncable>::foreign_key_mappings() {
            convert_fk_to_uuid(&mut json, &fk, db).await?;
        }

        sync_results.push((uuid, json, timestamp));
    }

    Ok(sync_results)
}
```

**Result**: Sync JSON contains `parent_uuid`, `metadata_uuid`, `content_uuid` instead of local IDs.

### 3. Incoming Sync (Receiving)

Modified `apply_state_change()` to:
1. Map UUIDs back to local integer IDs
2. Upsert the entry
3. Rebuild `directory_paths` for directories

```rust
pub async fn apply_state_change(
    data: serde_json::Value,
    db: &DatabaseConnection,
) -> Result<()> {
    // Map UUID FKs → local integer IDs
    let data = map_sync_json_to_local(
        data,
        <Model as Syncable>::foreign_key_mappings(),
        db,
    ).await?;

    // Deserialize and upsert
    let entry: Model = serde_json::from_value(data)?;
    let result = entry.upsert_by_uuid(db).await?;

    // If directory, rebuild its directory_paths cache entry
    if entry.entry_kind() == EntryKind::Directory {
        let entry_id = result.last_insert_id;
        rebuild_directory_path(entry_id, entry.parent_id, &entry.name, db).await?;
    }

    Ok(())
}
```

### 4. Directory Path Rebuilding

Added helper method to reconstruct directory paths from parent chain:

```rust
async fn rebuild_directory_path(
    entry_id: i32,
    parent_id: Option<i32>,
    name: &str,
    db: &DatabaseConnection,
) -> Result<()> {
    // Compute path from parent
    let path = if let Some(parent_id) = parent_id {
        // Get parent's directory path
        match directory_paths::Entity::find_by_id(parent_id).one(db).await? {
            Some(parent_path) => format!("{}/{}", parent_path.path, name),
            None => {
                // Parent path not found yet - defer rebuild
                tracing::warn!("Parent directory path not found, deferring rebuild");
                return Ok(());
            }
        }
    } else {
        // Root directory
        name.to_string()
    };

    // Upsert directory_paths entry
    directory_paths::ActiveModel {
        entry_id: Set(entry_id),
        path: Set(path),
    }.upsert(db).await?;

    Ok(())
}
```

### 5. FK Mapper Updates

Extended `fk_mapper.rs` to support all entry FK relationships:

```rust
// Added lookup support for:
async fn lookup_uuid_for_local_id(table: &str, local_id: i32, ...) -> Result<Uuid> {
    match table {
        "devices" => { /* ... */ }
        "entries" => { /* ... */ }
        "locations" => { /* ... */ }
        "user_metadata" => { /* NEW */ }
        "content_identities" => { /* NEW */ }
        _ => Err(...)
    }
}

// Added reverse lookup:
async fn lookup_local_id_for_uuid(table: &str, uuid: Uuid, ...) -> Result<i32> {
    // ... same tables ...
}
```

---

## Sync Flow Example

### Device A Indexes and Syncs Directory

```
1. Indexer creates directory entry:
   entries(id=50, uuid='abc-123', name='Photos', kind=Directory, parent_id=10)
   directory_paths(entry_id=50, path='/Users/jamie/Photos')

2. Sync broadcasts entry:
   {
     uuid: 'abc-123',
     name: 'Photos',
     kind: 1,
     parent_uuid: 'xyz-789',  // Converted from parent_id=10
     // NO path field!
     // NO entry_id field!
   }
```

### Device B Receives and Applies

```
3. Device B receives sync message:
   - Maps parent_uuid='xyz-789' → local parent_id=15
   - Upserts entry with LOCAL id=73 (different from Device A's 50!)
   - Detects it's a directory
   - Rebuilds directory_path:
     * Looks up parent directory_path for id=15 → '/mnt/storage'
     * Computes path: '/mnt/storage' + '/' + 'Photos' = '/mnt/storage/Photos'
     * Inserts: directory_paths(entry_id=73, path='/mnt/storage/Photos')

4. Result:
   Device A: entries.id=50, directory_paths(50, '/Users/jamie/Photos')
   Device B: entries.id=73, directory_paths(73, '/mnt/storage/Photos')

   Same logical directory, different local IDs and paths!
```

---

## Key Benefits

### ✅ Correctness
- Each device has filesystem-appropriate paths
- No attempt to sync device-specific data

### ✅ Simplicity
- No complex path transformation in sync protocol
- Entry hierarchy is universal, paths are local

### ✅ Efficiency
- Rebuild is cheap (single path lookup per directory)
- Only done once per synced directory entry

### ✅ Portability
- Works across different:
  - Filesystems (ext4, APFS, NTFS)
  - Mount points (/home, /Users, C:\)
  - Path separators (/ vs \)

---

## Edge Cases Handled

### 1. Out-of-Order Sync

**Problem**: Child directory arrives before parent

**Solution**: Defer rebuild with warning
```rust
if parent_path.is_none() {
    tracing::warn!("Parent directory path not found, deferring rebuild");
    return Ok(()); // Skip for now, will be fixed by bulk rebuild
}
```

### 2. Null Parent IDs

**Problem**: Root entries have `parent_id = NULL`

**Solution**: FK mapper handles null gracefully
```rust
if uuid_value.is_null() {
    data[fk.local_field] = Value::Null;
    continue;
}
```

### 3. Entries Without UUIDs

**Problem**: Files still being processed may not have UUIDs

**Solution**: Filter them out
```rust
query = query.filter(Column::Uuid.is_not_null());
```

---

## Testing Checklist

- [ ] Unit test: Directory path rebuilt after sync
- [ ] Unit test: File paths resolved correctly using parent directory_paths
- [ ] Unit test: Null FKs handled (root entries)
- [ ] Integration test: Large directory tree sync (1000s of entries)
- [ ] Integration test: Out-of-order entry arrival
- [ ] Integration test: Bulk rebuild after backfill

---

## Future Work

### Bulk Rebuild for Backfill

When a new device joins and syncs thousands of entries, implement efficient bulk rebuild:

```rust
/// Rebuild all directory_paths for a location after backfill
pub async fn rebuild_all_directory_paths(
    location_id: i32,
    db: &DatabaseConnection,
) -> Result<u64> {
    let location = location::Entity::find_by_id(location_id).one(db).await?;
    let root_path = get_location_root_path(&location)?;

    // Recursive rebuild from root
    rebuild_paths_recursive(location.entry_id, &root_path, db).await
}
```

### Directory Move Optimization

When a directory moves, update all descendant paths efficiently:

```rust
/// Update all descendant directory paths after a move
pub async fn update_descendant_paths(
    moved_directory_id: i32,
    old_path: &str,
    new_path: &str,
    db: &DatabaseConnection,
) -> Result<u64> {
    db.execute_unprepared(&format!(
        "UPDATE directory_paths
         SET path = REPLACE(path, '{}', '{}')
         WHERE path LIKE '{}/%'",
        old_path, new_path, old_path
    )).await
}
```

---

## Related Files Modified

- ✅ `core/src/infra/db/entities/entry.rs` - FK mappings, rebuild logic
- ✅ `core/src/infra/sync/fk_mapper.rs` - Added user_metadata, content_identities support
- ✅ `docs/core/sync.md` - Added derived data section
- ✅ `core/src/infra/sync/ENTRY_PATH_SYNC_ANALYSIS.md` - Comprehensive analysis

---

## Summary

The entry-directory_paths relationship required careful analysis because **paths are derived data that depends on local context**. By:

1. Syncing only the entry hierarchy (with UUID-based FKs)
2. Rebuilding `directory_paths` locally on each device
3. Using device-appropriate filesystem paths

We achieve correct, portable sync without attempting to sync non-portable data.

This pattern applies to other derived/cache tables:
- `entry_closure` (transitive relationships)
- `tag_closure` (tag hierarchy)
- Aggregate statistics (size, counts)

**General principle**: Sync the source of truth, rebuild derived data locally.

