---
id: LSYNC-006
title: Transaction Manager Core
status: Done
assignee: jamiepine
parent: LSYNC-000
priority: Critical
tags: [sync, database, transaction, architecture, leaderless]
design_doc: core/src/infra/sync/NEW_SYNC.md
completed: 2025-10-09
---

## Description

Implement the TransactionManager for atomic database writes with automatic sync broadcasts. In the leaderless model, all devices can write - no leader checks needed!

**Architecture Change**: No longer checks for leader role. All devices broadcast their changes directly.

## Implementation Steps

1. Create `TransactionManager` struct with event bus
2. Implement `commit_device_owned<M>()` for device-owned data:
   - Atomic DB write
   - Broadcast StateChange to peers (no log!)
   - Emit event
3. Implement `commit_shared<M>()` for shared resources:
   - Generate HLC
   - Atomic DB write + shared_changes log entry
   - Broadcast SharedChange to peers
   - Emit event
4. Implement `commit_batch<M>()` for batching
5. Add `with_tx()` for raw SQL compatibility
6. Remove all leader checks!

## Technical Details

- Location: `core/src/infra/transaction/manager.rs`
- **No leader status check** (major simplification!)
- Determines sync strategy from model's `device_id` field
- Device-owned: Just broadcast state
- Shared: Log + broadcast with HLC

## API Design

```rust
impl TransactionManager {
    /// Commit device-owned resource (state-based)
    pub async fn commit_device_owned<M: Syncable>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<M, TxError> {
        // 1. Write to database
        let saved = db.transaction(|txn| {
            model.insert(txn).await
        }).await?;

        // 2. Broadcast state (no log!)
        self.broadcast_state_change(saved.clone()).await?;

        // 3. Emit event
        self.event_bus.emit(Event::ResourceChanged { ... });

        Ok(saved)
    }

    /// Commit shared resource (log-based with HLC)
    pub async fn commit_shared<M: Syncable>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<M, TxError> {
        // 1. Generate HLC
        let hlc = self.hlc_generator.lock().await.next();

        // 2. Atomic: DB write + log entry
        let saved = db.transaction(|txn| async {
            let saved = model.insert(txn).await?;

            // Write to sync log
            sync_db.append(SharedChangeEntry {
                hlc,
                model_type: M::SYNC_MODEL,
                record_uuid: saved.sync_id(),
                change_type: ChangeType::Insert,
                data: serde_json::to_value(&saved)?,
            }, txn).await?;

            Ok(saved)
        }).await?;

        // 3. Broadcast with HLC
        self.broadcast_shared_change(hlc, saved.clone()).await?;

        // 4. Emit event
        self.event_bus.emit(Event::ResourceChanged { ... });

        Ok(saved)
    }
}
```

## Acceptance Criteria

- [x] TransactionManager commits device-owned resources (no log) ✅
- [x] TransactionManager commits shared resources (with HLC log) ✅
- [x] No leader checks anywhere! ✅
- [x] Events emitted automatically ✅
- [ ] Batch operations supported (pending)
- [ ] Unit tests verify atomicity (pending)
- [ ] Integration tests validate both sync strategies (pending)

## Implementation Notes (Oct 9, 2025)

Successfully implemented in `core/src/infra/sync/transaction.rs`:

- `commit_device_owned()` - Emits events for state-based broadcast
- `commit_shared()` - Generates HLC, writes to peer log, emits events for broadcast
- Both methods properly integrate with EventBus for triggering sync
- No leader checks in the code
- Ready for SyncService to consume events and broadcast

## Migration from Leader Model

**Remove**:

- `next_sequence()` method (replaced with HLC)
- `is_leader()` checks
- Sequence number tracking
- Leader-specific logic

**Add**:

- HLC generator integration
- Strategy selection (device-owned vs shared)
- State broadcast for device-owned
- Log + broadcast for shared

## References

- `core/src/infra/sync/NEW_SYNC.md` - Leaderless architecture
- HLC: LSYNC-009
- Syncable trait: LSYNC-007
