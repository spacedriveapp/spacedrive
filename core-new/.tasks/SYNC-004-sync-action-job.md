---
id: SYNC-004
title: "Implement SyncAction and SyncJob"
status: To Do
assignee: unassigned
parent: SYNC-000
priority: High
tags: [sync, actions, jobs]
whitepaper: Section 4.5.4
---

## Description

Implement the `SyncAction` and `SyncJob` to make synchronization a first-class operation within the Action and Job systems. The `SyncAction` will encapsulate the list of operations produced by the diffing engine, and the `SyncJob` will execute them.

## Implementation Steps

1.  Define a `SyncAction` within the `Action` enum. It should contain the list of primitive operations from the diffing logic.
2.  Create a `SyncActionHandler` that validates the sync plan.
3.  Define a `SyncJob` that takes the list of operations as its state.
4.  The `SyncJob::run` method will iterate through the operations and dispatch them as smaller, atomic jobs (e.g., `FileCopyJob`, `DeleteJob`).
5.  The `SyncJob` must provide detailed progress updates as it executes the plan.

## Acceptance Criteria
-   [ ] A `SyncAction` can be dispatched to the `ActionManager`.
-   [ ] The `SyncJob` correctly executes the plan of copy and delete operations.
-   [ ] The job can be paused and resumed.
