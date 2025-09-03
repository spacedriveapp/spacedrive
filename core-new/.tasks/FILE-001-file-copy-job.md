---
id: FILE-001
title: File Copy Job with Strategy Pattern
status: Done
assignee: james
parent: FILE-000
priority: High
tags: [core, jobs, file-ops, vdfs]
whitepaper: Section 4.4.6
---

## Description

A flexible `FileCopyJob` will be implemented to handle all copy and move operations. It uses a strategy pattern to select the optimal file transfer method (e.g., local move, cross-volume stream, remote transfer) based on the source and destination `SdPath`.

## Implementation Notes

- The `FileCopyJob` and `CopyOptions` will be defined in `src/operations/files/copy/job.rs`.
- The `CopyStrategyRouter` in `src/operations/files/copy/routing.rs` selects the appropriate strategy.
- Implement strategies include `LocalMoveStrategy`, `LocalStreamCopyStrategy`, and `RemoteTransferStrategy` in `src/operations/files/copy/strategy.rs`.
- The job provides detailed, byte-level progress updates.

## Acceptance Criteria

- [x] The job can copy files and directories locally.
- [x] The job correctly selects an atomic `rename` for same-volume moves.
- [x] The job can orchestrate a cross-device transfer between two peers.
- [x] The job correctly handles `delete_after_copy` for move operations.
