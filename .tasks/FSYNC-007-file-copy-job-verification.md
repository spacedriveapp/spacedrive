---
id: FSYNC-007
title: FileCopyJob Verification State
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, job-system, verification]
depends_on: [FILE-001, FSYNC-006]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Modify `FileCopyJob` to add a `Verifying` state to its state machine. After a file is transferred, it enters this state, sends a `ValidationRequest`, and awaits a `ValidationResponse` before moving to `Completed`.

## Implementation Notes

Update `src/ops/file/copy.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum FileCopyState {
    Queued,
    Copying { bytes_transferred: u64 },
    Verifying { awaiting_response: bool }, // NEW
    Completed,
    Failed { error: String },
}

impl JobHandler for FileCopyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        match self.state {
            FileCopyState::Copying { .. } => {
                // ... existing copy logic
                self.state = FileCopyState::Verifying { awaiting_response: true };
            }
            FileCopyState::Verifying { .. } => {
                // Send ValidationRequest
                let response = self.network
                    .send_validation_request(ValidationRequest {
                        transfer_id: self.transfer_id,
                        destination_path: self.destination.clone(),
                        expected_blake3: self.source_hash.clone(),
                    })
                    .await?;

                if response.is_valid {
                    self.state = FileCopyState::Completed;
                } else {
                    self.state = FileCopyState::Failed {
                        error: "Hash mismatch".to_string()
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

## Acceptance Criteria

- [ ] Verifying state added to FileCopyState enum
- [ ] State transition after copy completion
- [ ] ValidationRequest sent to destination
- [ ] Hash comparison and error handling
- [ ] Only advance to Completed on successful verification
- [ ] Tests for verification flow

