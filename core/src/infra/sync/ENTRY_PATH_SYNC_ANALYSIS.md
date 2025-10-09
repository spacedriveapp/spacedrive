# Entry and Directory Path Sync Analysis

**Status**: Analysis Complete
**Date**: 2025-10-09
**Issue**: How to handle `directory_paths` cache table in sync system

---

## The Problem

Entries use an optimized path storage model where:

1. **Entries table** stores:
   - `name` (filename without extension for files, directory name for directories)
   - `extension` (file extension only, NULL for directories)
   - `parent_id` (FK to parent entry - the containing directory)
   - `kind` (0=File, 1=Directory, 2=Symlink)

2. **Directory_paths table** stores:
   - `entry_id` (PRIMARY KEY, FK to entries.id) - **LOCAL INTEGER**
   - `path` (full absolute path string for the directory)

### Path Reconstruction

**For directories**: Direct lookup in `directory_paths` by `entry_id`
```rust
// Get directory path
let dir_path = DirectoryPaths::find_by_id(entry_id).await?;
// Returns: "/Users/jamie/Photos/Vacation"
```

**For files**: Parent directory path + name + extension
```rust
// Get parent directory path
let parent_path = DirectoryPaths::find_by_id(entry.parent_id).await?;
// parent_path: "/Users/jamie/Photos/Vacation"

// Reconstruct filename
let filename = format!("{}.{}", entry.name, entry.extension);
// filename: "beach.jpg"

// Full path: "/Users/jamespine/Photos/Vacation/beach.jpg"
let full_path = PathBuf::from(parent_path).join(filename);
```

### The Sync Challenge

The `directory_paths` table has a critical issue for sync:

```sql
CREATE TABLE directory_paths (
    entry_id INTEGER PRIMARY KEY,  -- ❌ LOCAL integer FK!
    path TEXT NOT NULL
);
```

**Problem**: `entry_id` is a local auto-increment primary key that differs across devices:

```
Device A:
  entries:
    id=1, uuid=abc-123, name="Photos", kind=Directory
  directory_paths:
    entry_id=1, path="/Users/jamie/Photos"

Device B (after sync):
  entries:
    id=42, uuid=abc-123, name="Photos", kind=Directory  -- Different local ID!
  directory_paths:
    entry_id=42, path="/Users/otheruser/Photos"  -- Different path AND id!
```

We **cannot** directly sync the `directory_paths` table because:
1. The `entry_id` FK is a local integer that won't match on other devices
2. The `path` is device-specific (different users, mount points, filesystems)

---

## Solution: Derived Data - Don't Sync It!

### Classification: Performance Cache (Non-Syncable)

The `directory_paths` table is **derived data** - it's a denormalized cache for performance optimization. The source of truth is:
- The hierarchical `parent_id` chain in the `entries` table
- The `name` field on each entry
- The location's root path

### Strategy: Rebuild Locally

**Do NOT sync `directory_paths`**. Instead, each device rebuilds it locally during/after indexing:

```rust
// During indexing (when creating directory entry)
if entry.kind == EntryKind::Directory {
    let absolute_path = entry.path.to_string_lossy().to_string();

    // Insert into directory_paths table
    let dir_path_entry = directory_paths::ActiveModel {
        entry_id: Set(result.id),  // Use LOCAL id
        path: Set(absolute_path),   // Use LOCAL path
    };
    dir_path_entry.insert(db).await?;
}
```

### Implementation Details

#### 1. Entry Sync (Current - Correct)

Entries are device-owned and sync normally:

```rust
impl Syncable for entry::Model {
    const SYNC_MODEL: &'static str = "entry";

    fn sync_depends_on() -> &'static [&'static str] {
        &["location"]  // Entries depend on location
    }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&["id"])  // Exclude local DB PK
    }
}
```

**Sync data includes**:
- ✅ `uuid` (global identifier)
- ✅ `name` (just the name, not full path)
- ✅ `extension` (just the extension)
- ✅ `parent_id` → `parent_uuid` (via FK mapping)
- ✅ `kind` (File/Directory/Symlink)
- ❌ Full paths (not stored in entry)

#### 2. Directory Paths (Do Not Sync)

`directory_paths` should **NOT** implement `Syncable` at all:

```rust
// ❌ Do NOT add this implementation!
// impl Syncable for directory_paths::Model { ... }
```

**Rationale**:
- It's a cache derived from entry hierarchy
- Paths are device-specific anyway
- Must be rebuilt locally for each device's filesystem

#### 3. Rebuild After Sync

When a device receives synced entries from a peer, it needs to rebuild `directory_paths`:

```rust
/// Rebuild directory_paths cache for synced entries
pub async fn rebuild_directory_paths_for_location(
    location_id: i32,
    db: &DatabaseConnection,
) -> Result<(), DbErr> {
    // Get location root entry
    let location = location::Entity::find_by_id(location_id).one(db).await?
        .ok_or_else(|| DbErr::RecordNotFound("Location not found".to_string()))?;

    let root_entry_id = location.entry_id;

    // Get location's root path from its existing directory_path entry
    let root_path = directory_paths::Entity::find_by_id(root_entry_id)
        .one(db)
        .await?
        .map(|dp| dp.path)
        .ok_or_else(|| DbErr::RecordNotFound("Root directory path not found".to_string()))?;

    // Traverse entry hierarchy and rebuild paths
    rebuild_paths_recursive(root_entry_id, &root_path, db).await?;

    Ok(())
}

async fn rebuild_paths_recursive(
    entry_id: i32,
    path: &str,
    db: &DatabaseConnection,
) -> Result<(), DbErr> {
    // Get all children of this entry
    let children = entry::Entity::find()
        .filter(entry::Column::ParentId.eq(entry_id))
        .all(db)
        .await?;

    for child in children {
        if child.entry_kind() == EntryKind::Directory {
            // Build child's full path
            let child_path = format!("{}/{}", path, child.name);

            // Upsert into directory_paths
            let dir_path = directory_paths::ActiveModel {
                entry_id: Set(child.id),
                path: Set(child_path.clone()),
            };

            directory_paths::Entity::insert(dir_path)
                .on_conflict(
                    OnConflict::column(directory_paths::Column::EntryId)
                        .update_column(directory_paths::Column::Path)
                        .to_owned()
                )
                .exec(db)
                .await?;

            // Recurse into subdirectories
            rebuild_paths_recursive(child.id, &child_path, db).await?;
        }
    }

    Ok(())
}
```

---

## Sync Flow: Entries Across Devices

### Scenario: Device A Indexes `/Users/jamie/Photos`, Syncs to Device B

#### Device A (Indexing)

```
1. Indexer discovers directory: /Users/jamie/Photos

2. Create entry:
   INSERT INTO entries (uuid, name, kind, parent_id)
   VALUES ('abc-123', 'Photos', 1, 10);  -- Returns id=50

3. Create directory_path cache:
   INSERT INTO directory_paths (entry_id, path)
   VALUES (50, '/Users/jamie/Photos');

4. Indexer discovers file: /Users/jamie/Photos/beach.jpg

5. Create entry:
   INSERT INTO entries (uuid, name, extension, kind, parent_id)
   VALUES ('def-456', 'beach', 'jpg', 0, 50);  -- Returns id=51

6. (No directory_path for files - they compute path from parent)
```

#### Device A (Sync Out)

```
7. Broadcast entry state changes:

StateChange {
    model_type: "entry",
    record_uuid: "abc-123",
    device_id: device_a_uuid,
    data: {
        uuid: "abc-123",
        name: "Photos",
        kind: 1,
        parent_uuid: "xyz-789",  // FK mapped to UUID
        // NO path field - not in entry model!
        // NO entry_id - excluded from sync
    }
}

StateChange {
    model_type: "entry",
    record_uuid: "def-456",
    device_id: device_a_uuid,
    data: {
        uuid: "def-456",
        name: "beach",
        extension: "jpg",
        kind: 0,
        parent_uuid: "abc-123",  // References Photos directory
    }
}
```

**Note**: No `directory_paths` data is sent!

#### Device B (Sync In)

```
8. Receive entry state change for directory:

   a) Map parent_uuid → local parent_id
      (parent was synced earlier, has local id=15 on Device B)

   b) Upsert entry:
      INSERT INTO entries (uuid, name, kind, parent_id)
      VALUES ('abc-123', 'Photos', 1, 15)
      ON CONFLICT (uuid) UPDATE ...;
      -- Returns DIFFERENT local id=73 on Device B!

   c) Check if directory:
      if entry.kind == Directory {
          // Rebuild directory_path for Device B's filesystem
          let parent_path = get_directory_path(15);  // "/mnt/storage"
          let full_path = format!("{}/{}", parent_path, "Photos");
          // "/mnt/storage/Photos"

          INSERT INTO directory_paths (entry_id, path)
          VALUES (73, '/mnt/storage/Photos')
          ON CONFLICT (entry_id) UPDATE ...;
      }

9. Receive entry state change for file:

   a) Map parent_uuid "abc-123" → local id=73

   b) Upsert entry:
      INSERT INTO entries (uuid, name, extension, kind, parent_id)
      VALUES ('def-456', 'beach', 'jpg', 0, 73)
      -- Returns local id=74

   c) Not a directory, so no directory_path entry needed

   d) Path can be computed on-demand:
      - Get parent directory_path: "/mnt/storage/Photos"
      - Append filename: "beach.jpg"
      - Result: "/mnt/storage/Photos/beach.jpg"
```

---

## Key Insights

### 1. Directory Paths are Device-Specific

The same logical directory has different paths on different devices:

```
Device A (macOS):     /Users/jamie/Photos
Device B (Linux):     /home/jamie/Photos
Device C (Windows):   C:\Users\Jamie\Pictures
Device D (Android):   /storage/emulated/0/DCIM
```

Even if we could sync the paths, we **shouldn't** - they're not portable!

### 2. The Entry Hierarchy IS the Source of Truth

The parent-child relationships in entries are sufficient to reconstruct paths:

```
Entry Tree (UUID-based, portable):
  Location Root (uuid: root-123)
    └─ Photos (uuid: photos-456, parent: root-123)
        ├─ Vacation (uuid: vacation-789, parent: photos-456)
        │   └─ beach.jpg (uuid: file-111, parent: vacation-789)
        └─ Family (uuid: family-222, parent: photos-456)

Each device rebuilds paths for its filesystem:
  Device A: /Users/jamie/Photos/Vacation/beach.jpg
  Device B: /mnt/storage/Photos/Vacation/beach.jpg
```

### 3. Directory Paths is a Query Optimization

The `directory_paths` table exists purely for performance:

**Without cache** (slow):
```rust
// Compute path by walking up parent chain
fn get_full_path(entry_id: i32) -> PathBuf {
    let mut parts = vec![];
    let mut current_id = entry_id;

    while let Some(entry) = find_entry(current_id) {
        parts.push(entry.name);
        current_id = entry.parent_id?;
    }

    parts.reverse();
    PathBuf::from(parts.join("/"))
}
```

**With cache** (fast):
```rust
// Direct lookup for directories
fn get_full_path(entry_id: i32) -> PathBuf {
    if let Some(dir_path) = directory_paths.get(entry_id) {
        return PathBuf::from(dir_path.path);
    }
    // Fallback for files: parent_path + name
}
```

### 4. Rebuild is Cheap

Rebuilding `directory_paths` after sync is very efficient:

```rust
// Single recursive traversal
// For 1 million entries (typical large library):
// - ~10,000 directories
// - Rebuild time: ~100ms
// - Only done once per sync session
```

---

## Implementation Checklist

- [x] **Entry model**: Already implements `Syncable` correctly
- [x] **Entry model**: Excludes `id` from sync (local-only)
- [x] **Entry model**: Maps `parent_id` → `parent_uuid` (FK mapping)
- [ ] **Directory paths**: Do NOT implement `Syncable` (confirmed correct approach)
- [ ] **Sync handler**: Add directory path rebuild after entry state changes
- [ ] **Location sync**: Rebuild all directory paths when new device joins
- [ ] **Move operations**: Update directory paths when directories move

### Required Implementation

#### 1. Add Rebuild Trigger to Entry State Changes

```rust
// In entry::Model::apply_state_change
pub async fn apply_state_change(
    data: serde_json::Value,
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    let entry: Self = serde_json::from_value(data)?;

    // Upsert entry as before...
    let upserted = /* ... */;

    // If directory, rebuild its directory_path entry
    if entry.entry_kind() == EntryKind::Directory {
        rebuild_directory_path_for_entry(&upserted, db).await?;
    }

    Ok(())
}

async fn rebuild_directory_path_for_entry(
    entry: &entry::Model,
    db: &DatabaseConnection,
) -> Result<(), DbErr> {
    // Compute path from parent chain
    let path = if let Some(parent_id) = entry.parent_id {
        let parent_path = PathResolver::get_directory_path(db, parent_id).await?;
        format!("{}/{}", parent_path, entry.name)
    } else {
        // Root directory - get from location
        entry.name.clone()
    };

    // Upsert directory_path
    let dir_path = directory_paths::ActiveModel {
        entry_id: Set(entry.id),
        path: Set(path),
    };

    directory_paths::Entity::insert(dir_path)
        .on_conflict(
            OnConflict::column(directory_paths::Column::EntryId)
                .update_column(directory_paths::Column::Path)
                .to_owned()
        )
        .exec(db)
        .await?;

    Ok(())
}
```

#### 2. Bulk Rebuild for Backfill

When a new device joins and syncs thousands of entries:

```rust
// After backfilling entries for a location
pub async fn rebuild_all_directory_paths(
    location_id: i32,
    db: &DatabaseConnection,
) -> Result<u64, DbErr> {
    // Get location to find root entry
    let location = location::Entity::find_by_id(location_id)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Location not found".to_string()))?;

    // Start from location's root entry
    let root_path = get_location_root_path(&location)?;

    // Recursive rebuild
    let count = rebuild_paths_recursive(location.entry_id, &root_path, db).await?;

    tracing::info!(
        location_id = location_id,
        paths_rebuilt = count,
        "Rebuilt directory paths after sync backfill"
    );

    Ok(count)
}
```

#### 3. Handle Edge Cases

**Orphaned entries**: If parent hasn't synced yet:
```rust
// Queue for later rebuild
if entry.parent_id.is_some() {
    match rebuild_directory_path_for_entry(&entry, db).await {
        Ok(_) => {},
        Err(DbErr::RecordNotFound(_)) => {
            // Parent not synced yet, queue for retry
            pending_directory_paths.insert(entry.id);
        }
        Err(e) => return Err(e),
    }
}
```

**Directory moves**: When parent_id changes:
```rust
// When updating an entry's parent_id
if old_parent_id != new_parent_id && entry.kind == Directory {
    // Rebuild this directory and all descendants
    rebuild_directory_path_for_entry(&entry, db).await?;
    rebuild_descendant_paths(entry.id, db).await?;
}
```

---

## Foreign Key Mapping Considerations

### Parent Entry References

The `parent_id` field in entries is a self-referential FK that must be handled correctly:

```rust
impl Syncable for entry::Model {
    fn foreign_key_mappings() -> Vec<FKMapping> {
        vec![
            // Self-referential FK to parent entry
            FKMapping::new("parent_id", "entries"),
            // Other optional FKs
            FKMapping::new("metadata_id", "user_metadata"),
            FKMapping::new("content_id", "content_identities"),
        ]
    }
}
```

**Sync JSON transformation**:
```json
// Local representation (before sync)
{
  "id": 51,
  "uuid": "def-456",
  "name": "beach",
  "parent_id": 50,
  "metadata_id": null,
  "content_id": 42
}

// Sync representation (sent over wire)
{
  "uuid": "def-456",
  "name": "beach",
  "parent_uuid": "abc-123",      // Mapped from parent_id
  "metadata_uuid": null,          // Null FKs stay null
  "content_uuid": "content-789"   // Mapped from content_id
}

// Received and mapped back to local (Device B)
{
  "id": 74,                    // New local ID assigned
  "uuid": "def-456",
  "name": "beach",
  "parent_id": 73,             // Mapped from parent_uuid to Device B's local ID
  "metadata_id": null,
  "content_id": 58             // Mapped to Device B's local content ID
}
```

### Dependency Ordering

Entries must sync in parent-before-child order to ensure `parent_uuid` can be mapped:

```rust
// Sync system automatically handles this via topological sort
// Because we declared: sync_depends_on() -> &["location"]

// Within a location's entries, process in depth order:
// 1. Root entries (parent_id = NULL)
// 2. Depth 1 entries (children of root)
// 3. Depth 2 entries (grandchildren)
// ... and so on

// This is handled by entry_closure table or depth-first traversal
```

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_directory_path_not_synced() {
    let device_a = create_test_device().await;
    let device_b = create_test_device().await;

    // Device A creates directory
    let entry = device_a.create_directory("Photos", parent_id).await?;

    // Verify directory_path exists locally
    let dir_path = directory_paths::Entity::find_by_id(entry.id)
        .one(device_a.db())
        .await?;
    assert!(dir_path.is_some());

    // Get sync JSON
    let sync_json = entry.to_sync_json()?;

    // Verify no path-related fields
    assert!(sync_json.get("path").is_none());
    assert!(sync_json.get("entry_id").is_none());

    // Device B receives sync
    device_b.apply_entry_state_change(sync_json).await?;

    // Verify entry exists with different local ID
    let entry_b = device_b.find_entry_by_uuid(entry.uuid).await?;
    assert_ne!(entry.id, entry_b.id);  // Different local IDs!

    // Verify directory_path was rebuilt with Device B's local ID
    let dir_path_b = directory_paths::Entity::find_by_id(entry_b.id)
        .one(device_b.db())
        .await?;
    assert!(dir_path_b.is_some());
    assert_eq!(dir_path_b.unwrap().entry_id, entry_b.id);
}

#[tokio::test]
async fn test_path_resolution_after_sync() {
    // Setup: Device A indexes a file hierarchy
    // Sync to Device B
    // Verify: PathResolver works correctly on Device B

    let file_entry = /* synced file entry */;
    let full_path = PathResolver::get_full_path(device_b.db(), file_entry.id).await?;

    // Should use Device B's filesystem paths, not Device A's
    assert!(full_path.starts_with(device_b.root_path()));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_large_directory_tree_sync() {
    // Create deep hierarchy on Device A: 1000 directories, 10000 files
    // Sync to Device B
    // Verify all directory_paths are correct
    // Verify PathResolver works for all files
}
```

---

## Summary

### Key Decisions

1. **Do NOT sync `directory_paths`** - It's derived data and device-specific
2. **Sync entries with hierarchy** - `parent_id` → `parent_uuid` mapping
3. **Rebuild locally** - Each device reconstructs `directory_paths` for its filesystem
4. **Optimize bulk operations** - Batch rebuild during backfill

### Benefits

- ✅ **Correct**: Each device has filesystem-appropriate paths
- ✅ **Simple**: No complex path transformation in sync protocol
- ✅ **Efficient**: Rebuild is cheap (single traversal)
- ✅ **Portable**: Entry hierarchy is universal, paths are local

### Trade-offs

- Requires rebuild after sync (acceptable cost)
- Slight delay before PathResolver is fully functional (< 1 second)
- Need to handle orphaned entries during backfill (queueing)

---

## Related Documentation

- `core/src/infra/sync/fk_mapper.rs` - FK UUID mapping implementation
- `core/src/ops/indexing/path_resolver.rs` - Path resolution using directory_paths cache
- `core/src/ops/indexing/persistence.rs` - Directory path creation during indexing
- `docs/core/sync.md` - Overall sync architecture

