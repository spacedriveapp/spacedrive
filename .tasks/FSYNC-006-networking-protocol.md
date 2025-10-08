---
id: FSYNC-006
title: File Transfer Validation Protocol
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, networking, protocol]
depends_on: [NET-001]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Extend the `file_transfer` protocol with messages for Commit-Then-Verify (CTV), enabling cryptographic verification of transferred files before deleting source copies.

## Implementation Notes

Update `src/service/network/protocol/file_transfer.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum FileTransferMessage {
    // ... existing messages (TransferRequest, TransferChunk, etc.)

    /// Request destination to verify a completed transfer
    ValidationRequest {
        transfer_id: Uuid,
        destination_path: String,
        expected_blake3: String,
    },

    /// Response from destination with verification result
    ValidationResponse {
        transfer_id: Uuid,
        is_valid: bool,
        actual_blake3: Option<String>,
        error: Option<String>,
    },
}

impl FileTransferProtocol {
    pub async fn handle_validation_request(
        &self,
        request: ValidationRequest,
    ) -> Result<ValidationResponse> {
        // 1. Read destination file
        // 2. Calculate BLAKE3 hash
        // 3. Compare with expected
        // 4. Return result
    }
}
```

## Acceptance Criteria

- [ ] ValidationRequest/Response messages defined
- [ ] Handler for validation requests implemented
- [ ] BLAKE3 hash calculation on destination
- [ ] Error handling for missing/corrupted files
- [ ] Integration test for full validation flow

