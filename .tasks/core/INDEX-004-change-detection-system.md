---
id: INDEX-004
title: Change Detection System (Batch + Real-Time)
status: Done
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, change-detection, watcher, stale-detection]
whitepaper: Section 4.3.3
last_updated: 2025-12-16
---

## Description

Implement the dual-mode change detection system that keeps the index synchronized with filesystem state. Batch change detection runs during indexer jobs to detect offline changes (stale file detection), while real-time change detection processes filesystem watcher events as they occur.

## Architecture

### Batch Change Detection (ChangeDetector)

The `ChangeDetector` compares database state against filesystem during indexer scans:

```rust
pub struct ChangeDetector {
    // Maps inode → EntryRecord for existing entries
    inode_map: HashMap<u64, EntryRecord>,
    // Maps path → EntryRecord for path-only matching (Windows fallback)
    path_map: HashMap<PathBuf, EntryRecord>,
    // Tracks which entries we've seen this scan
    seen_entries: HashSet<i32>,
}
```

#### Detection Process

1. **Load Existing Entries**: Query database for all entries under indexing path
2. **Build Lookup Maps**: Create inode and path maps for fast comparisons
3. **Compare**: For each discovered filesystem entry, check against maps
4. **Classify Changes**:
   - **New**: Path not in database
   - **Modified**: Size or mtime differs
   - **Moved**: Same inode at different path (Unix only)
   - **Deleted**: In database but missing from filesystem
5. **Batch Process**: Execute changes in transactions

#### Inode Tracking

- **Unix**: Use stable inodes for move detection
- **Windows**: Fall back to path-only matching (file indices unstable across reboots)

```rust
impl ChangeDetector {
    async fn check_path(
        &self,
        path: &Path,
        metadata: &Metadata,
        inode: Option<u64>,
    ) -> Option<Change> {
        if let Some(inode) = inode {
            // Unix: Check inode first (detects moves)
            if let Some(existing) = self.inode_map.get(&inode) {
                if existing.path != path {
                    return Some(Change::Moved { old: existing.path, new: path });
                }
                if existing.size != metadata.len() || existing.mtime != metadata.modified() {
                    return Some(Change::Modified { path });
                }
                return None; // Unchanged
            }
        }

        // Not found by inode, check path
        if let Some(existing) = self.path_map.get(path) {
            if existing.size != metadata.len() || existing.mtime != metadata.modified() {
                return Some(Change::Modified { path });
            }
            return None; // Unchanged
        }

        // Not in database
        Some(Change::New { path })
    }

    fn find_deleted(&self) -> Vec<Change> {
        self.path_map
            .keys()
            .filter(|path| !self.seen_entries.contains(&self.path_map[path].id))
            .map(|path| Change::Deleted { path })
            .collect()
    }
}
```

### Real-Time Change Detection (ChangeHandler Trait)

The `ChangeHandler` trait defines the interface for responding to filesystem events:

```rust
pub trait ChangeHandler {
    async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>>;
    async fn create(&mut self, metadata: &DirEntry, parent_path: &Path) -> Result<EntryRef>;
    async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()>;
    async fn move_entry(&mut self, entry: &EntryRef, old_path: &Path, new_path: &Path) -> Result<()>;
    async fn delete(&mut self, entry: &EntryRef) -> Result<()>;
}
```

#### Implementations

**DatabaseAdapter** (Persistent):
- Writes to SQLite database
- Updates closure tables on moves
- Updates directory paths cache
- Creates sync operations for cross-device propagation

**MemoryAdapter** (Ephemeral):
- Updates EphemeralIndex in-memory structures
- Updates NameRegistry for name-based lookups
- No database I/O

#### Event Routing

The filesystem watcher routes events to the appropriate handler:

```rust
async fn handle_filesystem_event(&self, event: Event) -> Result<()> {
    let path = event.path();

    // Determine if this path belongs to ephemeral or persistent index
    if let Some(ephemeral_index) = self.ephemeral_cache.get_index_for_path(path).await {
        // Route to MemoryAdapter
        let mut adapter = MemoryAdapter::new(ephemeral_index);
        adapter.handle_change(event).await?;
    } else if let Some(location) = self.find_location_for_path(path, db).await? {
        // Route to DatabaseAdapter
        let mut adapter = DatabaseAdapter::new(db, location.id);
        adapter.handle_change(event).await?;
    }

    Ok(())
}
```

## Implementation Files

### Batch Change Detection
- `core/src/ops/indexing/change_detection/detector.rs` - ChangeDetector implementation
- `core/src/ops/indexing/change_detection/types.rs` - Change enum (New/Modified/Moved/Deleted)
- `core/src/ops/indexing/phases/processing.rs` - Integration into Phase 2

### Real-Time Change Detection
- `core/src/ops/indexing/change_detection/handler.rs` - ChangeHandler trait definition
- `core/src/ops/indexing/change_detection/persistent.rs` - DatabaseAdapter implementation
- `core/src/ops/indexing/handlers/persistent.rs` - DatabaseAdapter for ChangeHandler
- `core/src/ops/indexing/handlers/ephemeral.rs` - MemoryAdapter for ChangeHandler
- `core/src/ops/indexing/handlers/mod.rs` - Handler module exports

### Database Operations
- `core/src/ops/indexing/database_storage.rs` - Low-level CRUD used by DatabaseAdapter
- `core/src/ops/indexing/ephemeral/writer.rs` - In-memory operations used by MemoryAdapter

## Acceptance Criteria

### Batch Change Detection
- [x] ChangeDetector loads existing entries from database
- [x] Inode-based move detection works on Unix systems
- [x] Path-based fallback works on Windows
- [x] Detects New files (not in database)
- [x] Detects Modified files (size or mtime changed)
- [x] Detects Moved files (same inode, different path)
- [x] Detects Deleted files (in database, missing from filesystem)
- [x] Changes processed in batch transactions (1,000 items)
- [x] Integrated into Phase 2 (Processing)

### Real-Time Change Detection
- [x] ChangeHandler trait defines standard interface
- [x] DatabaseAdapter implements ChangeHandler for persistent storage
- [x] MemoryAdapter implements ChangeHandler for ephemeral storage
- [x] Filesystem events route to correct adapter (ephemeral vs persistent)
- [x] Create events insert new entries
- [x] Modify events update size/mtime
- [x] Move events update parent_id and rebuild closures
- [x] Delete events remove entries and closures
- [x] Directory path cache updated on create/move/delete

### Stale File Detection (Offline Changes)

**Note**: Automated stale detection on app startup is tracked separately in INDEX-009. The ChangeDetector provides the foundation but automatic reconciliation is not yet fully implemented.

## Platform-Specific Behavior

| Platform | Inode Support | Move Detection | Path Stability |
|----------|--------------|----------------|----------------|
| macOS | Yes (FSEvents) | Via inode | Stable |
| Linux | Yes | Via inode | Stable |
| Windows | Limited | Via path only | Unstable across reboots |

## Performance Characteristics

### Batch Change Detection
- **Load existing entries**: O(N) where N = entries in location
- **Build lookup maps**: O(N) hash map construction
- **Check each file**: O(1) hash lookup
- **Find deleted**: O(N) iteration
- **Total**: ~O(N) where N = files in location

### Real-Time Change Detection
- **Event routing**: O(1) hash lookup
- **Database write**: O(log N) SQLite insert
- **Closure update (move)**: O(subtree size)
- **Total per event**: ~O(1) to O(subtree) depending on operation

## Testing

### Manual Testing

```bash
# Test batch change detection (stale detection)
# 1. Index a directory
spacedrive index location ~/Documents --mode shallow

# 2. Stop Spacedrive
spacedrive stop

# 3. Make changes while offline
touch ~/Documents/new_file.txt
echo "modified" >> ~/Documents/existing.txt
mv ~/Documents/old.txt ~/Documents/renamed.txt
rm ~/Documents/deleted.txt

# 4. Restart and verify detection
spacedrive start
spacedrive index location ~/Documents --mode shallow

# Should detect: 1 new, 1 modified, 1 moved, 1 deleted
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_change_detector_new_files` - Detect new files
- `test_change_detector_modified_files` - Detect size/mtime changes
- `test_change_detector_moved_files_unix` - Detect moves via inode
- `test_change_detector_deleted_files` - Detect deleted files
- `test_change_handler_create` - Real-time create events
- `test_change_handler_modify` - Real-time modify events
- `test_change_handler_move` - Real-time move events
- `test_change_handler_delete` - Real-time delete events
- `test_stale_detection_after_offline` - Offline change detection

## Related Tasks

- INDEX-001 - Hybrid Architecture (defines DatabaseAdapter vs MemoryAdapter)
- INDEX-002 - Five-Phase Pipeline (Phase 2 uses ChangeDetector)
- INDEX-003 - Database Architecture (move operations rebuild closures)
- INDEX-009 - Stale File Detection (automated offline change reconciliation)
