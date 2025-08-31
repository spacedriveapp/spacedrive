---
id: CORE-005
title: Implement Advanced File Type System
status: Done
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, file-types]
whitepaper: Section 4.1.7
---

## Description

A sophisticated, multi-method file type identification system will be implemented. It goes beyond simple extension matching to provide accurate, content-aware identification and semantic categorization for a wide range of file types.

## Implementation Notes
-   The core logic resides in `src/file_type/registry.rs`.
-   The system combines **extension matching**, **magic byte detection**, and **content analysis** with a priority-based system to resolve conflicts (e.g., `.ts` files).
-   File types will be defined in declarative **TOML files** (`src/file_type/definitions/`), making the system highly extensible.
-   Files are grouped into 17 semantic `ContentKind` categories, enabling intuitive filtering and specialized handling.

## Acceptance Criteria
-   [x] The `FileTypeRegistry` can load definitions from TOML files.
-   [x] The system correctly identifies files using both extensions and magic byte patterns.
-   [x] The system can handle extension conflicts using a priority score.
-   [x] Files are correctly assigned a `ContentKind`.
