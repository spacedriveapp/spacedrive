---
id: FSYNC-013
title: Offload Policy (Smart Cache)
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Low
tags: [file-sync, policy, storage-management]
depends_on: [FSYNC-009, VOL-000]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the **Offload** sync policy, which automatically moves least-recently-used files to free up space on a volume while keeping entries in the VDFS index for searchability.

## Use Case

> A video editor works on a laptop with a small SSD but has a large home server. Recently accessed files stay local, older files move to the server automatically when space is low.

## Implementation Notes

Create `src/ops/sync/policies/offload.rs`:

```rust
pub struct OffloadPolicy {
    volume_manager: Arc<VolumeManager>,
}

impl SyncPolicy for OffloadPolicy {
    async fn apply(
        &self,
        delta: Delta,
        config: PolicyConfig,
    ) -> Result<Vec<FileOperation>> {
        let mut operations = Vec::new();

        // Check source volume free space
        let volume = self.volume_manager.get_volume_for_entry(&self.source_entry)?;
        let free_space_gb = volume.free_space / (1024 * 1024 * 1024);

        if free_space_gb < config.space_threshold_gb {
            tracing::info!(
                "Volume below threshold ({} GB < {} GB), offloading files",
                free_space_gb, config.space_threshold_gb
            );

            // Get LRU files from source (sorted by accessed_at)
            let lru_files = self.get_lru_files(&self.source_entry, config.target_offload_gb).await?;

            for file in lru_files {
                // Skip pinned files
                if file.has_tag("Pinned") {
                    continue;
                }

                operations.push(FileOperation::Move {
                    source: file.path.clone(),
                    destination: self.map_destination_path(&file.path)?,
                });
            }
        }

        Ok(operations)
    }
}
```

## Configuration

```rust
pub struct OffloadConfig {
    pub space_threshold_gb: u64, // Trigger when below this
    pub target_offload_gb: u64, // How much to offload
}
```

## Acceptance Criteria

- [ ] OffloadPolicy monitors volume free space
- [ ] LRU selection based on `accessed_at`
- [ ] Respects "Pinned" tag
- [ ] Moves files (not copy)
- [ ] Entry remains in VDFS for search
- [ ] Unit tests for space calculations
- [ ] Integration test with volume manager

