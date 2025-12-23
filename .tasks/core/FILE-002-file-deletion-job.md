---
id: FILE-002
title: File Deletion Job
status: Done
assignee: jamiepine
parent: FILE-000
priority: High
tags: [core, jobs, file-ops]
whitepaper: Section 4.4
---

## Description

A job for handling file and directory deletion. This job will be orchestrated by the Action System and provides different modes for deletion, ensuring safe and reliable file removal.

## Implementation Notes

- The `DeleteJob` will be defined in `src/operations/files/delete/job.rs`.
- It supports multiple deletion modes, including `Trash` and `Permanent` deletion.
- The implementation includes platform-specific logic for finding the correct trash directory on Unix, macOS, and Windows.
- The job is resumable, tracking completed deletions to ensure it can recover from interruptions.

## Acceptance Criteria

- [x] The `FileDeleteAction` correctly dispatches a `DeleteJob`.
- [x] The job can move files to the system's trash location.
- [x] The job supports permanent file deletion.
