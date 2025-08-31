---
id: SYNC-002
title: "Develop the Sync Engine Service"
status: To Do
assignee: unassigned
parent: SYNC-000
priority: High
tags: [sync, core, services]
whitepaper: Section 4.5.4
---

## Description

Implement the `SyncEngine`, a background service responsible for orchestrating all synchronization tasks. The engine's main loop will periodically evaluate all active `SyncRelationship`s and trigger sync operations.

## Implementation Steps

1.  Create a new service in `src/services/sync_engine.rs`.
2.  Implement the main loop that loads all active `SyncRelationship`s from the database.
3.  For each relationship, trigger the VDFS index diffing logic (from `SYNC-003`).
4.  Based on the diff, generate a `SyncAction` (from `SYNC-004`).
5.  If the relationship's policy is `Automatic`, commit the action directly to the `JobManager`.
6.  If the policy is `Manual`, store the generated action preview for user review.
7.  Subscribe to `LocationWatcher` events to enable real-time re-evaluation for `Automatic` sync relationships.

## Acceptance Criteria
-   [ ] The `SyncEngine` service can be started and stopped gracefully.
-   [ ] The service correctly identifies active sync relationships.
-   [ ] The service correctly triggers actions based on the relationship's policy.
