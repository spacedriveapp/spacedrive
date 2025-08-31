---
id: JOB-001
title: Implement Job Manager for Task Scheduling
status: Done
assignee: james
parent: JOB-000
priority: High
tags: [core, jobs]
whitepaper: Section 4.4
---

## Description

A `JobManager` has been implemented for each library to schedule, execute, and monitor background tasks. It is built on top of a generic `TaskSystem` for concurrency management.

## Implementation Notes
-   The `JobManager` is defined in `src/infrastructure/jobs/manager.rs`.
-   It maintains its own private database (`jobs.db`) for storing job state, history, and checkpoints.
-   It is responsible for resuming interrupted jobs on startup.
-   It provides APIs to `dispatch`, `pause`, and `resume` jobs.

## Acceptance Criteria
-   [x] Each library has its own `JobManager` instance.
-   [x] The manager can dispatch a new job and return a `JobHandle`.
-   [x] The manager can list jobs by status by querying both memory and its database.
-   [x] Interrupted jobs (e.g., from a crash) are correctly paused and can be resumed on next startup.
