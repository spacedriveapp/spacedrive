---
id: VSS-003
title: "Reference Sidecars for Live Photo Support"
status: Done
assignee: james
parent: VSS-000
priority: Medium
tags: [vss, feature, photos, indexing, deprecated]
whitepaper: "Section 4.1.5"
last_updated: 2025-11-01
---

## Status Update (Nov 1, 2025)

**Live Photo detection and pairing has been moved to the Photos extension** as part of the domain-separated architecture. The reference sidecar infrastructure remains in core and is complete.

The core reference sidecar pattern is implemented and ready for use by extensions:
- `source_entry_id` column exists in `sidecars` table
- `create_reference_sidecar()` method implemented
- `convert_reference_to_owned()` method implemented
- Database schema supports reference tracking

Live Photo detection will be reimplemented in the Photos extension using these core primitives.

## Original Description

Implement the "Reference Sidecar" feature as described in **REFERENCE_SIDECARS.md**. This allows Spacedrive to track pre-existing files (like the video component of a Live Photo) as virtual sidecars without moving them from their original locations.

## Implementation Notes

- Added `source_entry_id: Option<i32>` column to `sidecars` table
- Implemented `SidecarManager::create_reference_sidecar()`
- Implemented `SidecarManager::convert_reference_to_owned()`
- Live Photo detection moved to Photos extension (will use these primitives)

## Acceptance Criteria

- [x] Reference sidecar infrastructure exists in core
- [x] Can create sidecar records that reference existing entries
- [x] Files are NOT moved when creating reference sidecars
- [x] Can convert reference sidecars to managed sidecars
- [ ] Live Photo detection implemented in Photos extension (separate task)

## Migration Notes

Live Photo functionality will be tracked in Photos extension tasks. Core reference sidecar support is complete and available for any extension to use.
