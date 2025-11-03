---
id: CORE-008
title: Virtual Sidecar System (VSS)
status: In Progress
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, sidecars, derivatives, addressing]
whitepaper: Section 4.1.5
last_updated: 2025-11-01
related_tasks: [CORE-002, VSS-001, VSS-002]
---

## Description

Implement the Virtual Sidecar System (VSS) for managing derivative data—thumbnails, OCR text, video transcripts, embeddings, and agent-generated intelligence—as first-class addressable files within the VDFS.

**V2 Design (Nov 2025):** Sidecars integrated as native `SdPath::Sidecar` variant, enabling unified addressing, standard file operations, and cross-device semantics. See `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md` for complete specification.

## Current Implementation Status

### Completed (Infrastructure)

1. Database schema (`sidecars` and `sidecar_availability` tables)
2. `SidecarManager` service with core CRUD operations
3. Deterministic path computation with 2-level hex sharding
4. Presence queries (local + remote device availability)
5. Reference sidecar support (track without moving files)
6. Bootstrap scan for filesystem reconciliation
7. Get-or-enqueue pattern for lazy materialization

### Not Implemented (Integration)

1. `SdPath::Sidecar` variant and URI parsing
2. Resolution integration with SdPathResolver
3. Job system dispatch (TODO at line 273)
4. Filesystem watcher for sidecars directory (TODO at line 708)
5. Checksum computation (TODO at line 696)
6. Cross-device sidecar transfer protocol
7. Actual generation jobs (thumbnails, OCR, transcripts)
8. CLI sidecar commands

## Implementation Steps

### Phase 1: SdPath Integration
- [ ] Add `SdPath::Sidecar { content_id, kind, variant, format }` enum variant
- [ ] Implement `sidecar://` URI parsing
- [ ] Add display formatting for sidecar URIs
- [ ] Write unit tests for parsing/display

### Phase 2: Resolution
- [ ] Implement `resolve_sidecar()` in SdPathResolver
- [ ] Add resolution mode support (blocking, async, fetch-only)
- [ ] Integrate with existing SidecarManager
- [ ] Handle pending/missing sidecars gracefully

### Phase 3: Operations
- [ ] Add sidecar support to ReadAction
- [ ] Add sidecar support to FileCopyAction
- [ ] Implement restricted DeleteAction for sidecars
- [ ] Implement ListAction for sidecar directories
- [ ] Add operation validation (prevent move/rename)

### Phase 4: Job System
- [ ] Implement ThumbnailGenerationJob
- [ ] Implement OcrExtractionJob
- [ ] Implement TranscriptGenerationJob
- [ ] Hook into indexing pipeline (Intelligence Queueing Phase)
- [ ] Implement job dispatch in SidecarManager

### Phase 5: Cross-Device Sync
- [ ] Implement availability digest exchange
- [ ] Implement sidecar transfer protocol
- [ ] Add sync scheduler for periodic updates
- [ ] Implement prefetch policies

### Phase 6: CLI & SDK
- [ ] Add `sd sidecars` command family
- [ ] Implement sidecar glob patterns
- [ ] Add SDK APIs for extensions
- [ ] Document patterns and examples

## Acceptance Criteria

### Core Functionality
- [ ] Thumbnails auto-generated for images during indexing
- [ ] OCR text extracted from documents automatically
- [ ] Sidecars addressable via `sidecar://` URIs
- [ ] Can copy sidecars to physical locations
- [ ] Can list all sidecars for a content item

### Cross-Device
- [ ] Devices exchange sidecar availability information
- [ ] Missing sidecars can be fetched from remote devices
- [ ] Sidecar transfers reuse P2P file transfer infrastructure
- [ ] Availability tracking stays current across library

### Integration
- [ ] Extensions can read/write sidecars via SDK
- [ ] CLI supports sidecar operations
- [ ] Actions support sidecar paths
- [ ] Resolver handles all sidecar resolution modes

### Quality
- [ ] Deterministic paths work without DB queries
- [ ] Idempotent generation (checks before regenerating)
- [ ] Reference sidecars can be converted to managed
- [ ] Cleanup policies prevent unbounded growth

## Design Documentation

Primary spec: `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md` (Nov 2025)

Supporting docs:
- `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM.md` (Original spec)
- `workbench/core/storage/REFERENCE_SIDECARS.md` (Reference pattern)
- `workbench/core/storage/SIDECAR_SCALING_DESIGN.md` (Future scaling)
- `docs/core/virtual-sidecars.mdx` (User documentation)
