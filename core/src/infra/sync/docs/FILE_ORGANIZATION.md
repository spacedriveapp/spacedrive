# Sync System - File Organization Guide

Quick reference for navigating the sync system codebase.

## Documentation Index

**New to the sync system?** Read these in order:
1. [`SYNC_IMPLEMENTATION_GUIDE.md`](./SYNC_IMPLEMENTATION_GUIDE.md) - Implementation guide with architectural principles
2. [`/docs/core/sync.md`](/Users/jamespine/Projects/spacedrive/docs/core/sync.md) - Comprehensive architectural design
3. [`SYNC_IMPLEMENTATION_ROADMAP.md`](./SYNC_IMPLEMENTATION_ROADMAP.md) - Current status and priorities
4. This file - Where everything lives in the codebase

---

## File Structure by Layer

### Layer 1: Infrastructure (`core/src/infra/sync/`)

Core sync primitives and data structures.

```
infra/sync/
├── mod.rs                              Module exports
├── hlc.rs                              Hybrid Logical Clock implementation
├── peer_log.rs                         Per-device sync.db for shared changes
├── syncable.rs                         Syncable trait for models
├── registry.rs                         Model registry with apply function pointers
├── deterministic.rs                    Deterministic UUIDs for system resources
├── transaction.rs                      TransactionManager for atomic commits (stubbed)
├── transport.rs                        NetworkTransport trait - sync's network interface
│
├── SYNC_IMPLEMENTATION_GUIDE.md        START HERE - Implementation guide
├── SYNC_IMPLEMENTATION_ROADMAP.md      Comprehensive implementation tracking
├── NETWORK_INTEGRATION_STATUS.md       Network integration progress
└── FILE_ORGANIZATION.md                This file
```

**Purpose**: Low-level sync primitives that other layers build on.

---

### Layer 2: Sync Service (`core/src/service/sync/`)

High-level sync coordination and state management.

```
service/sync/
├── mod.rs                              SyncService - main entry point
├── peer.rs                             PeerSync - core sync engine with network
├── state.rs                            Sync state machine + buffer queue
├── protocol_handler.rs                 StateSyncHandler + LogSyncHandler
├── backfill.rs                         Backfill coordinator for new devices
└── applier.rs                          DEPRECATED - Delete this
```

**Purpose**: Orchestrates sync operations, manages state, handles backfill.

**Note**: `peer.rs` now has `network: Arc<dyn NetworkTransport>` and can broadcast! ✅

---

### Layer 3: Network Integration (`core/src/service/network/`)

Network transport and protocol handling.

#### A. Protocol Handlers (Inbound)
```
service/network/protocol/sync/
├── mod.rs                              Module exports
├── messages.rs                         SyncMessage enum - all message types
├── handler.rs                          ️ SyncProtocolHandler - STUBBED (CRITICAL)
└── transport.rs                        DUPLICATE - Delete this
```

**Purpose**: Handle incoming sync messages from peers.

#### B. Network Transports (Outbound)
```
service/network/transports/
├── mod.rs                              Module organization + docs
└── sync.rs                             NetworkTransport impl for NetworkingService
```

**Purpose**: Send sync messages to peers.

#### C. Supporting Infrastructure
```
service/network/
├── core/
│   ├── mod.rs                          NetworkingService + endpoint() getter
│   └── sync_transport.rs               OLD VERSION - Delete this
└── device/
    └── registry.rs                     DeviceRegistry - UUIDNodeId mapping
```

**Purpose**: Bridge between sync layer (device UUIDs) and network layer (Iroh NodeIds).

---

### Layer 4: Database Entities (`core/src/infra/db/entities/`)

Models that implement sync apply functions.

```
infra/db/entities/
├── location.rs                         Location model + apply_state_change()
├── tag.rs                              Tag model + apply_shared_change()
│
├── entry.rs                            ️ Entry model (needs apply)
├── volume.rs                           ️ Volume model (needs apply)
├── device.rs                           ️ Device model (needs apply)
├── collection.rs                       ️ Album model (needs apply)
└── user_metadata.rs                    ️ UserMetadata model (needs apply)
```

**Purpose**: Domain models that participate in sync.

**Status**: 2/7 models have apply functions implemented.

---

### Layer 5: Operations/Actions (`core/src/ops/network/sync_setup/`)

CLI/API operations for managing sync configuration.

```
ops/network/sync_setup/
├── mod.rs                              Module exports
├── action.rs                           SyncSetupAction - configure sync partners
├── input.rs                            Action input schema
├── output.rs                           Action output schema
└── discovery/
    ├── mod.rs                          Module exports
    ├── query.rs                        DiscoverLibrariesQuery
    └── output.rs                       Discovery results
```

**Purpose**: User-facing operations to set up sync between devices.

---

### Layer 6: Database Migrations (`core/src/infra/db/migration/`)

Schema changes for sync-related tables.

```
infra/db/migration/
└── m20250200_000001_remove_sync_leadership.rs    Migration for leaderless sync
```

**Purpose**: Database schema evolution for sync features.

---

## ️ Data Flow Map

### Outbound Sync (Device A → Device B)

```
1. User Action
   └─ LocationManager.create_location()

2. Database Write
   └─ location.insert(db).await

3. Sync Trigger (TODO: Auto via TransactionManager)
   └─ peer_sync.broadcast_state_change(change)
      └─ service/sync/peer.rs:144-219

4. Network Transport
   └─ network.send_sync_message(device_b_uuid, message)
      └─ service/network/transports/sync.rs:32-102

5. UUID → NodeId Mapping
   └─ device_registry.get_node_id_for_device(device_b_uuid)
      └─ service/network/device/registry.rs

6. Iroh Send
   └─ endpoint.connect(node_id, SYNC_ALPN)
      └─ service/network/core/mod.rs

7. Bytes Over Network
   └─ Iroh QUIC transport
```

### Inbound Sync (Device B receives from Device A)

```
1. Iroh Receive
   └─ endpoint.accept() in NetworkingService

2. Protocol Router
   └─ Match ALPN = "sync"
      └─ service/network/core/mod.rs

3. SyncProtocolHandler (STUBBED - P1 PRIORITY!)
   └─ handler.handle_stream(send, recv, remote_node_id)
      └─ service/network/protocol/sync/handler.rs:45-52

4. Message Deserialization
   └─ SyncMessage::from_bytes()
      └─ service/network/protocol/sync/messages.rs

5. NodeId → UUID Mapping
   └─ device_registry.get_device_by_node(remote_node_id)
      └─ service/network/device/registry.rs

6. Dispatch to Handler
   └─ Match on SyncMessage variant
      ├─ StateChange → peer_sync.on_state_change_received()
      └─ SharedChange → peer_sync.on_shared_change_received()
         └─ service/sync/peer.rs:321-407

7. Apply via Registry
   └─ registry::apply_state_change(model_type, data, db)
      └─ infra/sync/registry.rs:160-189

8. Model Apply Function
   └─ location::Model::apply_state_change(data, db)
      └─ infra/db/entities/location.rs
```

---

## Finding Code

### "Where is X defined?"

| What | Where | Status |
|------|-------|--------|
| **NetworkTransport trait** | `infra/sync/transport.rs:42-114` | |
| **NetworkTransport impl** | `service/network/transports/sync.rs:23-152` | |
| **SyncMessage types** | `service/network/protocol/sync/messages.rs:14-107` | |
| **PeerSync broadcasting** | `service/sync/peer.rs:144-318` | |
| **Inbound handler** | `service/network/protocol/sync/handler.rs` | ️ Stubbed |
| **Registry dispatch** | `infra/sync/registry.rs:160-222` | |
| **HLC implementation** | `infra/sync/hlc.rs` | |
| **PeerLog (sync.db)** | `infra/sync/peer_log.rs` | |
| **DeviceRegistry** | `service/network/device/registry.rs` | |
| **TransactionManager** | `infra/sync/transaction.rs` | Stubbed |

### "How do I..."

| Task | File | Function |
|------|------|----------|
| **Send a sync message** | `service/sync/peer.rs` | `broadcast_state_change()` |
| **Handle received message** | `service/network/protocol/sync/handler.rs` | `handle_stream()` (stub) |
| **Add new model to sync** | `infra/db/entities/[model].rs` | Implement `apply_state_change()` |
| **Register model** | `infra/sync/registry.rs` | Add to `initialize_registry()` |
| **Generate HLC** | `infra/sync/hlc.rs` | `HLCGenerator::next()` |
| **Write to peer log** | `infra/sync/peer_log.rs` | `PeerLog::append()` |
| **Map UUIDNodeId** | `service/network/device/registry.rs` | `DeviceRegistry` methods |

---

## File Status Matrix

| File | LOC | Status | Priority | Notes |
|------|-----|--------|----------|-------|
| **Core Infrastructure** |
| `infra/sync/hlc.rs` | 150 | Complete | - | HLC implementation |
| `infra/sync/peer_log.rs` | 300 | Complete | - | Sync.db management |
| `infra/sync/syncable.rs` | 50 | Complete | - | Trait definition |
| `infra/sync/registry.rs` | 286 | Complete | - | Dynamic dispatch |
| `infra/sync/transport.rs` | 182 | Complete | - | NetworkTransport trait |
| `infra/sync/transaction.rs` | 226 | Stubbed | **P1** | Need auto-broadcast |
| **Network Layer** |
| `service/network/transports/sync.rs` | 169 | Complete | - | Outbound messaging |
| `service/network/protocol/sync/handler.rs` | 94 | ️ Stubbed | **P1** | **CRITICAL** |
| `service/network/protocol/sync/messages.rs` | 205 | Complete | - | Message types |
| **Sync Service** |
| `service/sync/peer.rs` | 482 | Complete | - | Core sync engine |
| `service/sync/state.rs` | 200 | Complete | - | State machine |
| `service/sync/mod.rs` | 184 | Complete | - | Service wrapper |
| **Models** |
| `infra/db/entities/location.rs` | ? | Complete | - | Has apply function |
| `infra/db/entities/tag.rs` | ? | Complete | - | Has apply function |
| `infra/db/entities/entry.rs` | ? | ️ Partial | P2 | Needs apply |
| `infra/db/entities/volume.rs` | ? | ️ Partial | P2 | Needs apply |
| `infra/db/entities/device.rs` | ? | ️ Partial | P2 | Needs apply |
| `infra/db/entities/collection.rs` | ? | ️ Partial | P2 | Needs apply |
| `infra/db/entities/user_metadata.rs` | ? | ️ Partial | P2 | Needs apply |

**Total Lines**: ~2,500 LOC
**Completion**: 75% (25/34 files)

---

## Files to Delete

These files are legacy/duplicate and should be removed:

1. **`service/sync/applier.rs`**
   - Status: Legacy stub from leader-based sync
   - Reason: Replaced by PeerSync
   - Action: `rm -f core/src/service/sync/applier.rs`
   - Update: Remove from `service/sync/mod.rs`

2. **`service/network/protocol/sync/transport.rs`**
   - Status: Duplicate of `transports/sync.rs`
   - Reason: Wrong location (should be in transports/)
   - Action: `rm -f core/src/service/network/protocol/sync/transport.rs`
   - Update: Already not imported

3. **`service/network/core/sync_transport.rs`**
   - Status: Old version, moved to transports/
   - Reason: File was moved
   - Action: `rm -f core/src/service/network/core/sync_transport.rs`
   - Update: Already updated to use transports/

---

## Quick Links

### Documentation
- [Implementation Roadmap](./SYNC_IMPLEMENTATION_ROADMAP.md) - Comprehensive tracking
- [Network Integration Status](./NETWORK_INTEGRATION_STATUS.md) - Phase progress
- [Sync Roadmap (docs)](../../../docs/core/sync-roadmap.md) - High-level overview

### Key Files (Most Important)
1. `service/sync/peer.rs` - Core sync engine 
2. `service/network/transports/sync.rs` - Network transport 
3. `infra/sync/registry.rs` - Model dispatch 
4. `service/network/protocol/sync/handler.rs` - Inbound handling ️ STUB

### Tests
- Unit tests: Inline in each file (`#[cfg(test)]` modules)
- Integration tests: `core/tests/sync_integration_test.rs` (TODO)

---

## Navigation Tips

### By Layer
- **Infrastructure**: `core/src/infra/sync/`
- **Service**: `core/src/service/sync/`
- **Network**: `core/src/service/network/`
- **Models**: `core/src/infra/db/entities/`
- **Operations**: `core/src/ops/network/sync_setup/`

### By Concern
- **Message Definition**: `service/network/protocol/sync/messages.rs`
- **Sending Messages**: `service/network/transports/sync.rs`
- **Receiving Messages**: `service/network/protocol/sync/handler.rs`
- **Broadcasting Logic**: `service/sync/peer.rs`
- **Model Application**: `infra/db/entities/[model].rs`

### By State
- **Complete**: Look at `location.rs` and `tag.rs` for examples
- **In Progress**: Check `transaction.rs` for stubbed methods
- **Blocked**: `handler.rs` blocks all inbound sync

---

**Last Updated**: October 9, 2025
**Purpose**: Quick reference for developers navigating the sync codebase

