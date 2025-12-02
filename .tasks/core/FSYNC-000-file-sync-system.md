---
id: FSYNC-000
title: File Sync System (Epic)
status: In Progress
assignee: james
parent: null
priority: High
tags: [sync, service, epic, index-driven]
whitepaper: Section 5.2
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
documentation: docs/core/file-sync.mdx
last_updated: 2025-10-15
---

## Description

Implement File Sync system - an index-driven service that orchestrates content synchronization between locations. File Sync operates entirely through VDFS index queries, transforming sync resolution from filesystem scanning into efficient database operations.

**Architecture:** Service-based orchestrator that dispatches FileCopyJob and DeleteJob to perform operations.

## Core Design Principles

1. **Index-Driven**: All sync decisions based on VDFS index queries
2. **Push-Based**: Device with files initiates transfers
3. **Library Sync First**: Requires complete metadata sync before operating
4. **Service as Orchestrator**: FileSyncService dispatches jobs, doesn't execute operations directly
5. **Leverage Existing Jobs**: Reuse FileCopyJob and DeleteJob with their routing/strategy infrastructure

## Architecture Decision: Service vs Job

**Why FileSyncService (not FileSyncJob):**

- Jobs cannot spawn child jobs in Spacedrive's architecture
- FileSyncJob would duplicate FileCopyJob's complex routing logic
- Bidirectional sync needs persistent state management beyond job lifecycle
- Service orchestrates, jobs execute operations
- Proper separation of concerns: sync logic vs. file operations

## Sync Modes

### Mirror Mode (MVP)

One-way sync: source → target. Creates exact copy with automatic cleanup.

### Bidirectional Mode

Two-way sync with conflict detection and resolution. Changes flow both directions.

### Selective Mode (Future)

Intelligent local storage management with access pattern tracking.

**Note:** Archive mode removed from design - users can achieve this with FileCopyJob + delete.

## Child Tasks

- **FSYNC-001**: DeleteJob Strategy Pattern & Remote Deletion (Phase 1) - Done
- **FSYNC-002**: Database Schema & Entities (Phase 2) - Done
- **FSYNC-003**: FileSyncService Core Implementation (Phase 3)
- **FSYNC-004**: Service Integration & API (Phase 4)
- **FSYNC-005**: Advanced Features (Phase 5)

## Key Benefits

**No Code Duplication** - Reuses FileCopyJob routing, strategies, VolumeManager infrastructure
**Proper Separation** - Service orchestrates, jobs execute
**Testable** - Sync logic independent from file operations
**Extensible** - Easy to add new sync modes
**Consistent** - Uses same code paths as manual operations

## Success Metrics

- Reliability: 99.9%+ sync success rate
- Performance: Matches FileCopyJob performance (100MB/s+ local)
- Correctness: No data loss, accurate conflict detection
- Resource Efficiency: Low CPU/memory overhead for monitoring

## References

- Design: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md (2255 lines)
- Docs: docs/core/file-sync.mdx
- Related: FILE-001 (File Copy Job), LSYNC-000 (Library Sync)
