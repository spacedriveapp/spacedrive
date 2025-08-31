---
id: CORE-001
title: Implement Entry-Centric Data Model
status: Done
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, database, model]
whitepaper: Section 4.1.2
---

## Description

Implemented the universal `Entry` data model, which represents any filesystem item (file, directory, symlink). Every `Entry` is designed for immediate metadata capability via a linked `UserMetadata` record, allowing users to tag and organize files the moment they are discovered.

## Implementation Notes
-   The core domain model is defined in `src/domain/entry.rs`.
-   The corresponding database entity is implemented in `src/infrastructure/database/entities/entry.rs`.
-   The `EntryKind` enum correctly differentiates between `File`, `Directory`, and `Symlink`.

## Acceptance Criteria
-   [x] `Entry` struct exists and contains fields for `metadata_id` and `content_id`.
-   [x] The system can represent files and directories using a unified model.
-   [x] The database schema correctly reflects the `Entry` model.
