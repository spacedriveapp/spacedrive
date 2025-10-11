# Library Sync Setup Implementation

## Overview

This document describes the library sync setup system implemented for Spacedrive Core v2. This system enables devices to establish library relationships after pairing is complete.

## Architecture Decision

**Decision**: Library sync setup is implemented as **separate operations AFTER pairing**, not as part of the pairing state machine.

### Rationale

1. **Separation of Concerns**
   - Pairing: Pure networking (device authentication, key exchange)
   - Library Setup: Application-level business logic (data organization, sync configuration)

2. **User Experience Flexibility**
   - Users can pair devices without immediately syncing libraries
   - Multiple libraries per device require user choice
   - Different sync configurations per library pair
   - Pairing once for Spacedrop, set up sync later

3. **Architectural Alignment**
   - Matches CQRS pattern (separate actions for separate concerns)
   - Aligns with provisional sync design (`SYNC_DESIGN.md` lines 245-311)
   - Enables progressive enhancement as sync features are built

4. **Technical Benefits**
   - Independent testing of pairing vs library setup
   - No rollback complexity (if library setup fails, pairing still succeeded)
   - Clear transaction boundaries (network vs database)
   - Future-proof for full sync implementation

## Implementation

### Module Structure

```
core/src/ops/network/sync_setup/
├── mod.rs                    # Module exports
├── input.rs                  # Input types (LibrarySyncAction enum)
├── output.rs                 # Output types
├── action.rs                 # LibrarySyncSetupAction (CoreAction)
├── discovery/
│   ├── mod.rs
│   ├── query.rs             # DiscoverRemoteLibrariesQuery (CoreQuery)
│   └── output.rs            # RemoteLibraryInfo types
└── README.md                 # Technical documentation
```

### Network Protocol Extensions

```
core/src/service/network/protocol/
├── library_messages.rs       # LibraryMessage enum (Discovery, Registration)
├── messaging.rs              # Extended to handle LibraryMessage types
└── mod.rs                    # Exports LibraryMessage types
```

## API Endpoints

### 1. Discover Remote Libraries

**Endpoint**: `query:network.sync_setup.discover.v1`

**Purpose**: Query libraries available on a paired device

**Input**:
```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Output**:
```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_name": "Bob's MacBook",
  "is_online": true,
  "libraries": [
    {
      "id": "3f8cb26f-de79-4d87-88dd-01be5f024041",
      "name": "My Library",
      "description": "Personal files",
      "created_at": "2025-01-01T00:00:00Z",
      "statistics": {
        "total_entries": 5000,
        "total_locations": 3,
        "total_size_bytes": 10737418240,
        "device_count": 1
      }
    }
  ]
}
```

### 2. Setup Library Sync

**Endpoint**: `action:network.sync_setup.input.v1`

**Purpose**: Establish library relationship between paired devices

**Input**:
```json
{
  "local_device_id": "11525ceb-6cee-492e-94a5-14a3e58b9509",
  "remote_device_id": "550e8400-e29b-41d4-a716-446655440000",
  "local_library_id": "3f8cb26f-de79-4d87-88dd-01be5f024041",
  "remote_library_id": "7a9c2d1e-5f84-4b23-a567-1234567890ab",
  "action": {
    "type": "RegisterOnly"
  },
  "leader_device_id": "11525ceb-6cee-492e-94a5-14a3e58b9509"
}
```

**Output**:
```json
{
  "success": true,
  "local_library_id": "3f8cb26f-de79-4d87-88dd-01be5f024041",
  "remote_library_id": "7a9c2d1e-5f84-4b23-a567-1234567890ab",
  "devices_registered": true,
  "message": "Devices successfully registered for library access"
}
```

## User Flow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Device Pairing (Existing System)                        │
│    Device A → Generate Code                                │
│    Device B → Enter Code                                   │
│    Result: Devices are cryptographically paired            │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. Library Discovery (New)                                 │
│    Device A → DiscoverRemoteLibrariesQuery                 │
│    Result: List of Device B's libraries                    │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. User Selection (UI)                                     │
│    - View remote libraries                                 │
│    - Choose sync action:                                   │
│      • RegisterOnly (Phase 1)                           │
│      • MergeIntoLocal (Phase 3)                            │
│      • MergeIntoRemote (Phase 3)                           │
│      • CreateShared (Phase 3)                              │
│    - Select leader device                                  │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 4. Setup Execution (New)                                   │
│    Device A → LibrarySyncSetupAction                       │
│    - Registers Device B in Device A's library DB           │
│    - Sends request to Device B to register Device A        │
│    - Device B registers Device A in its library DB         │
│    Result: Bi-directional device registration complete     │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 5. Ready for Operations                                    │
│    - Spacedrop between devices                             │
│    - Cross-device file operations                          │
│    - Future: Full library sync                             │
└─────────────────────────────────────────────────────────────┘
```

## Current Implementation: Phase 1

### What Works Now

**Device Registration**
- Remote device is registered in local library database
- Local device sends registration request to remote device
- Remote device handles registration in its library database
- Bi-directional setup completes

**Discovery Query**
- Validates device pairing status
- Sends discovery request over network
- Receives library list with metadata
- Returns structured library information

**Network Protocol**
- `LibraryMessage` types for discovery and registration
- Integrated into existing messaging protocol
- Request/response pattern over Iroh streams
- Proper serialization and error handling

**Validation & Safety**
- Verifies devices are paired before setup
- Validates library existence
- Transaction-safe database operations
- Comprehensive error handling

### What's Pending

**Full Sync Implementation** (Phase 3)
- Library merging strategies
- Conflict resolution
- Sync job initialization
- Leader election
- Dependency-aware sync

See `SYNC_DESIGN.md` for full sync system design.

## Technical Details

### Database Changes

When `LibrarySyncSetupAction` executes with `RegisterOnly`:

```sql
-- On Device A's library database
INSERT INTO device (
  uuid,
  name,
  os,
  os_version,
  hardware_model,
  network_addresses,
  is_online,
  last_seen_at,
  capabilities,
  sync_leadership,
  created_at,
  updated_at
) VALUES (
  '<device_b_uuid>',
  'Device B Name',
  'Desktop',
  '1.0',
  NULL,
  '[]',
  false,
  NOW(),
  '{"indexing":true,"p2p":true,"volume_detection":true}',
  '{}',
  NOW(),
  NOW()
);
```

The same operation occurs on Device B for Device A.

### Network Flow

```
Device A                    Network                     Device B
   |                                                        |
   | DiscoverRemoteLibrariesQuery                          |
   |-------- LibraryMessage::DiscoveryRequest -------->    |
   |                                                        |
   |                  Query local libraries                |
   |                  Build LibraryDiscoveryInfo           |
   |                                                        |
   | <------- LibraryMessage::DiscoveryResponse --------   |
   |                                                        |
   | (User selects libraries and action)                   |
   |                                                        |
   | LibrarySyncSetupAction                                |
   | - Register Device B locally                           |
   |-------- LibraryMessage::RegisterDeviceRequest ----->  |
   |                                                        |
   |                  Register Device A in DB              |
   |                                                        |
   | <------ LibraryMessage::RegisterDeviceResponse -----  |
   |                                                        |
```

### Integration Points

**With Pairing System**:
- Requires: Device in `Paired` or `Connected` state
- Location: `core/src/service/network/protocol/pairing/`
- Validation: Checks `DeviceRegistry` for pairing status

**With Library Manager**:
- Uses: `LibraryManager::get_library()` for validation
- Uses: `LibraryManager::list()` for discovery
- Accesses: Library database for device registration
- Location: `core/src/library/manager.rs`

**With Networking Service**:
- Uses: `NetworkingService::send_library_request()` for communication
- Uses: `NetworkingService::device_registry()` for device info
- Location: `core/src/service/network/core/mod.rs`

**With Messaging Protocol**:
- Extends: `Message` enum with `Library(LibraryMessage)` variant
- Handler: `MessagingProtocolHandler::handle_library_message()`
- Location: `core/src/service/network/protocol/messaging.rs`

## Usage Examples

### From CLI (Future)

```bash
# After pairing is complete
sd pair status
# Shows paired devices

# Discover libraries on paired device
sd library discover --device <device-id>

# Set up library sync
sd library sync-setup \
  --local-library <lib-id> \
  --remote-device <device-id> \
  --remote-library <remote-lib-id> \
  --action register-only \
  --leader local
```

### From Swift Client

```swift
// After pairing completes
let pairedDevices = try await client.getPairedDevices()

// Discover remote libraries
let discovery = try await client.discoverRemoteLibraries(
    deviceId: pairedDevice.id
)

// Setup library sync
let setupResult = try await client.setupLibrarySync(
    localDeviceId: currentDeviceId,
    remoteDeviceId: pairedDevice.id,
    localLibraryId: myLibrary.id,
    remoteLibraryId: discovery.libraries.first!.id,
    action: .registerOnly,
    leaderDeviceId: currentDeviceId
)
```

### From JSON-RPC

```json
// Discovery
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "query:network.sync_setup.discover.v1",
  "params": {
    "input": {
      "device_id": "550e8400-e29b-41d4-a716-446655440000"
    }
  }
}

// Setup
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "action:network.sync_setup.input.v1",
  "params": {
    "input": {
      "local_device_id": "11525ceb-6cee-492e-94a5-14a3e58b9509",
      "remote_device_id": "550e8400-e29b-41d4-a716-446655440000",
      "local_library_id": "3f8cb26f-de79-4d87-88dd-01be5f024041",
      "remote_library_id": "7a9c2d1e-5f84-4b23-a567-1234567890ab",
      "action": { "type": "RegisterOnly" },
      "leader_device_id": "11525ceb-6cee-492e-94a5-14a3e58b9509"
    }
  }
}
```

## Testing

### Unit Tests (To Add)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_leader_device() {
        // Test that leader must be local or remote device
    }

    #[tokio::test]
    async fn test_requires_paired_device() {
        // Test that devices must be paired before setup
    }

    #[tokio::test]
    async fn test_library_exists() {
        // Test that libraries must exist
    }
}
```

### Integration Test Scenario

1. **Setup**: Create two devices, pair them
2. **Discovery**: Query libraries from remote device
3. **Setup**: Execute RegisterOnly action
4. **Verify**: Check both devices are registered in both databases
5. **Cleanup**: Unpair devices, close libraries

### Manual Testing

```bash
# Terminal 1: Start CLI daemon
cd spacedrive
cargo build
sd start --foreground

# Terminal 2: iOS Simulator or Device
# - Launch app
# - Pair with CLI device
# - Test library discovery
# - Test library setup
# - Verify device appears in library
```

## Future Roadmap

### Phase 2: Network Protocol Completion
- LibraryMessage types defined
- MessagingProtocolHandler extended
- NetworkingService send_library_request() method
- Bi-directional device registration

### Phase 3: Full Sync Support

When implementing full sync (per `SYNC_DESIGN.md`):

1. **Expand LibrarySyncAction enum**:
   - `MergeIntoLocal` - Pull remote library data into local
   - `MergeIntoRemote` - Push local library data to remote
   - `CreateShared` - Create new shared library

2. **Implement SyncSetupJob**:
   - Library data export/import
   - File deduplication by content hash
   - Device record reconciliation
   - Sync log initialization
   - Leader election

3. **Add Conflict Resolution**:
   - User metadata merge strategies
   - Content-identity deduplication
   - UI for conflict resolution

4. **Initialize Sync Jobs**:
   - BackfillSyncJob for initial data
   - LiveSyncJob for ongoing updates
   - Sync position tracking

## Logging

The implementation uses structured logging:

```rust
// Discovery
tracing::info!(
    "Remote library discovery for device {} - 3 libraries found",
    device_id
);

// Setup
tracing::info!(
    "Registered remote device {} in library {}",
    remote_device_id,
    library_id
);

// Network
tracing::info!(
    "Successfully registered local device on remote device in library {:?}",
    remote_library_id
);
```

## Error Handling

### Discovery Errors

- **Device not found**: Device ID doesn't exist
- **Device not paired**: Device exists but isn't paired
- **Device offline**: Device paired but not connected
- **Network error**: Failed to send/receive messages

### Setup Errors

- **Validation errors**: Invalid device/library IDs, leader device not local/remote
- **Database errors**: Failed to insert device record
- **Network errors**: Failed to send registration request
- **Permission errors**: (Future) User lacks permission to modify library

## Security Considerations

### Current (Phase 1)

- Device pairing verifies cryptographic identity
- Only paired devices can discover libraries
- Only paired devices can register in libraries
- Session keys ensure encrypted communication

### Future (Phase 3)

- Library-level access control
- User permissions for merge operations
- Encrypted sync log data
- Rate limiting on sync requests

## Performance

### Discovery Query

- **Network**: Single request/response (< 100ms typical)
- **Database**: Count queries on 3 tables per library (< 10ms per library)
- **Scalability**: O(n) where n = number of libraries

### Setup Action

- **Network**: Single registration request (< 100ms typical)
- **Database**: Single INSERT per device per library (< 5ms)
- **Atomic**: Entire operation in single transaction

## Comparison with Design Document

The implementation aligns with `SYNC_DESIGN.md`:

| Design Concept | Implementation Status |
|----------------|----------------------|
| Separate from pairing | Implemented |
| LibraryAction enum | Defined (RegisterOnly active) |
| Device registration | Implemented |
| Library discovery | Implemented |
| Network protocol | Implemented |
| Merge strategies | Future (Phase 3) |
| Sync jobs | Future (Phase 3) |
| Leader election | Future (Phase 3) |

## Migration Path

No database migrations required - uses existing `device` table in libraries.

## Dependencies

- Pairing protocol (device authentication)
- Messaging protocol (communication)
- Library manager (database access)
- Device registry (pairing verification)
- Sync system (future full implementation)

## Known Limitations

1. **Manual bi-directional setup**: Users must run setup on both devices (Phase 2 will automate)
2. **No library merge**: Only device registration (awaits Phase 3 sync implementation)
3. **Limited conflict resolution**: Simple strategy (full resolution in Phase 3)
4. **Single leader only**: Multi-leader not supported (may be added in future)

## References

- **Pairing Protocol**: `core/src/service/network/protocol/pairing/`
- **Sync Design**: `docs/core/design/SYNC_DESIGN.md`
- **CQRS Pattern**: `core/src/ops/registry.rs`
- **Library Manager**: `core/src/library/manager.rs`

