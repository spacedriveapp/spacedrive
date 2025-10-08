---
id: LSYNC-000
title: "Epic: Library-based Synchronization"
status: In Progress
assignee: james
priority: High
tags: [epic, sync, networking, library-sync]
whitepaper: Section 4.5.1
---

## Description

This epic covers the implementation of the "Library Sync" system, enabling real-time, multi-device synchronization of library metadata. The architecture consists of three pillars: TransactionManager (write gatekeeper), Sync Log (append-only change log), and Sync Service (pull-based replication).

## Current Status

**Completed (Phase 1)**:
- NET-001: Iroh P2P stack
- NET-002: Device pairing protocol
- LSYNC-004: SyncRelationship schema
- LSYNC-005: Library sync setup (device discovery & registration)
- LSYNC-001: Protocol design (documented in `docs/core/`)

**In Progress (Phase 2)**:
- LSYNC-006: TransactionManager core
- LSYNC-007: Syncable trait & derives
- LSYNC-008: Sync log schema
- LSYNC-009: Leader election

**Upcoming (Phase 3)**:
- LSYNC-013: Sync protocol handler (message-based)
- LSYNC-010: Sync service (leader & follower)
- LSYNC-011: Conflict resolution
- LSYNC-002: Metadata sync (albums/tags)
- LSYNC-012: Entry sync (bulk optimization)

## Architecture

**Message-based Sync**: Push notifications via dedicated sync protocol instead of polling for better performance, lower latency, and battery efficiency.

See `docs/core/sync.md` for complete specification.

## Subtasks

### Phase 1: Foundation (Completed)
- LSYNC-001: Protocol design ✅
- LSYNC-003: Sync setup ✅
- LSYNC-004: Database schema ✅

### Phase 2: Core Infrastructure (In Progress)
- LSYNC-006: TransactionManager
- LSYNC-007: Syncable trait
- LSYNC-008: Sync log schema (separate DB)
- LSYNC-009: Leader election

### Phase 3: Sync Services (Next)
- LSYNC-013: Sync protocol handler (push-based)
- LSYNC-010: Sync service (leader & follower)
- LSYNC-011: Conflict resolution

### Phase 4: Application (After Phase 3)
- LSYNC-002: Metadata sync (albums/tags)
- LSYNC-012: Entry sync (bulk optimization)

### Future
- LSYNC-003: File operations (sync conduits)
