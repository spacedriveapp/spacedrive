---
id: SYNC-005
title: "Implement CLI Commands for Sync Management"
status: To Do
assignee: unassigned
parent: SYNC-000
priority: Medium
tags: [sync, cli]
whitepaper: Section 4.5.4
---

## Description

Provide a user interface through the CLI for creating, managing, and monitoring `SyncRelationship`s.

## Implementation Steps

1.  Add a `sync` subcommand to the CLI (`src/infrastructure/cli/commands/`).
2.  Implement `spacedrive sync create <source_loc> <target_loc> [--mode=one-way] [--policy=auto]`. This command will create a new `SyncRelationship` record in the database.
3.  Implement `spacedrive sync list` to display all configured relationships and their current status.
4.  Implement `spacedrive sync pause <id>` and `spacedrive sync resume <id>` to control the status of a relationship.
5.  Implement `spacedrive sync review` for manual policies, which will list pending sync actions and allow the user to approve them.

## Acceptance Criteria
-   [ ] A user can create a new sync relationship between two locations via the CLI.
-   [ ] A user can list all active sync relationships.
-   [ ] A user can pause and resume a relationship, which correctly stops and restarts the `SyncEngine`'s evaluation for that pair.
