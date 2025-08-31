---
id: SYNC-001
title: "Implement SyncRelationship Database Schema"
status: To Do
assignee: unassigned
parent: SYNC-000
priority: High
tags: [sync, database, schema]
whitepaper: Section 4.5.4
---

## Description

Create the database entity and migration for the `SyncRelationship`. This is a first-class entity that durably stores the configuration for a sync operation between two locations, forming the foundation of the entire sync engine.

## Implementation Steps

1.  Create a new database migration to add the `sync_relationships` table.
2.  Define the `SyncRelationship` entity in `src/infrastructure/database/entities/`.
3.  The schema must include columns for:
    -   `source_location_id` (Foreign Key to `locations`)
    -   `target_location_id` (Foreign Key to `locations`)
    -   `mode` (Enum: `OneWay`, `TwoWay`)
    -   `policy` (Enum: `Manual`, `Automatic`)
    -   `status` (Enum: `Active`, `Paused`, `Error`)

## Acceptance Criteria
-   [ ] A new `sync_relationships` table exists in the database schema.
-   [ ] The SeaORM entity for `SyncRelationship` is created.
-   [ ] Foreign key constraints to the `locations` table are correctly established.
