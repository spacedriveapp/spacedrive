---
id: FSYNC-009
title: State-Based Reconciliation Engine
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, core-logic]
depends_on: [FSYNC-003]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the core state-based reconciliation algorithm that compares live filesystem state against the VDFS index to generate a delta (list of COPY/DELETE operations). This is the heart of the sync system.

## Implementation Notes

Create `src/ops/sync/reconciliation.rs`:

```rust
pub struct Reconciler<'a> {
    db: &'a DatabaseConnection,
    source_entry: Entry,
    dest_entry: Entry,
}

impl<'a> Reconciler<'a> {
    /// Perform state reconciliation between source and destination
    pub async fn reconcile(&self) -> Result<Delta> {
        // 1. Scan source filesystem
        let source_state = self.scan_directory(&self.source_entry).await?;

        // 2. Scan destination filesystem
        let dest_state = self.scan_directory(&self.dest_entry).await?;

        // 3. Use VDFS index as cache to skip unchanged files
        let source_indexed = self.get_indexed_state(&self.source_entry).await?;
        let dest_indexed = self.get_indexed_state(&self.dest_entry).await?;

        // 4. Compare states and generate operations
        let mut delta = Delta::new();

        for (path, source_file) in &source_state {
            match dest_state.get(path) {
                None => {
                    // File exists in source but not dest - COPY
                    delta.add_copy(source_file.clone());
                }
                Some(dest_file) => {
                    // Check if modified (use BLAKE3 content hash)
                    if source_file.content_hash != dest_file.content_hash {
                        delta.add_copy(source_file.clone());
                    }
                }
            }
        }

        // Check for files in dest but not source (may need DELETE)
        for (path, dest_file) in &dest_state {
            if !source_state.contains_key(path) {
                delta.add_delete(dest_file.clone());
            }
        }

        Ok(delta)
    }

    /// Fast scan using VDFS index as cache
    async fn scan_directory(&self, entry: &Entry) -> Result<HashMap<PathBuf, FileMetadata>> {
        // Use watcher's indexed data + stat() to verify
    }
}

#[derive(Debug)]
pub struct Delta {
    pub copy_ops: Vec<FileMetadata>,
    pub delete_ops: Vec<FileMetadata>,
}
```

## Acceptance Criteria

- [ ] Reconciler struct implemented
- [ ] Fast scanning using VDFS index as cache
- [ ] Accurate delta calculation (COPY/DELETE)
- [ ] Content hash comparison (BLAKE3)
- [ ] Performance: Handle 100K+ files efficiently
- [ ] Unit tests for various sync scenarios

