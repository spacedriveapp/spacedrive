---
id: LSYNC-000
title: "Epic: Library-based Synchronization (Leaderless)"
status: In Progress
assignee: james
priority: High
tags: [epic, sync, networking, library-sync, leaderless]
whitepaper: Section 4.5.1
design_doc: core/src/infra/sync/NEW_SYNC.md
---

## Description

Implement library metadata synchronization using a **leaderless hybrid model**:
- **State-based sync** for device-owned data (locations, entries, volumes)
- **Log-based sync with HLC** for shared resources (tags, albums, metadata)

This eliminates leader bottlenecks while maintaining conflict resolution where needed.

## Architecture Revision (Oct 2025)

**Key Insight**: Device ownership eliminates 90% of conflicts. Only truly shared resources need ordered logs.

**Before**: Single leader assigns sequences for all changes
**After**: Each device broadcasts its own changes, HLC orders shared resources

See `core/src/infra/sync/NEW_SYNC.md` for complete rationale.

## Current Status

**Completed (Phase 1)**:
- NET-001: Iroh P2P stack ✅
- NET-002: Device pairing protocol ✅
- LSYNC-003: Library sync setup ✅

**In Progress (Phase 2)**:
- LSYNC-006: TransactionManager (simplified, no leader checks)
- LSYNC-007: Syncable trait (with device_id field)
- LSYNC-009: HLC implementation (replaces leader election)

**Upcoming (Phase 3)**:
- LSYNC-013: Hybrid protocol handler
- LSYNC-010: Peer sync service
- LSYNC-011: Conflict resolution (HLC-based)
- LSYNC-002: Metadata sync

**Cancelled/Obsolete**:
- ~~LSYNC-008: Central sync log~~ (replaced with per-device shared_changes)
- ~~Leader election~~ (no leader needed)

## Subtasks

### Phase 1: Foundation ✅
- LSYNC-001: Protocol design
- LSYNC-003: Sync setup

### Phase 2: Core Infrastructure (Revised)
- LSYNC-006: TransactionManager (no leader checks)
- LSYNC-007: Syncable trait (device ownership)
- LSYNC-009: HLC implementation

### Phase 3: Sync Services
- LSYNC-013: Hybrid protocol handler
- LSYNC-010: Peer sync service
- LSYNC-011: Conflict resolution

### Phase 4: Application
- LSYNC-002: Metadata sync (tags/albums)
- Entry sync optimization

## Architecture Summary

**Device-Owned Data** (no log, state-based):
- Locations, Entries, Volumes, Audit Logs
- Each device broadcasts its own state
- Peers apply (no conflicts possible)
- Efficient: just timestamp-based delta sync

**Shared Resources** (small log, HLC-based):
- Tags, Albums, UserMetadata (on content)
- Each device logs its shared changes
- Broadcast with HLC for ordering
- Peers ACK → aggressive pruning → log stays tiny

**Benefits**:
- ✅ No leader bottleneck
- ✅ Works fully offline
- ✅ Simpler (~800 lines less code)
- ✅ More resilient
- ✅ Aligns with architecture
