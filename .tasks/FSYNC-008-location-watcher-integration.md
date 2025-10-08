---
id: FSYNC-008
title: LocationWatcher Sync Conduit Integration
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [file-sync, indexing, watcher]
depends_on: [INDEX-001, FSYNC-003]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Update the `LocationWatcher` event handler to detect filesystem changes within Entries managed by Sync Conduits and trigger `SyncConduitJob` when appropriate based on the conduit's cadence.

## Implementation Notes

Update `src/ops/index/watcher.rs`:

```rust
impl LocationWatcher {
    async fn handle_filesystem_event(&mut self, event: FileSystemEvent) -> Result<()> {
        // ... existing event handling

        // Check if this path is under a sync conduit source
        let affected_conduits = self.find_conduits_for_path(&event.path).await?;

        for conduit in affected_conduits {
            // Check sync cadence
            match conduit.policy_config.sync_cadence {
                SyncCadence::Instantly => {
                    // Trigger immediately
                    self.job_manager.queue(SyncConduitJob {
                        sync_conduit_uuid: conduit.uuid,
                        current_phase: SyncPhase::DeltaCalculation,
                        processed_files: 0,
                        failed_files: vec![],
                    }).await?;
                }
                SyncCadence::EveryFiveMinutes => {
                    // Mark as needing sync, batch with timer
                    self.mark_conduit_dirty(conduit.uuid).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn find_conduits_for_path(&self, path: &Path) -> Result<Vec<SyncConduit>> {
        // Query sync_relationships where path is under source_entry
    }
}
```

## Acceptance Criteria

- [ ] Filesystem events checked against active conduits
- [ ] Instant cadence triggers immediate sync
- [ ] Batched cadences mark conduits as dirty
- [ ] Timer-based sync for non-instant cadences
- [ ] Performance: O(log n) lookup for conduits
- [ ] Integration test with watcher + conduit

