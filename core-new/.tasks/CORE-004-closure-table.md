---
id: CORE-004
title: Implement Hierarchical Indexing with Closure Table
status: Done
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, database, performance]
whitepaper: Section 4.3.4
---

## Description

To ensure high-performance hierarchical queries (e.g., finding all files in a directory subtree), a Closure Table pattern will be implemented in the database. This pre-calculates all ancestor-descendant relationships.

## Implementation Notes
-   The schema will be defined in `src/infrastructure/database/entities/entry_closure.rs`.
-   Query helpers for traversing the hierarchy (e.g., `get_descendants`, `get_ancestors`) will be implemented in `src/operations/indexing/hierarchy.rs`.
-   The indexing process in `src/operations/indexing/phases/processing.rs` correctly populates the closure table upon entry creation.

## Acceptance Criteria
-   [x] The database contains an `entry_closure` table.
-   [x] Creating a new file entry correctly populates its ancestor relationships.
-   [x] Subtree and ancestor queries are efficient and do not require recursive SQL.
