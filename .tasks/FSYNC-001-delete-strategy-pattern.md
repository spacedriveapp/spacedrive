---
id: FSYNC-001
title: DeleteJob Strategy Pattern & Remote Deletion
status: To Do
assignee: james
parent: FSYNC-000
priority: High
tags: [delete, strategy, remote, networking]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-14
---

## Description

Bring DeleteJob up to parity with FileCopyJob's architecture by implementing the strategy pattern with local and remote delete capabilities. This enables File Sync to dispatch delete operations across devices.

**Current State:** DeleteJob only supports local deletion with no routing logic
**Target State:** DeleteJob uses strategy pattern with local and remote strategies

## Problem

File Sync needs to delete files on remote devices as part of Mirror and Bidirectional sync modes. The current DeleteJob lacks:

- Strategy pattern for routing (FileCopyJob has this)
- Cross-device deletion capability
- Consistent architecture with other file operations

## Implementation Steps

### 1. Create DeleteStrategy Trait

```rust
// Location: core/src/ops/files/delete/strategy.rs

#[async_trait]
pub trait DeleteStrategy: Send + Sync {
    async fn execute(
        &self,
        ctx: &JobContext<'_>,
        paths: &[SdPath],
        mode: DeleteMode,
    ) -> Result<Vec<DeleteResult>>;
}

pub struct DeleteResult {
    pub path: SdPath,
    pub success: bool,
    pub bytes_freed: u64,
    pub error: Option<String>,
}
```

### 2. Implement LocalDeleteStrategy

Move existing DeleteJob logic into LocalDeleteStrategy:

- `move_to_trash()` for DeleteMode::Trash
- `permanent_delete()` for DeleteMode::Permanent
- `secure_delete()` for DeleteMode::Secure

### 3. Implement RemoteDeleteStrategy

```rust
// core/src/ops/files/delete/strategy.rs

pub struct RemoteDeleteStrategy;

impl DeleteStrategy for RemoteDeleteStrategy {
    async fn execute(&self, ctx, paths, mode) -> Result<Vec<DeleteResult>> {
        // Group paths by target device
        // Send delete request to each device via networking service
        // Parse response and return aggregated results
    }
}
```

**Network Protocol:**

```rust
pub enum FileDeleteMessage {
    Request {
        paths: Vec<SdPath>,
        mode: DeleteMode,
        request_id: Uuid,
    },
    Response {
        request_id: Uuid,
        results: Vec<DeleteResult>,
    },
}
```

### 4. Create DeleteStrategyRouter

```rust
// core/src/ops/files/delete/routing.rs

pub struct DeleteStrategyRouter;

impl DeleteStrategyRouter {
    pub async fn select_strategy(
        paths: &[SdPath],
        volume_manager: Option<&VolumeManager>,
    ) -> Box<dyn DeleteStrategy> {
        let all_local = paths.iter().all(|p| p.is_local());

        if all_local {
            Box::new(LocalDeleteStrategy)
        } else {
            Box::new(RemoteDeleteStrategy)
        }
    }
}
```

### 5. Update DeleteJob to Use Strategies

Refactor DeleteJob::run() to:

1. Select strategy via DeleteStrategyRouter
2. Execute deletion using selected strategy
3. Aggregate results and return DeleteOutput

## Files to Create/Modify

**New Files:**

- `core/src/ops/files/delete/strategy.rs` - Strategy trait and implementations
- `core/src/ops/files/delete/routing.rs` - Strategy router

**Modified Files:**

- `core/src/ops/files/delete/job.rs` - Refactor to use strategies
- `core/src/ops/files/delete/mod.rs` - Export new modules

**Networking:**

- `core/src/service/networking/handlers.rs` - Add file_delete handler

## Acceptance Criteria

- [ ] DeleteStrategy trait defined with execute() method
- [ ] LocalDeleteStrategy implements existing delete logic
- [ ] RemoteDeleteStrategy sends requests via networking service
- [ ] DeleteStrategyRouter selects strategy based on path locations
- [ ] DeleteJob refactored to use strategy pattern
- [ ] Network protocol handler for remote deletion
- [ ] Integration test: Delete local files via LocalStrategy
- [ ] Integration test: Delete remote files via RemoteStrategy
- [ ] Mixed local/remote deletions work correctly

## Technical Notes

**Why Strategy Pattern?**

- Consistent with FileCopyJob architecture (CopyStrategy pattern)
- Separates concerns: routing logic vs. deletion logic
- Easy to add new strategies (CloudDeleteStrategy for S3/R2)
- Testable: Mock strategies for unit tests

**Networking Integration:**
Reuses existing P2P infrastructure:

- QUIC transport for reliability
- Compression for small message payloads
- Request/response pattern with timeout handling

## References

- Implementation: FILE_SYNC_IMPLEMENTATION_PLAN.md (Lines 45-315)
- Similar pattern: core/src/ops/files/copy/strategy.rs (CopyStrategy)
- Related: FILE-001 (File Copy Job), FSYNC-003 (FileSyncService)
