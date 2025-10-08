---
id: LSYNC-009
title: Sync Leader Election & Lease Management
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, leadership, distributed-systems]
---

## Description

Implement the leader election mechanism that ensures each library has a single leader device responsible for assigning sync log sequence numbers. This prevents sequence collisions and ensures consistent ordering.

## Implementation Steps

1. Create `SyncLeader` struct with lease tracking
2. Implement `request_leadership()` method
3. Implement `is_leader()` check
4. Add heartbeat mechanism (leader sends every 30s)
5. Implement re-election on leader timeout (>60s)
6. Use highest device_id as tiebreaker
7. Integrate with TransactionManager

## Election Strategy

- **Initial leader**: Device that creates the library
- **Heartbeat**: Leader sends heartbeat every 30 seconds
- **Re-election**: If leader offline >60s, devices elect new leader
- **Tiebreaker**: Highest device_id wins
- **Lease**: Leader holds exclusive write lease

## Technical Details

- Location: `core/src/infra/sync/leader.rs`
- Leadership state stored in device's `sync_leadership` field
- Lease expires_at tracked per library
- TransactionManager checks leadership before assigning sequences

## Acceptance Criteria

- [ ] Leader election on library creation
- [ ] Heartbeat mechanism prevents false timeouts
- [ ] Re-election works when leader goes offline
- [ ] Only leader can create sync log entries
- [ ] Follower devices reject write attempts with clear error
- [ ] Integration tests validate failover scenarios

## Future Enhancements

- Multi-leader support with sequence partitioning
- Manual leader reassignment via admin action
- Leader election metrics and monitoring

## References

- `docs/core/sync.md` lines 238-280
- `docs/core/devices.md` - Sync leadership model
