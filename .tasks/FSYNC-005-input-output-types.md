---
id: FSYNC-005
title: Sync Conduit Input/Output Types
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [file-sync, types]
depends_on: [FSYNC-002]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Define input and output types for sync conduit actions and jobs, ensuring type safety and proper serialization for the frontend API.

## Implementation Notes

Create `src/ops/sync/input.rs` and `src/ops/sync/output.rs`:

```rust
// input.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConduitCreateInput {
    pub source_entry_id: i32,
    pub destination_entry_id: i32,
    pub policy: SyncPolicy,
    pub policy_config: PolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncPolicy {
    Replicate,
    Synchronize,
    Offload,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub sync_cadence: SyncCadence,
    pub delete_after_copy: bool, // For Archive
    pub space_threshold_gb: Option<u64>, // For Offload
    // ... policy-specific fields
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncCadence {
    Instantly,
    EveryFiveMinutes,
    Hourly,
    Daily,
    Manual,
}

// output.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConduit {
    pub id: i32,
    pub uuid: Uuid,
    pub source_entry: EntryStub,
    pub destination_entry: EntryStub,
    pub policy: SyncPolicy,
    pub status: SyncStatus,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub stats: SyncStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStats {
    pub files_synced: u64,
    pub bytes_transferred: u64,
    pub last_duration_ms: u64,
}
```

## Acceptance Criteria

- [ ] All input types defined with validation
- [ ] Output types include necessary relations
- [ ] Enums for policies and cadences
- [ ] Types implement Serialize/Deserialize
- [ ] Documentation comments on public types

