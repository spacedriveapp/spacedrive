---
id: CORE-002
title: Implement Universal SdPath Addressing
status: Done
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, addressing]
whitepaper: Section 4.1.4
---

## Description

Implement `SdPath`, the universal addressing system for the VDFS. This enum provides a core abstraction that makes device boundaries transparent, supporting both direct physical paths (`device_id` + `path`) and abstract content-aware paths.

## Implementation Notes
-   The `SdPath` enum and its methods are fully defined in `src/domain/addressing.rs`.
-   The system includes helper methods like `.is_local()`, `.as_local_path()`, and `.display()` for ergonomic use.
-   URI parsing (`SdPath::from_uri`) will be implemented to handle `sd://` schemes for both physical and content paths.

## Acceptance Criteria
-   [x] `SdPath` enum can represent both physical and content-based addresses.
-   [x] The system can differentiate between local and remote physical paths.
-   [x] URI strings can be reliably parsed into `SdPath` objects.
