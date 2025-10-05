# Library Sync Setup System

## Overview

This module handles library synchronization setup between paired devices. It is intentionally separate from the pairing protocol to maintain clean separation between networking concerns (device authentication) and application concerns (library data organization).

## Architecture

### Why Separate from Pairing?

1. **Separation of Concerns**: Pairing is a pure networking operation, while library setup involves application-level business logic
2. **User Flexibility**: Users may want to pair devices without immediately syncing libraries
3. **Progressive Enhancement**: Can start simple and evolve independently when sync is fully implemented
4. **Clean Testing**: Each component tests independently

### Components

#### Actions

- **`LibrarySyncSetupAction`** (`action.rs`): CoreAction that sets up library sync between two paired devices
  - Currently implements `RegisterOnly` mode: registers devices in each other's libraries
  - Future modes: `MergeIntoLocal`, `MergeIntoRemote`, `CreateShared` (awaits full sync implementation)

#### Queries

- **`DiscoverRemoteLibrariesQuery`** (`discovery/query.rs`): CoreQuery that discovers libraries on a remote paired device
  - Validates device is paired
  - Returns library metadata (name, stats, etc.)
  - Currently returns empty list (requires messaging protocol extension)

#### Message Types

- **`LibraryMessage`** (`service/network/protocol/library_messages.rs`): Protocol messages for library operations
  - `DiscoveryRequest`/`DiscoveryResponse`: Library discovery
  - `RegisterDeviceRequest`/`RegisterDeviceResponse`: Device registration

## Current Implementation Status

### ✅ Phase 1: Basic Structure (Complete)
- [x] Directory structure and module organization
- [x] Input/Output types with proper serialization
- [x] CQRS action and query registration
- [x] Device registration in local library database
- [x] Validation of paired devices
- [x] Error handling and logging

### 🚧 Phase 2: Network Implementation (Pending)
- [ ] LibraryMessage handler in messaging protocol
- [ ] Actual library discovery over network
- [ ] Remote device registration requests
- [ ] Bi-directional library setup

### 📋 Phase 3: Full Sync Support (Future)
- [ ] Library merging (MergeIntoLocal, MergeIntoRemote)
- [ ] Shared library creation
- [ ] Conflict resolution
- [ ] Sync job initialization
- [ ] Sync leadership management

## Usage Flow

```rust
// 1. User pairs two devices (via pairing protocol)
PairGenerateAction::execute() // Device A
PairJoinAction::execute()     // Device B
// Result: Devices are now paired

// 2. User initiates library sync setup
DiscoverRemoteLibrariesQuery {
    device_id: paired_device_id
}
// Result: List of libraries on remote device (currently empty)

// 3. User selects libraries and sync action
LibrarySyncSetupAction {
    local_device_id: device_a_id,
    remote_device_id: device_b_id,
    local_library_id: my_library_id,
    remote_library_id: their_library_id,
    action: LibrarySyncAction::RegisterOnly,
    leader_device_id: device_a_id,
}
// Result: Devices registered in each other's libraries
```

## Database Changes

The action registers paired devices in library databases:

```sql
INSERT INTO device (uuid, name, os, os_version, ...)
VALUES (remote_device_id, 'Remote Device', 'Desktop', '1.0', ...);
```

This enables:
- Future sync operations
- Device-to-device file operations (Spacedrop)
- Multi-device library awareness

## Integration Points

### With Pairing System
- Depends on: Device must be in `Paired` or `Connected` state
- Location: `core/src/service/network/protocol/pairing/`

### With Library Manager
- Uses: `LibraryManager::get_library()` for validation
- Accesses: Library database for device registration
- Location: `core/src/library/manager.rs`

### With Networking Service
- Uses: `NetworkingService::device_registry()` for device info
- Future: Will use messaging protocol for remote operations
- Location: `core/src/service/network/core/`

### With Sync System (Future)
- Will integrate with: Sync jobs, leader election, merge operations
- Design: See `docs/core/design/SYNC_DESIGN.md` lines 245-311
- Location: TBD (`core/src/sync/` when implemented)

## API Endpoints

### Query: `query:network.sync_setup.discover.v1`
**Input:**
```json
{
  "device_id": "uuid-of-paired-device"
}
```

**Output:**
```json
{
  "device_id": "uuid",
  "device_name": "Device Name",
  "libraries": [
    {
      "id": "uuid",
      "name": "My Library",
      "description": "Optional description",
      "created_at": "2025-01-01T00:00:00Z",
      "statistics": {
        "total_entries": 1000,
        "total_locations": 5,
        "total_size_bytes": 1000000,
        "device_count": 2
      }
    }
  ],
  "is_online": true
}
```

### Action: `action:network.sync_setup.input.v1`
**Input:**
```json
{
  "local_device_id": "uuid",
  "remote_device_id": "uuid",
  "local_library_id": "uuid",
  "remote_library_id": "uuid",
  "action": {
    "type": "RegisterOnly"
  },
  "leader_device_id": "uuid"
}
```

**Output:**
```json
{
  "success": true,
  "local_library_id": "uuid",
  "remote_library_id": "uuid",
  "devices_registered": true,
  "message": "Devices successfully registered for library access"
}
```

## Testing Recommendations

1. **Unit Tests**: Test action validation logic
2. **Integration Tests**: Test device registration in database
3. **E2E Tests**: Test full flow from pairing → discovery → setup
4. **Error Cases**: Test with unpaired devices, invalid libraries, etc.

## Future Enhancements

See `SYNC_DESIGN.md` for full sync system design including:
- Library merging strategies
- Conflict resolution
- Sync jobs and state machines
- Leader election
- Dependency-aware sync protocol

## Notes

- Remote operations currently log TODO warnings
- Full implementation awaits messaging protocol extension
- Design aligns with provisional sync architecture
- Maintains backward compatibility with existing pairing system

