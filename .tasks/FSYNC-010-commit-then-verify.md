---
id: FSYNC-010
title: Commit-Then-Verify (CTV) Implementation
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, verification, reliability]
depends_on: [FSYNC-006, FSYNC-007]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the complete Commit-Then-Verify (CTV) flow, ensuring files are cryptographically verified before source deletion in Archive mode. This guarantees data integrity for critical operations.

## Implementation Notes

Create `src/ops/sync/verification.rs`:

```rust
pub struct CommitThenVerify {
    network: Arc<NetworkService>,
}

impl CommitThenVerify {
    /// Execute CTV protocol for a file transfer
    pub async fn verify_transfer(
        &self,
        transfer_id: Uuid,
        source_path: PathBuf,
        dest_path: PathBuf,
        dest_device: DeviceId,
    ) -> Result<VerificationResult> {
        // 1. Calculate source hash (if not already cached)
        let source_hash = self.calculate_blake3(&source_path).await?;

        // 2. Send ValidationRequest to destination
        let request = ValidationRequest {
            transfer_id,
            destination_path: dest_path.to_string_lossy().to_string(),
            expected_blake3: source_hash.clone(),
        };

        let response = self.network
            .send_to_device(dest_device, FileTransferMessage::ValidationRequest(request))
            .await?;

        // 3. Parse response
        match response {
            FileTransferMessage::ValidationResponse(resp) if resp.is_valid => {
                Ok(VerificationResult::Success)
            }
            FileTransferMessage::ValidationResponse(resp) => {
                Err(VerificationError::HashMismatch {
                    expected: source_hash,
                    actual: resp.actual_blake3,
                })
            }
            _ => Err(VerificationError::InvalidResponse),
        }
    }

    async fn calculate_blake3(&self, path: &Path) -> Result<String> {
        // Use blake3 crate for fast hashing
    }
}

#[derive(Debug)]
pub enum VerificationResult {
    Success,
    Retry { attempts_left: u32 },
}
```

## Use Cases

- **Archive Policy**: Only delete source after CTV succeeds
- **Critical Transfers**: Medical records, financial data
- **Network Failures**: Detect incomplete/corrupted transfers

## Acceptance Criteria

- [ ] Full CTV flow implemented
- [ ] BLAKE3 hashing on both sides
- [ ] Network protocol integration
- [ ] Retry logic for transient failures
- [ ] Error reporting to user
- [ ] Integration test: transfer + verify + delete

