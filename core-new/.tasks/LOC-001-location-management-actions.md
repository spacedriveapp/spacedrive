---
id: LOC-001
title: Implement Location Management Actions
status: Done
assignee: james
parent: LOC-000
priority: High
tags: [core, actions, locations, indexing]
whitepaper: Section 4.3.3
---

## Description

The core actions for managing library locations have been implemented. This includes adding a new directory to be indexed, removing it, and triggering a rescan.

## Implementation Notes
-   **Add:** The `LocationAddAction` (`src/operations/locations/add/action.rs`) creates a location record and dispatches an initial `IndexerJob`.
-   **Remove:** The `LocationRemoveAction` (`src/operations/locations/remove/action.rs`) removes the location and all its associated entries from the database.
-   **Rescan:** The `LocationRescanAction` (`src/operations/locations/rescan/action.rs`) dispatches a new `IndexerJob` for an existing location.

## Acceptance Criteria
-   [x] A user can add a new local directory as a location.
-   [x] Adding a location automatically starts an indexing job.
-   [x] A user can remove a location, cleaning up its database entries.
-   [x] A user can trigger a manual rescan of an existing location.
