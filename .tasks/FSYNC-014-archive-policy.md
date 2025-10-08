---
id: FSYNC-014
title: Archive Policy (Move & Consolidate)
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [file-sync, policy, archival]
depends_on: [FSYNC-010]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the **Archive** sync policy, which moves completed work to long-term storage with cryptographic verification before deletion. The safest policy for irreplaceable data.

## Use Case

> A researcher finishes a data analysis project and wants to move the entire folder to an archival drive. Files must be verified before deletion from the workstation.

## Implementation Notes

Create `src/ops/sync/policies/archive.rs`:

```rust
pub struct ArchivePolicy {
    ctv: Arc<CommitThenVerify>,
}

impl SyncPolicy for ArchivePolicy {
    async fn apply(
        &self,
        delta: Delta,
        config: PolicyConfig,
    ) -> Result<Vec<FileOperation>> {
        let mut operations = Vec::new();

        for file in delta.copy_ops {
            operations.push(FileOperation::CopyWithVerification {
                source: file.path.clone(),
                destination: self.map_destination_path(&file.path)?,
                verify_before_delete: true, // CRITICAL: CTV enabled
            });
        }

        Ok(operations)
    }

    async fn post_operation_hook(&self, operation: &FileOperation) -> Result<()> {
        match operation {
            FileOperation::CopyWithVerification { source, destination, .. } => {
                // Only delete source after successful CTV
                let verified = self.ctv.verify_transfer(
                    Uuid::new_v4(),
                    source.clone(),
                    destination.clone(),
                    self.destination_device,
                ).await?;

                match verified {
                    VerificationResult::Success => {
                        // Safe to delete source
                        tokio::fs::remove_file(source).await?;
                        tracing::info!("Archived and deleted: {:?}", source);
                    }
                    _ => {
                        tracing::error!("Verification failed, keeping source: {:?}", source);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

## Safety Features

- **Mandatory CTV**: Cannot be disabled
- **Hash Verification**: BLAKE3 on both sides
- **No Delete on Failure**: Source preserved if verification fails
- **Audit Log**: All archives logged

## Acceptance Criteria

- [ ] ArchivePolicy uses CTV for all transfers
- [ ] Source deleted ONLY after verification
- [ ] Detailed error handling
- [ ] Audit log of archived files
- [ ] Unit tests for verification flow
- [ ] Integration test: full archive + verify + delete

