---
id: LSYNC-006
title: TransactionManager Core Implementation
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: Critical
tags: [sync, database, transaction, architecture]
---

## Description

Implement the TransactionManager, the sole gatekeeper for all syncable database writes. It guarantees atomic DB commits + sync log creation, ensuring that state changes are always logged for synchronization.

## Implementation Steps

1. Create `TransactionManager` struct with event bus and sync sequence tracking
2. Implement `commit<M, R>()` for single resource changes
3. Implement `commit_batch<M, R>()` for 10-1K item batches
4. Implement `commit_bulk<M>()` for 1K+ items (metadata-only sync logs)
5. Add sequence number generation (only on leader devices)
6. Integrate with event emission (automatic ResourceChanged events)
7. Add `with_tx()` for raw SQL compatibility

## Technical Details

- Location: `core/src/infra/transaction/manager.rs`
- Must check leader status before assigning sequence numbers
- Atomic transaction: DB write + sync log entry creation
- Auto-emit events via EventBus after commit
- Bulk operations create single metadata sync log (not per-item)

## Acceptance Criteria

- [ ] TransactionManager can commit single resources with sync logs
- [ ] Batch operations create per-item sync logs
- [ ] Bulk operations create metadata-only sync logs
- [ ] Events emitted automatically after commits
- [ ] Leader check prevents non-leaders from creating sync logs
- [ ] Unit tests verify atomicity
- [ ] Integration tests validate sync log creation

## References

- `docs/core/sync.md` - Complete specification
- Phase 1 dependency for sync system
