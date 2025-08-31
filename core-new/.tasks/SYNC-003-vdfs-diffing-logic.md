---
id: SYNC-003
title: "Implement VDFS Index Diffing Logic"
status: To Do
assignee: unassigned
parent: SYNC-000
priority: High
tags: [sync, vdfs, core-logic]
whitepaper: Section 4.5.4
---

## Description

Create the core "diffing" logic that compares the state of two locations using only the VDFS index. This function is the brain of the sync engine, determining the exact set of operations (copy, move, delete) required to bring a target location in sync with a source.

## Implementation Steps

1.  Create a new module, e.g., `src/operations/sync/diffing.rs`.
2.  The main function should accept a `source_location_id` and a `target_location_id`.
3.  Query the `entry` and `content_identity` tables for all descendants of both locations.
4.  Compare the two sets of entries based on relative paths and content hashes.
5.  Produce a list of primitive operations (e.g., `CreateDir`, `CopyFile`, `DeleteFile`) needed to reconcile the target with the source.

## Acceptance Criteria
-   [ ] Given two identical locations, the diffing logic produces an empty set of operations.
-   [ ] When a file is added to the source, the diff produces a `CopyFile` operation.
-   [ ] When a file is removed from the source, the diff produces a `DeleteFile` operation for the target.
