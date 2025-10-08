---
id: FSYNC-003
title: SyncConduitJob Definition
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, job-system]
depends_on: [FSYNC-002, JOB-000]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement the `SyncConduitJob`, the core job that executes sync operations for a conduit. This job is resumable, long-running, and orchestrates the entire sync lifecycle.

## Implementation Notes

Create `src/ops/sync/` module with `job.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::infra::job::prelude::*;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct SyncConduitJob {
    pub sync_conduit_uuid: Uuid,

    // Internal state for resumption
    pub current_phase: SyncPhase,
    pub processed_files: usize,
    pub failed_files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncPhase {
    DeltaCalculation,
    Executing { total: usize, completed: usize },
    Verifying,
    Completed,
}

impl Job for SyncConduitJob {
    const NAME: &'static str = "sync_conduit";
    const RESUMABLE: bool = true;
}

#[async_trait::async_trait]
impl JobHandler for SyncConduitJob {
    type Output = SyncOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // 1. Load sync conduit config
        // 2. Perform state-based reconciliation
        // 3. Generate delta (COPY/DELETE ops)
        // 4. Dispatch file operations
        // 5. Wait for verification
        // 6. Update last_sync_at
        todo!("Implement in FSYNC-009")
    }
}
```

## Acceptance Criteria

- [ ] `SyncConduitJob` struct defined with resumable state
- [ ] Job registered in job system
- [ ] Basic job lifecycle implemented (stub for now)
- [ ] Output type defined
- [ ] Compiles and can be instantiated

