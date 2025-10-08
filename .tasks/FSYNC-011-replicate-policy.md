---
id: FSYNC-011
title: Replicate Policy (One-Way Mirror)
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [file-sync, policy, backup]
depends_on: [FSYNC-009]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the **Replicate** sync policy, which creates a one-way mirror from source to destination. Perfect for automated backups.

## Use Case

> A photographer wants to automatically back up her `Active Projects` folder from her laptop's SSD to her NAS. New photos are copied over, and deletions are optionally mirrored.

## Implementation Notes

Create `src/ops/sync/policies/replicate.rs`:

```rust
pub struct ReplicatePolicy;

impl SyncPolicy for ReplicatePolicy {
    async fn apply(
        &self,
        delta: Delta,
        config: PolicyConfig,
    ) -> Result<Vec<FileOperation>> {
        let mut operations = Vec::new();

        // Always copy new/modified files from source to dest
        for file in delta.copy_ops {
            operations.push(FileOperation::Copy {
                source: file.path.clone(),
                destination: self.map_destination_path(&file.path)?,
            });
        }

        // Optionally mirror deletions (config-dependent)
        if config.mirror_deletions {
            for file in delta.delete_ops {
                operations.push(FileOperation::Delete {
                    path: self.map_destination_path(&file.path)?,
                });
            }
        }

        Ok(operations)
    }
}
```

## Configuration

```rust
pub struct ReplicateConfig {
    pub mirror_deletions: bool, // Default: false
    pub preserve_timestamps: bool, // Default: true
}
```

## Acceptance Criteria

- [ ] ReplicatePolicy struct implements SyncPolicy trait
- [ ] One-way propagation (source → dest)
- [ ] Optional deletion mirroring
- [ ] Timestamp preservation
- [ ] Unit tests for various scenarios
- [ ] Integration test with live filesystem

