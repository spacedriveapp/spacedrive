---
id: LSYNC-005
title: Library Sync Setup (Device Registration & Discovery)
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, networking, library-setup, device-pairing]
---

## Description

Implement the library sync setup flow that enables paired devices to discover each other's libraries and register for synchronization. This is Phase 1 of the sync system (RegisterOnly mode).

## Implementation Notes

- Complete implementation detailed in `docs/core/sync-setup.md`
- Devices must be paired (NET-002) before library sync setup
- Two-phase process: Discovery → Registration
- Bidirectional device registration in each library's database
- Leader election during setup
- No actual sync replication in Phase 1 (just registration)

## Acceptance Criteria

- [x] Paired devices can discover each other's libraries
- [x] Devices can be registered in remote library databases
- [x] Sync leadership assigned during setup
- [x] `sync_setup.discover.v1` query implemented
- [x] `sync_setup.input.v1` action implemented
- [x] Integration tests validate cross-device setup
