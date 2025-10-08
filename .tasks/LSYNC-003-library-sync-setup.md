---
id: LSYNC-003
title: Library Sync Setup (Device Registration & Discovery)
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, networking, library-setup, device-pairing]
---

## Description

Implement the library sync setup flow that enables paired devices to discover each other's libraries and register for synchronization.

**Update (Oct 2025)**: No longer assigns leader roles (leaderless architecture).

## Implementation Notes

- Complete implementation detailed in `docs/core/sync-setup.md`
- Devices must be paired (NET-002) before library sync setup
- Two-phase process: Discovery → Registration
- Bidirectional device registration in each library's database
- ~~Leader election during setup~~ (No longer needed)
- Creates `sync_partners` records (new)
- No actual sync replication in Phase 1 (just registration)

## Changes for Leaderless Model

**Removed**:
- Leader election during setup
- Setting `sync_leadership` field

**Added**:
- Create `sync_partners` table entries
- All devices are peers

## Acceptance Criteria

- [x] Paired devices can discover each other's libraries
- [x] Devices can be registered in remote library databases
- [x] ~~Sync leadership assigned during setup~~ (No longer applicable)
- [x] `sync_partners` records created
- [x] `sync_setup.discover.v1` query implemented
- [x] `sync_setup.input.v1` action implemented
- [x] Integration tests validate cross-device setup

## References

- `docs/core/sync-setup.md` - Setup flow
- `core/src/infra/sync/NEW_SYNC.md` - Leaderless model
