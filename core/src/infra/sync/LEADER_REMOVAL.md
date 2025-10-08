# Leader Removal Checklist

This document tracks all leader-related code that needs to be removed or updated for the leaderless hybrid sync model.

## Core Infrastructure

### 1. `/core/src/infra/sync/leader.rs`
- [ ] **Remove entire file** - Contains LeadershipManager, SyncRole, SyncLeadership
- Alternative: Keep minimal version for device state tracking only

### 2. `/core/src/infra/sync/transaction_manager.rs`
- [ ] Remove `NotLeader` error variant (line 43)
- [ ] Remove leader checks in `log_change()` (lines 131-134)
- [ ] Remove leader checks in `log_batch()` (lines 196-198)
- [ ] Remove leader checks in `log_bulk()` (lines 237-239)
- [ ] Remove `leadership_manager` field and constructor parameter

### 3. `/core/src/infra/sync/mod.rs`
- [ ] Remove exports: `LeadershipManager`, `SyncLeadership`, `SyncRole`

## Service Layer

### 4. `/core/src/service/sync/mod.rs`
- [ ] Remove `SyncRole` usage
- [ ] Remove role-based branching in sync loop
- [ ] Simplify to single sync behavior

### 5. `/core/src/service/sync/leader.rs`
- [ ] **Remove or rename** to `sync_broadcaster.rs`
- [ ] Remove leader-specific logic

### 6. `/core/src/service/sync/follower.rs`
- [ ] **Rename** to `sync_receiver.rs` or similar
- [ ] Update terminology from "follower"

## Library Management

### 7. `/core/src/library/manager.rs`
- [ ] Remove `LeadershipManager` creation (lines 219-228)
- [ ] Remove leader initialization logic (lines 250-269)
- [ ] Update `TransactionManager` creation

### 8. `/core/src/library/mod.rs`
- [ ] Remove `leadership_manager` field
- [ ] Remove `leadership_manager()` getter
- [ ] Update struct initialization

## Context

### 9. `/core/src/context.rs`
- [ ] Remove `LeadershipManager` import
- [ ] Remove `leadership_manager` field
- [ ] Remove leadership manager initialization (lines 45-50)

## Network Protocol

### 10. `/core/src/service/network/protocol/sync/messages.rs`
- [ ] Update message descriptions (remove "Leader → Follower")
- [ ] Remove `role` field from `Heartbeat`

### 11. `/core/src/service/network/protocol/sync/handler.rs`
- [ ] Remove leader/follower specific handling

## Database Schema

### 12. `/core/src/infra/db/entities/device.rs`
- [ ] Remove or deprecate `sync_leadership` field

### 13. `/core/src/infra/db/migration/`
- [ ] Create migration to remove `sync_leadership` column from devices table

## Domain Models

### 14. `/core/src/domain/device.rs`
- [ ] Remove or update `SyncRole` enum
- [ ] Remove sync leadership methods

## Operations

### 15. Various ops files
- [ ] `/core/src/ops/devices/list/` - Remove leader status from output
- [ ] `/core/src/ops/network/sync_setup/` - Remove leader assignment

## Testing & Examples

### 16. Examples
- [ ] Update `sync_integration_demo.rs`
- [ ] Update `library_demo.rs`

### 17. Tests
- [ ] Remove or update leader election tests
- [ ] Update integration tests

## Terminology Updates

Replace throughout codebase:
- "leader" → "broadcaster" or "sender"
- "follower" → "receiver"
- "leader election" → (remove)
- "leadership" → (remove)

## New Components Needed

### 1. HLC Implementation
- [ ] Create `core/src/infra/sync/hlc.rs`
- [ ] Implement Hybrid Logical Clock

### 2. State-Based Sync
- [ ] Create state sync for device-owned data
- [ ] Implement efficient delta sync

### 3. Per-Device Sync Logs
- [ ] Modify sync.db schema for per-device logs
- [ ] Add peer acknowledgment tracking
- [ ] Implement log pruning

## Migration Strategy

1. **Phase 1**: Add new components in parallel
   - Implement HLC
   - Add state-based sync
   - Keep leader system running

2. **Phase 2**: Switch to hybrid model
   - Use state sync for device-owned data
   - Use HLC for shared resources
   - Disable leader writes

3. **Phase 3**: Remove leader code
   - Delete all items in this checklist
   - Clean up tests and docs