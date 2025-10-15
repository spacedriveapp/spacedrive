---
id: FILE-003
title: Cloud Volume File Operations
status: To Do
assignee: james
parent: FILE-000
priority: High
tags: [core, file-ops, cloud, jobs]
whitepaper: Section 4.4.6
related_tasks: [FILE-001, CLOUD-003, VOL-004]
last_updated: 2025-10-14
---

## Description

Extend the file copy job system to support cloud volumes, enabling users to copy files to/from cloud storage (S3, R2, etc.) using the existing FileCopyJob infrastructure. This task builds on FILE-001's strategy pattern and CLOUD-003's VolumeBackend abstraction.

## Implementation Steps

1. Create `CloudCopyStrategy` that uses `VolumeBackend` for I/O
   - Read from cloud using `backend.read()` or `backend.read_range()`
   - Write to cloud using `backend.write()`
   - Support streaming with progress tracking
   - Handle chunked transfers for large files

2. Update `CopyStrategyRouter` to detect and route cloud paths
   - Detect `SdPath::Cloud` variant
   - Route local-to-cloud, cloud-to-local, and cloud-to-cloud transfers
   - Consider cloud backend capabilities (streaming, resumption)

3. Implement cloud-aware strategy selection logic
   - Cloud-to-cloud on same volume: use cloud-native copy if available
   - Cloud-to-local: stream download with progress
   - Local-to-cloud: stream upload with progress
   - Cloud-to-cloud across volumes: orchestrate via local buffer or direct transfer

4. Add progress tracking for cloud transfers
   - Report bytes uploaded/downloaded
   - Handle network interruptions gracefully
   - Support resume for interrupted transfers (if backend supports)

5. Integrate with existing `FileCopyJob` and `CopyAction`
   - Ensure `CopyAction` accepts cloud `SdPath` inputs
   - Update validation to allow cloud paths
   - Test end-to-end with CLI and future UI

## Acceptance Criteria

- [ ] User can copy a local file to a cloud volume
- [ ] User can copy a file from a cloud volume to local storage
- [ ] User can copy files between two different cloud volumes
- [ ] Progress is accurately reported for cloud transfers
- [ ] Transfers can be cancelled mid-operation
- [ ] Checksum verification works for cloud transfers

## Implementation Files

**Strategy Implementation:**

- `core/src/ops/files/copy/strategy.rs` - Add `CloudCopyStrategy`

**Routing Logic:**

- `core/src/ops/files/copy/routing.rs` - Update `CopyStrategyRouter::select_strategy()`

**Volume Backend:**

- `core/src/volume/backend/cloud.rs` - Already provides `read()`, `write()`, `read_range()`

**Testing:**

- `core/tests/test_cloud_file_ops.rs` - Integration tests for cloud file operations

## Technical Notes

- Cloud transfers should use `backend.read_range()` for efficient chunked streaming
- Consider rate limiting for cloud API calls to avoid throttling
- Error handling should distinguish between network errors and cloud service errors
- For cloud-to-cloud on same backend, investigate if OpenDAL supports native copy operations

## Next Steps

1. Implement `CloudCopyStrategy` with basic read/write operations
2. Update router to detect cloud paths and select cloud strategy
3. Add integration tests with MinIO or LocalStack
4. Benchmark performance and optimize chunk sizes
5. Add support for cloud-native copy operations where available
