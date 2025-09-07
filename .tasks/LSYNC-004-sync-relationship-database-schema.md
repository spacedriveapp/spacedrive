---
id: LSYNC-004
title: SyncRelationship Database Schema
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, database, schema]
whitepaper: Section 4.5.4
---

## Description

The database entity and migration for the `SyncRelationship` have been implemented. This is a first-class entity that durably stores the configuration for a sync operation between two locations, forming the foundation of the entire sync engine.

## Implementation Notes
- The `sync_relationships` entity, which defines the relationship between a source and target location, is fully defined in the code.
- The schema includes fields for sync strategy, direction, and schedule.
- The corresponding database migration to create this table also exists.

## Acceptance Criteria
-   [x] A `sync_relationships` table exists in the database schema.
-   [x] The SeaORM entity for `SyncRelationship` is created.
-   [x] Foreign key constraints to the `locations` table are correctly established.
