---
id: FSYNC-001
title: SyncRelationship Database Schema (File Sync Conduits)
status: Done
assignee: james
parent: FSYNC-000
priority: High
tags: [file-sync, sync-conduits, database, schema]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

The database entity and migration for the `SyncRelationship` (Sync Conduits) have been implemented. This is a first-class entity that durably stores the configuration for a sync conduit between two entries/locations, enabling file content synchronization as described in the Sync Conduits design.

**Note**: This is for **File Sync (Sync Conduits)**, NOT Library Sync. Library Sync handles metadata replication across devices, while Sync Conduits handle actual file content synchronization between storage locations.

## Implementation Notes
- The `sync_relationships` entity, which defines the relationship between a source and destination entry, is fully defined in the code.
- The schema includes fields for sync policy (Replicate, Synchronize, Offload, Archive), policy configuration, status, and cadence.
- The corresponding database migration to create this table also exists.

## Acceptance Criteria
-   [x] A `sync_relationships` table exists in the database schema.
-   [x] The SeaORM entity for `SyncRelationship` is created.
-   [x] Foreign key constraints to the `entry` table are correctly established.

