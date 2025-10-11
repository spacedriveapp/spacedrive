<!--CREATED: 2025-06-18-->
# Device Management Design

## Overview

Spacedrive needs a robust device identification system that persists across application restarts and works seamlessly with library synchronization. Each device running Spacedrive must have a unique, persistent identifier that remains constant throughout its lifetime.

## Requirements

1. **Device Uniqueness**: Each Spacedrive installation must have a globally unique device ID
2. **Persistence**: Device ID must survive application restarts
3. **Library Awareness**: Libraries must know which device they're currently running on
4. **Sync Compatibility**: Device IDs enable proper sync conflict resolution and file ownership tracking

## Architecture

### Device State Storage

The device ID and metadata are stored in a platform-specific configuration location:
- **macOS**: `~/Library/Application Support/com.spacedrive/device.json`
- **Linux**: `~/.config/spacedrive/device.json`
- **Windows**: `%APPDATA%\Spacedrive\device.json`

### Device Configuration File

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Jamie's MacBook Pro",
  "created_at": "2024-01-15T10:30:00Z",
  "hardware_model": "MacBookPro18,1",
  "os": "macOS",
  "version": "0.1.0"
}
```

### Library-Device Relationship

When a device connects to a library:
1. The device registers itself in the library's `devices` table
2. The library tracks which device is currently active
3. All operations (file creation, modification) are tagged with the device ID

### Database Schema

The `devices` table in each library:
```sql
CREATE TABLE devices (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    hardware_model TEXT,
    os TEXT NOT NULL,
    last_seen_at TIMESTAMP NOT NULL,
    is_online BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
```

## Implementation Flow

1. **Application Startup**:
   - Check for existing device configuration
   - If not found, generate new device ID and save configuration
   - Load device ID into memory

2. **Library Connection**:
   - Register/update device in library's devices table
   - Mark device as online
   - Store current device ID in library context

3. **File Operations**:
   - All SdPath creations use the persistent device ID
   - Entry modifications track the device that made changes

4. **Application Shutdown**:
   - Mark device as offline in all connected libraries

## Benefits

1. **Consistent Identity**: Device maintains same ID across all libraries and sessions
2. **Sync Foundation**: Enables proper multi-device synchronization
3. **Audit Trail**: Can track which device created/modified files
4. **Conflict Resolution**: Device IDs help resolve sync conflicts

## Security Considerations

- Device ID should not contain personally identifiable information
- Device configuration file should have appropriate file permissions
- Consider encryption for sensitive device metadata in future versions

## Future Enhancements

1. **Device Pairing**: Secure device-to-device authentication
2. **Device Capabilities**: Track what each device can do (indexing, P2P, etc.)
3. **Device Groups**: Organize devices into groups for easier management
4. **Remote Device Management**: Remove/disable devices from another device