---
id: LSYNC-010
title: Sync Service (Leader & Follower)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, replication, service, leader, follower]
depends_on: [LSYNC-006, LSYNC-008, LSYNC-009, LSYNC-013]
---

## Description

Implement the complete sync service with both leader and follower functionality. The leader pushes notifications to followers when new sync log entries are created, and followers receive these notifications and apply changes locally.

**Architecture**: Message-based (push) instead of polling for better performance, lower latency, and battery efficiency.

## Implementation Steps

### Core Service
1. Create `SyncService` struct with role-specific behavior
2. Initialize service when library opens (determines leader/follower role)
3. Integrate with SyncProtocolHandler for messaging
4. Handle role transitions (leader election changes)

### Leader Functionality
5. Subscribe to TransactionManager commit events
6. Implement `on_commit()` - called after sync log entry created
7. Implement `notify_followers()` - sends NewEntries to all followers
8. Implement batch notification logic (debounce within 100ms)
9. Implement message handler for `SyncMessage::FetchEntries`
10. Implement message handler for `SyncMessage::Heartbeat`
11. Track connected followers per library

### Follower Functionality
12. Implement message handler for `SyncMessage::NewEntries`
13. Implement `request_entries()` - uses SyncProtocolHandler to fetch
14. Implement `apply_sync_entry()` - deserializes and applies changes
15. Track `last_synced_sequence` per library
16. Handle bulk operation metadata (trigger local indexing)
17. Handle connection loss with reconnect logic
18. Send heartbeats to leader

## Technical Details

- Location: `core/src/service/sync/`
  - `mod.rs` - SyncService orchestrator
  - `leader.rs` - Leader-specific logic
  - `follower.rs` - Follower-specific logic
- Push-based: Leader notifies when changes happen
- Batch size: Max 100 entries per request
- Error handling: Retry with exponential backoff
- Gap detection: Detect missed entries and reconcile

## Complete Flow

### Leader Side
```
1. TransactionManager commits change + creates sync log entry
2. Leader receives commit event
3. Leader groups entries (if multiple commits in <100ms)
4. Leader sends SyncMessage::NewEntries to all followers:
   - library_id
   - from_sequence (start of batch)
   - to_sequence (end of batch)
   - entry_count
5. Follower requests entries with FetchEntries
6. Leader responds with EntriesResponse (up to 100 entries)
7. Follower sends Acknowledge
```

### Follower Side
```
1. Listen for SyncMessage::NewEntries from leader
2. On notification:
   - Send FetchEntries request (since last_synced_sequence)
   - Receive EntriesResponse
   - For each entry:
     - Deserialize model
     - Apply change to local DB
     - Update last_synced_sequence
   - Send Acknowledge
3. On connection loss:
   - Reconnect
   - Send Heartbeat with current sequence
   - Leader sends SyncRequired if needed
```

## Service Structure

```rust
pub struct SyncService {
    library_id: Uuid,
    role: SyncRole,
    sync_log_db: Arc<SyncLogDb>,
    protocol_handler: Arc<SyncProtocolHandler>,
    event_bus: Arc<EventBus>,

    // Leader-specific
    pending_batches: Arc<Mutex<HashMap<Uuid, NotificationBatch>>>,
    followers: Arc<RwLock<HashSet<Uuid>>>,

    // Follower-specific
    last_synced_sequence: Arc<Mutex<u64>>,
}

impl SyncService {
    /// Create and start sync service for library
    pub async fn start(
        library_id: Uuid,
        role: SyncRole,
        sync_log_db: Arc<SyncLogDb>,
        protocol_handler: Arc<SyncProtocolHandler>,
    ) -> Result<Self, SyncError>;

    /// Leader: Notify followers of new entries
    async fn notify_followers(&self, from_seq: u64, to_seq: u64);

    /// Follower: Apply sync entry locally
    async fn apply_sync_entry(&self, entry: SyncLogEntry);

    /// Handle role transition (election change)
    pub async fn transition_role(&mut self, new_role: SyncRole);
}

## Acceptance Criteria

### Leader
- [ ] Leader service receives commit events
- [ ] Notifications sent to all followers instantly
- [ ] Batch notifications for rapid commits (100ms window)
- [ ] Handles FetchEntries requests
- [ ] Responds with EntriesResponse
- [ ] Tracks connected followers
- [ ] Handles disconnections gracefully

### Follower
- [ ] Follower receives NewEntries push notifications
- [ ] Entries fetched and applied correctly
- [ ] Sequence tracking prevents duplicate application
- [ ] Bulk operations trigger local jobs (not replication)
- [ ] Connection loss handled with reconnect
- [ ] Gap detection triggers full reconciliation

### Integration
- [ ] Service starts correctly based on device role
- [ ] Role transitions handled (leader election)
- [ ] Integration tests validate device-to-device sync
- [ ] Multi-follower scenario tested

## Performance Benefits vs Polling

- **Latency**: Instant (push) vs 5s average (polling)
- **Bandwidth**: Only when changes occur vs constant polls
- **Battery**: Idle until notification vs wake every 5s

## References

- `docs/core/sync.md` - Complete sync specification
- Protocol: LSYNC-013 (Sync protocol handler)
- Leader election: LSYNC-009
- TransactionManager: LSYNC-006
