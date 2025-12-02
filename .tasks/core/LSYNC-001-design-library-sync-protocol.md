---
id: LSYNC-001
title: Design Library Sync Protocol (Leaderless)
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, networking, protocol, design, leaderless]
whitepaper: Section 4.5.1
design_doc: core/src/infra/sync/NEW_SYNC.md
---

## Description

Design the detailed protocol for Library Sync using a **leaderless hybrid model**.

**Architecture Update (Oct 2025)**: Moved from leader-based to peer-to-peer model based on data ownership analysis.

## Design Documents

- `core/src/infra/sync/NEW_SYNC.md` - **Current leaderless design**
- ~~`docs/core/sync.md`~~ - Outdated leader-based model
- `docs/core/sync-setup.md` - Library sync setup flow (still valid)
- `docs/core/events.md` - Unified event system
- `docs/core/devices.md` - Device system

## Architecture Decisions (Revised)

- **Leaderless peer-to-peer**: No leader election, all devices equal
- **Hybrid sync strategy**:
  - State-based for device-owned data (locations, entries)
  - Log-based with HLC for shared resources (tags, albums)
- **Bulk optimization**: Batched state transfers for efficiency
- **Conflict resolution**: HLC ordering + union merge
- **No central coordinator**: Each device broadcasts to peers

## Key Changes from Original Design

| Aspect | Old Design | New Design |
|--------|-----------|------------|
| Architecture | Leader/follower | Peer-to-peer |
| Ordering | Central sequences | HLC timestamps |
| Sync log | One central log | Per-device for shared changes only |
| Device-owned data | Goes through leader | Direct state broadcast |
| Complexity | High (election, heartbeats) | Low (simpler) |

## Acceptance Criteria

- [x] Detailed protocol design document created
- [x] Hybrid strategy (state + log) defined
- [x] HLC ordering mechanism specified
- [x] Peer broadcast protocol designed
- [x] Implementation tasks updated

## References

- `core/src/infra/sync/NEW_SYNC.md` - Complete new specification
- HLC implementation: LSYNC-009
- Peer sync service: LSYNC-010
