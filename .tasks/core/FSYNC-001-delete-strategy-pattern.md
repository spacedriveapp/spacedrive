---
id: FSYNC-001
title: DeleteJob Strategy Pattern & Remote Deletion
status: Done
assignee: jamiepine
parent: FSYNC-000
priority: High
tags: [delete, strategy, remote, networking]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-15
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

## Progress Notes

**2025-10-15**: COMPLETE 
- Created `strategy.rs` with DeleteStrategy trait, LocalDeleteStrategy, RemoteDeleteStrategy (~350 lines)
- Created `routing.rs` with DeleteStrategyRouter (~50 lines)
- Refactored DeleteJob to use strategy pattern (reduced from ~400 to ~70 lines core logic)
- Created `file_delete.rs` network protocol handler (~250 lines)
- Updated module exports (ops/files/mod.rs, protocol/mod.rs)
- Made LocalDeleteStrategy methods public for network handler access
- Created comprehensive integration tests in `delete_strategy_test.rs` (7 tests)
- Fixed platform-specific trash implementation (macOS cfg attributes)
- All tests passing 

**2025-10-15**: VolumeBackend Integration 
- Added `delete()` method to VolumeBackend trait (core/src/volume/backend/mod.rs)
- Implemented delete() in LocalBackend (wraps tokio::fs operations)
- Implemented delete() in CloudBackend (uses OpenDAL delete/remove_all)
- Updated LocalDeleteStrategy to handle cloud paths via VolumeBackend
- Added `delete_cloud_path()` helper for cloud deletion routing
- Cloud paths now work through LocalDeleteStrategy (no separate CloudDeleteStrategy needed)
- Added 2 new cloud deletion tests (test_cloud_backend_delete_file, test_cloud_backend_delete_directory)
- All 9 tests passing 

## Acceptance Criteria

- [x] DeleteStrategy trait defined with execute() method
- [x] LocalDeleteStrategy implements existing delete logic
- [x] RemoteDeleteStrategy sends requests via networking service
- [x] DeleteStrategyRouter selects strategy based on path locations
- [x] DeleteJob refactored to use strategy pattern
- [x] Network protocol handler for remote deletion (handler to receive requests)
- [x] Integration test: Delete local files via LocalStrategy
- [x] Integration test: Strategy routing works correctly
- [x] All delete modes tested (Permanent, Trash, Secure)
- [x] VolumeBackend delete() method implemented for cloud storage
- [x] LocalDeleteStrategy supports cloud paths via VolumeBackend
- [x] Integration tests for cloud deletion (file and directory)

## Technical Notes

**Why Strategy Pattern?**

- Consistent with FileCopyJob architecture (CopyStrategy pattern)
- Separates concerns: routing logic vs. deletion logic
- Supports heterogeneous storage via VolumeBackend integration
- LocalDeleteStrategy handles both local filesystem and cloud paths
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
