---
id: LSYNC-001
title: Design Library Sync Protocol
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, networking, protocol, design]
whitepaper: Section 4.5.1
---

## Description

Design the detailed protocol for Library Sync. This includes defining the communication flow between peers, the format of the messages, and the logic for initiating and managing a sync session.

**Update**: The sync protocol has been fully designed and documented as the "Three Pillars" architecture:
1. TransactionManager (sole gatekeeper for writes)
2. Sync Log (append-only, sequentially-ordered)
3. Sync Service (pull-based replication)

## Design Documents

- `docs/core/sync.md` - Complete sync system specification
- `docs/core/sync-setup.md` - Library sync setup flow
- `docs/core/events.md` - Unified event system
- `docs/core/normalized_cache.md` - Client-side cache
- `docs/core/devices.md` - Device system and leadership

## Architecture Decisions

- **Pull-based sync**: Followers poll leader every 5 seconds
- **Bulk optimization**: 1K+ items create metadata-only sync logs
- **Leader election**: Single leader per library assigns sequences
- **Conflict resolution**: Last-Write-Wins via version field
- **No CRDTs**: Simpler approach, sufficient for metadata

## Acceptance Criteria

- [x] Detailed protocol design document created
- [x] Protocol addresses all aspects of Library Sync
- [x] Design reviewed and approved
- [x] Implementation tasks created (LSYNC-006 through LSYNC-011)
