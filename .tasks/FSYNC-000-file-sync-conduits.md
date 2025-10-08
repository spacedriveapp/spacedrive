---
id: FSYNC-000
title: "Epic: File Sync Conduits"
status: In Progress
assignee: james
priority: High
tags: [epic, file-sync, sync-conduits, storage]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

This epic covers the implementation of **Sync Conduits**, a system for synchronizing file content between user-defined points within the Spacedrive VDFS. This is **distinct from Library Sync** (LSYNC-000), which handles metadata replication across devices.

Sync Conduits provide users with explicit, transparent, and configurable control over how physical file content is mirrored, backed up, or managed across different storage locations.

## Key Distinction

- **Library Sync (LSYNC-000)**: Metadata replication across devices (tags, albums, entries)
- **File Sync Conduits (FSYNC-000)**: File content synchronization between storage locations

## Architecture

### Core Concepts

1. **Sync Conduit**: A durable, long-running job representing a user-configured sync relationship between source and destination Entries
2. **State-Based Reconciliation**: Periodic comparison of filesystem state against VDFS index (not event replay)
3. **Four Sync Policies**:
   - **Replicate**: One-way mirror (backups)
   - **Synchronize**: Two-way sync (multi-device workflows)
   - **Offload**: Smart cache (free up space)
   - **Archive**: Move and consolidate (long-term storage)

### Sync Lifecycle

1. **Trigger**: LocationWatcher or timer (Sync Cadence)
2. **Delta Calculation**: Live scan + VDFS cache
3. **Execution**: Dispatch COPY/DELETE actions
4. **Verification**: Commit-Then-Verify (BLAKE3 hash)
5. **Completion**: All actions verified

## Subtasks

### Phase 1: Foundation
- [x] FSYNC-001: Database schema
- [ ] FSYNC-002: SeaORM entity & ActiveModel
- [ ] FSYNC-003: Job definition (SyncConduitJob)
- [ ] FSYNC-004: Actions (create/update/delete conduits)
- [ ] FSYNC-005: Input/Output types

### Phase 2: Core Infrastructure
- [ ] FSYNC-006: Networking protocol (ValidationRequest/Response)
- [ ] FSYNC-007: FileCopyJob modifications (Verifying state)
- [ ] FSYNC-008: LocationWatcher integration
- [ ] FSYNC-009: State-based reconciliation engine
- [ ] FSYNC-010: Commit-Then-Verify (CTV) implementation

### Phase 3: Sync Policies
- [ ] FSYNC-011: Replicate policy (one-way mirror)
- [ ] FSYNC-012: Synchronize policy (two-way sync)
- [ ] FSYNC-013: Offload policy (smart cache)
- [ ] FSYNC-014: Archive policy (move & consolidate)

### Phase 4: User Experience
- [ ] FSYNC-015: UI components (Sync Status panel)
- [ ] FSYNC-016: "Sync To..." action dialog
- [ ] FSYNC-017: Conduit management UI

## References

- Design: `docs/core/design/sync/SYNC_CONDUIT_DESIGN.md`
- Library Sync: LSYNC-000 (separate system)

