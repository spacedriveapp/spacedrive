---
id: LSYNC-000
title: "Epic: Library-based Synchronization (Leaderless)"
status: Done
assignee: james
priority: High
tags: [epic, sync, networking, library-sync, leaderless]
whitepaper: Section 4.5.1
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-12-02
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

**Completed (Phase 2)** - Oct 9, 2025:
- LSYNC-006: TransactionManager ✅
- LSYNC-007: Syncable trait ✅
- LSYNC-009: HLC implementation ✅
- LSYNC-013: Hybrid protocol handler ✅

**Completed (Phase 3)** - Oct 15, 2025:
- LSYNC-010: Peer sync service ✅
- LSYNC-011: Conflict resolution (HLC ordering) ✅
- LSYNC-002: Metadata sync ✅
- Model implementations:
  - Device-owned: Device ✅, Location ✅, Entry ✅, Volume ✅
  - Shared: Tag ✅, Collection ✅, ContentIdentity ✅, UserMetadata ✅

**Upcoming (Phase 4)**:
- Enhanced integration testing for all 8 models
- Backfill optimization for new devices joining
- Retry queue for failed sync operations
- Performance optimization and monitoring

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
- Locations, Entries, Volumes, Devices
- Each device broadcasts its own state
- Peers apply (no conflicts possible)
- Efficient: just timestamp-based delta sync

**Shared Resources** (small log, HLC-based):
- Tags, Collections, ContentIdentity, UserMetadata
- Each device logs its shared changes
- Broadcast with HLC for ordering
- Peers ACK → aggressive pruning → log stays tiny

**Benefits**:
- No leader bottleneck
- Works fully offline
- Simpler (~800 lines less code)
- More resilient
- Aligns with architecture

## Implementation Summary (Oct 2025)

**Total Models Syncing**: 8
- 4 device-owned (state-based)
- 4 shared (HLC log-based)

**Infrastructure Complete**:
- Syncable trait with FK mapping
- PeerLog (sync.db per device)
- HLC implementation
- Conflict resolution (LWW)
- Registry for dynamic dispatch
- Integration tests (10 passing)

**Key Features**:
- HLC conflict resolution prevents stale overwrites
- Deterministic UUIDs for ContentIdentity enable dedup
- Per-device sync.db stays small via ACK pruning
- FK mapper translates integer IDs to UUIDs transparently
