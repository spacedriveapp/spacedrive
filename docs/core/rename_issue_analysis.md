# Rename Issue Analysis - V1 vs V2 Comparison

## Executive Summary

The rename detection in V2's macOS handler is broken because **it cannot get the inode for files that no longer exist on disk**. V1 solved this by querying the database for the inode, but V2's platform handler lacks database access.

## The Problem

When macOS FSEvents reports a rename operation, it sends two separate events:
1. **Old path event**: The file at this path no longer exists (already renamed)
2. **New path event**: The file now exists at this new path

The challenge: To match these two events, we need the **inode** as the linking key. But how do we get the inode for a file that no longer exists?

## V1's Solution (Working)

### Code Location
`spacedrive_v1/core/src/location/manager/watcher/macos.rs:384-477`

### Algorithm

```rust
async fn handle_single_rename_event(&mut self, path: PathBuf) -> Result<()> {
    match fs::metadata(&path).await {
        Ok(meta) => {
            // FILE EXISTS - this is the "new" path
            let inode = get_inode(&meta);

            // Check if we already have this entry in DB (avoid duplicates)
            if !check_file_path_exists(&path, &db).await? {
                // Check if old_paths_map has this inode
                if let Some((_, old_path)) = self.old_paths_map.remove(&inode) {
                    // MATCH! Emit rename event
                    rename(location_id, &path, &old_path, meta, &library).await?;
                } else {
                    // No match yet, store for later
                    self.new_paths_map.insert(inode, (Instant::now(), path));
                }
            }
        }

        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // FILE DOESN'T EXIST - this is the "old" path

            // ⭐ CRITICAL: Query database to get inode for old path
            let inode = extract_inode_from_path(location_id, &path, &library).await?;

            // Check if new_paths_map has this inode
            if let Some((_, new_path)) = self.new_paths_map.remove(&inode) {
                // MATCH! Emit rename event
                rename(location_id, &new_path, &path, ...).await?;
            } else {
                // No match yet, store for later
                self.old_paths_map.insert(inode, (Instant::now(), path));
            }
        }
    }
}
```

### Key Function: `extract_inode_from_path()`

Located in `spacedrive_v1/core/src/location/manager/watcher/utils.rs:1116-1149`:

```rust
pub(super) async fn extract_inode_from_path(
    location_id: location::id::Type,
    path: impl AsRef<Path> + Send,
    library: &Library,
) -> Result<INode, LocationManagerError> {
    library
        .db
        .file_path()
        .find_first(loose_find_existing_file_path_params(
            location_id,
            location_path,
            path,
        )?)
        .select(file_path::select!({ inode }))
        .exec()
        .await?
        .map_or(
            Err(FilePathError::NotFound(path.into()).into()),
            |file_path| {
                Ok(inode_from_db(&file_path.inode[0..8]))
            },
        )
}
```

**This is the magic**: Query the database to get the inode of a file that no longer exists!

## V2's Current Implementation (Broken)

### Code Location
`spacedrive/core/src/service/watcher/platform/macos.rs:124-207`

### Algorithm

```rust
async fn handle_single_rename_event(&self, path: PathBuf, ...) -> Result<Vec<Event>> {
    match tokio::fs::metadata(&path).await {
        Ok(metadata) => {
            // FILE EXISTS - this is the "new" path
            if let Some(inode) = self.get_inode_from_path(&path).await {
                let mut old_paths = self.old_paths_map.write().await;
                if let Some((_, old_path)) = old_paths.remove(&inode) {
                    // MATCH! Emit rename event
                    events.push(Event::FsRawChange {
                        library_id: location.library_id,
                        kind: FsRawEventKind::Rename { from: old_path, to: path },
                    });
                } else {
                    // No match yet, store for later
                    let mut new_paths = self.new_paths_map.write().await;
                    new_paths.insert(inode, (Instant::now(), path));
                }
            }
        }

        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // FILE DOESN'T EXIST - this is the "old" path

            // ❌ BUG: Tries to get inode from filesystem (FAILS!)
            // Since the file doesn't exist anymore, we can't get its inode from the filesystem
            // Instead, we need to track the path itself temporarily and match it with the new path
            // For now, just store the path - we'll need the context to look up the inode from DB
            // This is a limitation of the current simplified implementation

            // For now, treat as a potential deletion
            let locations = watched_locations.read().await;
            for location in locations.values() {
                if path.starts_with(&location.path) {
                    events.push(Event::FsRawChange {
                        library_id: location.library_id,
                        kind: FsRawEventKind::Remove { path: path.clone() },
                    });
                    break;
                }
            }
        }
    }
}
```

### The Bug

Lines 174-179 have a comment acknowledging the limitation:
```rust
// Since the file doesn't exist anymore, we can't get its inode from the filesystem
// Instead, we need to track the path itself temporarily and match it with the new path
// For now, just store the path - we'll need the context to look up the inode from DB
// This is a limitation of the current simplified implementation
```

But then it just emits a Remove event instead of actually implementing the fix!

## V2's Infrastructure (Already Available!)

V2 already has the necessary database query functions in `responder.rs`:

```rust
/// Resolve an entry ID by absolute path (directory first, then file by parent/name/extension)
async fn resolve_entry_id_by_path(ctx: &impl IndexingCtx, abs_path: &Path) -> Result<Option<i32>> {
    // ...
}

/// Resolve a directory entry by exact cached path in directory_paths
async fn resolve_directory_entry_id(ctx: &impl IndexingCtx, abs_path: &Path) -> Result<Option<i32>> {
    let path_str = abs_path.to_string_lossy().to_string();
    let model = entities::directory_paths::Entity::find()
        .filter(entities::directory_paths::Column::Path.eq(path_str))
        .one(ctx.library_db())
        .await?;
    Ok(model.map(|m| m.entry_id))
}

/// Resolve a file entry by parent directory path + file name (+ extension)
async fn resolve_file_entry_id(ctx: &impl IndexingCtx, abs_path: &Path) -> Result<Option<i32>> {
    // Queries: parent directory → entry by parent_id + name + extension
}
```

And once we have the entry_id, we can get the inode:

```rust
let entry = entities::entry::Entity::find_by_id(entry_id)
    .one(db)
    .await?;
let inode = entry.inode; // i64 field
```

## Solution Options

### Option A: Pass Database Context to MacOSHandler (Recommended)

**Approach**: Give the platform handler access to database to query inodes.

**Pros**:
- Matches V1's proven approach
- Clean separation of concerns (platform handles platform-specific logic)
- Minimal changes to event flow

**Cons**:
- Platform handler now depends on DB (but only for rename logic)
- Need to pass library_id or db connection through

**Implementation**:
1. Add `library_id` and `CoreContext` to `MacOSHandler`
2. Create helper method `get_inode_from_db(&self, path: &Path) -> Option<u64>`
3. Update `handle_single_rename_event()` to use DB lookup when file doesn't exist
4. Keep the existing eviction logic

### Option B: Move Rename Matching to Responder

**Approach**: Platform handler emits separate old/new path events, responder buffers and matches them.

**Pros**:
- Platform handler stays pure (no DB dependency)
- Responder already has full DB access

**Cons**:
- More complex event flow
- Need to buffer events in responder (timing issues)
- Responder is meant to be stateless

**Implementation**:
1. Emit `FsRawEventKind::RenamePart { path, existed: bool }` events
2. Responder buffers these events
3. Match pairs by querying DB for both inodes
4. Emit final Rename event

### Option C: Hybrid Approach

**Approach**: Platform handler tracks paths, responder enhances with DB lookup.

**Pros**:
- Shares the work appropriately
- Platform handler stays mostly pure

**Cons**:
- Most complex to implement
- Split responsibility makes debugging harder

## Recommended Solution: Option A

### Why Option A?

1. **Proven**: V1 used this approach successfully
2. **Simpler**: Single point of rename logic (platform handler)
3. **Efficient**: Only query DB when needed (file doesn't exist)
4. **Maintainable**: All rename logic in one place

### Implementation Steps

1. **Update MacOSHandler signature**:
```rust
pub struct MacOSHandler {
    // ... existing fields ...

    /// Database connection for inode lookups (needed for rename tracking)
    library_db: Option<Arc<sea_orm::DatabaseConnection>>,
    location_id: Option<Uuid>,
}
```

2. **Add method to query inode from DB**:
```rust
async fn get_inode_from_db(&self, path: &Path) -> Option<u64> {
    let db = self.library_db.as_ref()?;

    // Try directory lookup first
    let path_str = path.to_string_lossy().to_string();
    if let Ok(Some(dir)) = directory_paths::Entity::find()
        .filter(directory_paths::Column::Path.eq(&path_str))
        .one(db)
        .await
    {
        let entry = entry::Entity::find_by_id(dir.entry_id)
            .one(db)
            .await
            .ok()??;
        return Some(entry.inode? as u64);
    }

    // Try file lookup by parent + name + extension
    let parent = path.parent()?;
    let parent_str = parent.to_string_lossy().to_string();
    let parent_dir = directory_paths::Entity::find()
        .filter(directory_paths::Column::Path.eq(parent_str))
        .one(db)
        .await
        .ok()??;

    let name = path.file_stem()?.to_str()?;
    let ext = path.extension().and_then(|s| s.to_str());

    let mut q = entry::Entity::find()
        .filter(entry::Column::ParentId.eq(parent_dir.entry_id))
        .filter(entry::Column::Name.eq(name));
    if let Some(e) = ext {
        q = q.filter(entry::Column::Extension.eq(e));
    } else {
        q = q.filter(entry::Column::Extension.is_null());
    }

    let entry = q.one(db).await.ok()??;
    entry.inode.map(|i| i as u64)
}
```

3. **Update `handle_single_rename_event()`**:
```rust
async fn handle_single_rename_event(...) -> Result<Vec<Event>> {
    // ...
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        // File doesn't exist - this is the "old" part of a rename
        trace!("Rename event: path doesn't exist {}", path.display());

        // ⭐ Query database to get inode for old path
        if let Some(inode) = self.get_inode_from_db(&path).await {
            // Check if new_paths_map has this inode
            let mut new_paths = self.new_paths_map.write().await;
            if let Some((_, new_path)) = new_paths.remove(&inode) {
                // We found a match! This is a real rename operation
                trace!("Detected rename: {} -> {}", path.display(), new_path.display());

                // Generate rename event
                let locations = watched_locations.read().await;
                for location in locations.values() {
                    if new_path.starts_with(&location.path) {
                        events.push(Event::FsRawChange {
                            library_id: location.library_id,
                            kind: FsRawEventKind::Rename {
                                from: path.clone(),
                                to: new_path,
                            },
                        });
                        break;
                    }
                }
            } else {
                // No matching new path yet - store in old_paths_map
                trace!("Storing old path for rename: {}", path.display());
                let mut old_paths = self.old_paths_map.write().await;
                old_paths.insert(inode, (Instant::now(), path.clone()));
            }
        } else {
            // Not in database - could be a file that was never indexed or a temp file
            trace!("Path not found in database, treating as removal: {}", path.display());
            // Emit removal event
            // ...
        }
    }
}
```

4. **Wire up DB connection in watcher initialization**:
```rust
// In LocationWatcher::ensure_worker_for_location() or similar
if let Some(macos_handler) = platform_handler.inner.as_macos() {
    macos_handler.set_library_db(library.db().conn(), location_id);
}
```

## Performance Considerations

### Query Cost
- Each unmatched old-path rename event triggers **1-2 DB queries**:
  - 1 query for directory path lookup
  - 1 query for entry lookup (if file)

### Optimization Opportunities
1. **Batch queries**: If multiple renames happen simultaneously, batch the lookups
2. **Cache recent lookups**: Keep small LRU cache of path → inode mappings
3. **Early exit**: If DB query fails (path not indexed), immediately treat as remove

### Expected Impact
- Typical rename: 2 events × 1-2 queries = **2-4 queries total**
- With 100ms timeout window: Most renames match within window, minimal DB load
- Worst case (no match): 2 queries + eventual eviction event

This is acceptable overhead for rename functionality.

## Testing Plan

1. **Single file rename**: `mv file.txt renamed.txt`
2. **Move to different directory**: `mv file.txt subdir/file.txt`
3. **Bulk renames**: Rename 100 files rapidly
4. **Edge case**: Rename file that's not in DB (should fallback to create/remove)
5. **Performance**: Measure query overhead with bulk operations

## Conclusion

The rename issue is well-understood and has a proven solution from V1. The v2 infrastructure already supports the necessary queries. The fix requires:

1. ✅ Adding DB access to MacOSHandler
2. ✅ Implementing `get_inode_from_db()` helper
3. ✅ Updating `handle_single_rename_event()` to use DB lookup
4. ✅ Testing the implementation

Estimated effort: 2-3 hours of implementation + testing.

