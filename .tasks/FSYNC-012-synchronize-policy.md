---
id: FSYNC-012
title: Synchronize Policy (Two-Way Sync)
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [file-sync, policy, two-way]
depends_on: [FSYNC-009]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the **Synchronize** sync policy, which keeps two directories identical with bidirectional sync. Uses last-writer-wins conflict resolution.

## Use Case

> A developer works on a project from a desktop PC and a laptop. The project folder must be identical on both machines, with changes syncing in both directions.

## Implementation Notes

Create `src/ops/sync/policies/synchronize.rs`:

```rust
pub struct SynchronizePolicy;

impl SyncPolicy for SynchronizePolicy {
    async fn apply(
        &self,
        delta: Delta,
        config: PolicyConfig,
    ) -> Result<Vec<FileOperation>> {
        let mut operations = Vec::new();

        // For each file in delta, determine winner based on mtime
        for file in delta.copy_ops {
            let source_mtime = file.modified_at;
            let dest_file = self.get_destination_file(&file.path).await?;

            match dest_file {
                None => {
                    // File only exists in source - copy to dest
                    operations.push(FileOperation::Copy {
                        source: file.path.clone(),
                        destination: self.map_path(&file.path)?,
                    });
                }
                Some(dest) => {
                    // Both exist - last-writer-wins
                    if source_mtime > dest.modified_at {
                        operations.push(FileOperation::Copy {
                            source: file.path.clone(),
                            destination: self.map_path(&file.path)?,
                        });
                    } else {
                        operations.push(FileOperation::Copy {
                            source: dest.path.clone(),
                            destination: self.reverse_map_path(&dest.path)?,
                        });
                    }
                }
            }
        }

        Ok(operations)
    }
}
```

## Conflict Resolution

- **Last-Writer-Wins (LWW)**: Compare `modified_at` timestamps
- **No CRDTs**: Keep it simple for file content
- **Conflict Detection**: Log conflicts for user review

## Acceptance Criteria

- [ ] SynchronizePolicy implements bidirectional sync
- [ ] Last-writer-wins based on mtime
- [ ] Handles files in both directions
- [ ] Conflict logging for user visibility
- [ ] Unit tests for conflict scenarios
- [ ] Integration test with two-way changes

