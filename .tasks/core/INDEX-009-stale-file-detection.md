---
id: INDEX-009
title: Stale File Detection Algorithm
status: To Do
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, stale-detection, offline-recovery, sync]
whitepaper: Section 4.3.4
last_updated: 2025-12-16
related_tasks: [INDEX-004, LSYNC-020]
---

## Description

Implement the algorithm for detecting stale files after the application has been offline or when the watcher service was not running. This ensures that changes made while Spacedrive was not actively monitoring are correctly detected and reconciled when the app restarts or when manual verification is triggered.

## Problem Statement

The real-time change detection system (ChangeHandler trait) only captures events while Spacedrive is running and actively watching locations. When the app is:

- Stopped/offline
- Crashed unexpectedly
- Watcher paused or disabled
- Running on a different device

...filesystem changes are not immediately detected. Stale detection fills this gap by:

1. **Detecting offline modifications** - Files changed while app wasn't running
2. **Detecting offline deletions** - Files removed while app wasn't running
3. **Detecting offline moves** - Files renamed/moved while app wasn't running
4. **Detecting missed watcher events** - Edge cases where watcher failed to fire

## Current Implementation Status

The ChangeDetector in INDEX-004 provides the foundation for stale detection, but automated offline detection is not fully implemented:

- ✅ **Manual verification** - `IndexVerifyAction` can detect discrepancies on-demand
- ✅ **Batch change detection** - ChangeDetector compares filesystem vs database during reindex
- ❌ **Automatic startup detection** - App doesn't automatically check for stale files on launch
- ❌ **Last-seen timestamps** - No tracking of when watcher was last active per location
- ❌ **Smart rescanning** - No heuristics to determine which paths need stale detection
- ❌ **Background reconciliation** - No automated background stale file cleanup

## Proposed Architecture

### Watcher Lifecycle Tracking

Track when each location was last successfully watched:

```sql
CREATE TABLE location_watcher_state (
    location_id INTEGER PRIMARY KEY,
    last_watch_start TIMESTAMP,
    last_watch_stop TIMESTAMP,
    last_successful_event TIMESTAMP,
    watch_interrupted BOOLEAN
);
```

### Startup Stale Detection

On app startup, automatically trigger stale detection for locations that were:

1. **Watched during last session** - Check if any changes occurred while offline
2. **Interrupted** - Watcher crashed or was force-stopped
3. **Offline for >N hours** - Heuristic threshold for automatic scanning

```rust
async fn detect_stale_on_startup(library: &Library) -> Result<()> {
    let locations = load_watched_locations(&library.db).await?;

    for location in locations {
        let watcher_state = get_watcher_state(location.id, &library.db).await?;

        // Check if location needs stale detection
        if should_run_stale_detection(&watcher_state) {
            info!("Running stale detection for location {}", location.name);

            // Spawn background stale detection job
            let job = StaleDetectionJob::new(location.id);
            library.jobs().dispatch(job).await?;
        }
    }

    Ok(())
}

fn should_run_stale_detection(state: &WatcherState) -> bool {
    // Always run if interrupted
    if state.watch_interrupted {
        return true;
    }

    // Run if offline for more than 1 hour
    let offline_duration = Utc::now() - state.last_watch_stop;
    if offline_duration > Duration::hours(1) {
        return true;
    }

    // Run if no successful events in last session (watcher might have failed silently)
    if state.last_successful_event < state.last_watch_start {
        return true;
    }

    false
}
```

### Stale Detection Job

Similar to IndexVerifyAction but runs automatically:

```rust
pub struct StaleDetectionJob {
    location_id: i32,
}

impl Job for StaleDetectionJob {
    async fn execute(&self, ctx: &JobContext) -> Result<()> {
        // 1. Run ephemeral scan of location
        let ephemeral_index = self.scan_location(ctx).await?;

        // 2. Load database entries
        let db_entries = self.load_db_entries(ctx).await?;

        // 3. Compare and detect changes
        let changes = ChangeDetector::compare(&ephemeral_index, &db_entries);

        // 4. Apply changes to database
        for change in changes {
            match change {
                Change::New(path) => self.create_entry(path, ctx).await?,
                Change::Modified(path) => self.update_entry(path, ctx).await?,
                Change::Moved { old, new } => self.move_entry(old, new, ctx).await?,
                Change::Deleted(path) => self.delete_entry(path, ctx).await?,
            }
        }

        // 5. Update watcher state
        self.mark_location_reconciled(ctx).await?;

        Ok(())
    }
}
```

### Inode-Based Move Detection (Critical for Offline Changes)

When app is offline, files can be moved/renamed. On restart, detect these via inode matching:

```rust
async fn detect_moves(
    ephemeral_entries: &HashMap<PathBuf, FileNode>,
    db_entries: &HashMap<PathBuf, EntryRecord>,
) -> Vec<MoveOperation> {
    let mut moves = Vec::new();

    // Build inode → db_entry map
    let mut inode_map: HashMap<u64, &EntryRecord> = HashMap::new();
    for entry in db_entries.values() {
        if let Some(inode) = entry.inode {
            inode_map.insert(inode, entry);
        }
    }

    // Check each filesystem entry
    for (fs_path, fs_node) in ephemeral_entries {
        if let Some(inode) = fs_node.inode {
            // File exists in DB with same inode but different path?
            if let Some(db_entry) = inode_map.get(&inode) {
                if db_entry.path != *fs_path {
                    moves.push(MoveOperation {
                        entry_id: db_entry.id,
                        old_path: db_entry.path.clone(),
                        new_path: fs_path.clone(),
                        inode,
                    });
                }
            }
        }
    }

    moves
}
```

**Critical**: This only works on Unix systems. Windows requires fallback to path-only matching.

## Implementation Plan

### Phase 1: Watcher State Tracking

**Files**:
- `core/src/infra/db/migrations/` - Add `location_watcher_state` table
- `core/src/service/watcher/mod.rs` - Update watcher start/stop to record timestamps
- `core/src/service/watcher/worker.rs` - Update last_successful_event on each event

**Tasks**:
1. Add database schema for watcher lifecycle tracking
2. Record watcher start/stop times per location
3. Update timestamp on each successful event
4. Mark interrupted flag on unexpected shutdown

### Phase 2: Startup Stale Detection

**Files**:
- `core/src/library/mod.rs` - Hook startup stale detection
- `core/src/ops/indexing/stale.rs` - New module for stale detection logic

**Tasks**:
1. Implement `detect_stale_on_startup()` function
2. Check watcher state for each location
3. Spawn StaleDetectionJob for locations needing reconciliation
4. Don't block app startup (run in background)

### Phase 3: StaleDetectionJob Implementation

**Files**:
- `core/src/ops/indexing/jobs/stale_detection.rs` - New job type

**Tasks**:
1. Create StaleDetectionJob similar to IndexVerifyAction
2. Run ephemeral scan + database comparison
3. Apply changes via DatabaseAdapter
4. Update watcher state on completion
5. Report results to user (notification or log)

### Phase 4: Inode-Based Move Detection

**Files**:
- `core/src/ops/indexing/change_detection/detector.rs` - Enhance with move detection

**Tasks**:
1. Build inode → entry map from database
2. Compare filesystem inodes against database
3. Detect same inode at different path
4. Handle Windows fallback (no stable inodes)

### Phase 5: UI Integration

**Files**:
- `packages/interface/src/` - Notification UI for stale detection results

**Tasks**:
1. Show notification when stale files detected
2. Display count of changes found (new/modified/deleted)
3. Allow user to review changes before applying
4. Add setting to enable/disable automatic stale detection

## Acceptance Criteria

- [ ] Watcher state tracked in database (start/stop/last_event timestamps)
- [ ] App startup triggers stale detection for offline locations
- [ ] StaleDetectionJob runs in background without blocking startup
- [ ] Detects new files created while offline
- [ ] Detects modified files (size/mtime changed while offline)
- [ ] Detects deleted files (removed while offline)
- [ ] Detects moved files via inode matching (Unix systems)
- [ ] Windows fallback works (path-only matching)
- [ ] User notified when stale files found and reconciled
- [ ] Settings allow disabling automatic stale detection
- [ ] Manual stale detection still available via IndexVerifyAction
- [ ] Doesn't run stale detection if watcher was active until shutdown
- [ ] Handles edge case: location on external drive that was unmounted

## Edge Cases

### External Drive Unmounted While Offline

**Scenario**: USB drive was ejected while app offline

**Behavior**:
- On startup, drive is not mounted
- Stale detection should skip (don't mark files as deleted)
- Wait for drive to be mounted before reconciling

**Solution**:
```rust
// Check if location path is accessible before stale detection
if !location_path.exists() {
    info!("Location {} not accessible, skipping stale detection", location.name);
    return Ok(());
}
```

### Very Long Offline Period

**Scenario**: App offline for weeks, thousands of changes

**Behavior**:
- Don't block startup with massive scan
- Run stale detection in low-priority background job
- Show progress in UI

### Multiple Devices with Same Location

**Scenario**: Device A and Device B both have `/shared` mounted. Device A was offline.

**Behavior**:
- Device A's stale detection might conflict with Device B's changes
- Need to coordinate via library sync
- Device B's changes should have higher authority (it was online)

**Related**: LSYNC-020 (Device-Owned Deletion Sync)

## Testing

### Manual Testing

```bash
# 1. Start Spacedrive and add location
spacedrive start
spacedrive location add ~/Documents

# 2. Verify watcher active
spacedrive location info ~/Documents | grep "watcher: active"

# 3. Stop Spacedrive
spacedrive stop

# 4. Make changes while offline
touch ~/Documents/new_file.txt
echo "modified" >> ~/Documents/existing.txt
rm ~/Documents/old.txt

# 5. Restart Spacedrive
spacedrive start

# 6. Verify stale detection ran
spacedrive job list | grep StaleDetection

# 7. Check changes applied
spacedrive db query "SELECT * FROM entry WHERE name = 'new_file.txt'"
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_stale_detection_on_startup` - Verify automatic startup detection
- `test_watcher_state_tracking` - Verify timestamps recorded
- `test_stale_detection_skips_if_recent` - Don't run if just stopped
- `test_stale_detection_detects_offline_changes` - Full offline change cycle
- `test_stale_detection_inode_moves` - Move detection via inodes

## Performance Considerations

### Startup Impact

- Stale detection should NOT block app startup
- Run in low-priority background thread
- User can interact with app while detection runs
- Show progress in notification/status bar

### Large Locations

For locations with 1M+ files:
- Stale detection could take 5-10 minutes
- Don't run automatically if location >500K files
- Prompt user instead: "Location ~/Photos has been offline. Run stale detection?"

### Frequency Tuning

- **< 1 hour offline**: Skip (watcher state is fresh)
- **1-24 hours offline**: Run automatically
- **> 24 hours offline**: Prompt user before running
- **> 1 week offline**: Always prompt (likely external drive)

## Related Tasks

- INDEX-004 - Change Detection System (provides ChangeDetector foundation)
- INDEX-007 - Index Verification System (provides manual verification)
- LSYNC-020 - Device-Owned Deletion Sync (conflict resolution for multi-device)
- LOC-000 - Location Operations (watcher lifecycle)
