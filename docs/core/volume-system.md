# Volume System Documentation

The Volume System in Spacedrive Core v2 provides cross-platform storage volume detection, monitoring, and management capabilities. It enables volume-aware file operations and persistent tracking of storage devices across sessions.

## Overview

The Volume System consists of two main layers:

1. **Runtime Volume Detection** - Detects and monitors available storage volumes
2. **Persistent Volume Tracking** - Tracks volumes in the database with user preferences

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Volume Manager │────│  Runtime Volumes │    │ Domain Volumes  │
│                 │    │                  │    │                 │
│ - Detection     │    │ - Live Detection │    │ - Persistence   │
│ - Monitoring    │────│ - Fingerprinting │────│ - User Prefs    │
│ - Caching       │    │ - Events         │    │ - Tracking      │
│ - Events        │    │ - Statistics     │    │ - Library Assoc │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────────┐
                    │     Event Bus       │
                    │                     │
                    │ - VolumeAdded       │
                    │ - VolumeRemoved     │
                    │ - VolumeUpdated     │
                    │ - VolumeMountChanged│
                    └─────────────────────┘
```

## Core Components

### VolumeManager

Central management for all volume operations:

```rust
use spacedrive_core::volume::VolumeManager;

// Get all detected volumes
let volumes = volume_manager.get_all_volumes().await;

// Find volume for a specific path
let volume = volume_manager.volume_for_path(&path).await;

// Track a volume in a library
volume_manager.track_volume(
    &fingerprint,
    &library_ctx,
    Some("My External Drive".to_string())
).await?;
```

### Runtime Volume Types

#### Volume
Represents a detected storage volume:

```rust
pub struct Volume {
    pub fingerprint: VolumeFingerprint,
    pub name: String,
    pub mount_point: PathBuf,
    pub mount_points: Vec<PathBuf>,
    pub mount_type: MountType,
    pub disk_type: DiskType,
    pub file_system: FileSystem,
    pub total_bytes_capacity: u64,
    pub total_bytes_available: u64,
    pub is_mounted: bool,
    pub is_read_only: bool,
    // ... performance metrics
}
```

#### Volume Classification

**Mount Types:**
- `System` - Root filesystem, boot partitions
- `External` - USB drives, external storage
- `Network` - NFS, SMB, cloud mounts

**Disk Types:**
- `SSD` - Solid State Drive
- `HDD` - Hard Disk Drive  
- `Network` - Network storage
- `Virtual` - RAM disk, virtual storage

**Filesystems:**
- `APFS`, `NTFS`, `Ext4`, `Btrfs`, `ZFS`, `ReFS`
- `FAT32`, `ExFAT`, `HFSPlus`
- `Other(String)` - Unknown filesystems

### Volume Fingerprinting

Volumes are uniquely identified using Blake3-based fingerprinting:

```rust
// Fingerprint combines multiple identifiers
let fingerprint = VolumeFingerprint::new(
    &mount_point,
    hardware_id.as_deref(),
    total_capacity,
    &filesystem
);
```

This ensures volumes can be reliably identified even when mount points change.

## Platform Support

### macOS Detection
- Uses `diskutil` and `df` commands
- Detects APFS volumes and mount points
- Identifies SSD vs HDD via `diskutil info`

### Linux Detection  
- Uses `df -h -T` for filesystem information
- Detects disk type via `/sys/block/*/queue/rotational`
- Supports major Linux filesystems

### Windows Detection
- Uses PowerShell `Get-Volume` cmdlet
- Full implementation pending

## Volume Events

The system emits events for volume state changes:

```rust
// Listen for volume events
let mut subscriber = event_bus.subscribe();

while let Ok(event) = subscriber.recv().await {
    match event {
        Event::VolumeAdded(volume) => {
            println!("New volume: {}", volume.name);
        }
        Event::VolumeRemoved { fingerprint } => {
            println!("Volume removed: {}", fingerprint);
        }
        Event::VolumeMountChanged { fingerprint, is_mounted } => {
            println!("Volume {} mount changed: {}", fingerprint, is_mounted);
        }
        _ => {}
    }
}
```

## Volume Tracking

### Runtime vs Tracked Volumes

**Runtime Volumes:**
- Automatically detected by the system
- Temporary - exist only while mounted
- No user customization
- Available to all libraries

**Tracked Volumes:**
- Explicitly tracked by user choice
- Persistent across sessions
- User customizable (names, colors, icons)
- Associated with specific libraries

### Tracking Flow

1. **Volume Detection**
   ```rust
   // Volume is detected and appears in runtime list
   let volumes = volume_manager.get_all_volumes().await;
   ```

2. **User Initiates Tracking**
   ```rust
   // User clicks "Track this volume" in UI
   volume_manager.track_volume(
       &fingerprint,
       &library_context,
       Some("My Photos Drive".to_string())
   ).await?;
   ```

3. **Volume Persisted**
   ```rust
   // Volume saved to database with user preferences
   // Associated with specific library
   // Available across sessions
   ```

### Domain Volume Model

Tracked volumes are stored as domain models:

```rust
pub struct Volume {
    pub id: Uuid,
    pub library_id: Option<Uuid>,
    pub device_id: Uuid,
    pub fingerprint: String,
    pub name: String,
    pub is_tracked: bool,
    
    // User preferences
    pub display_name: Option<String>,
    pub is_favorite: bool,
    pub color: Option<String>,
    pub icon: Option<String>,
    
    // Statistics
    pub total_files: Option<u64>,
    pub total_directories: Option<u64>,
    
    // Performance metrics
    pub read_speed_mbps: Option<u64>,
    pub write_speed_mbps: Option<u64>,
    
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}
```

## Volume Operations

### Core Operations

```rust
// Check if two paths are on the same volume
let same_volume = volume_manager.same_volume(&path1, &path2).await;

// Find volumes with sufficient space
let available_volumes = volume_manager
    .volumes_with_space(required_bytes).await;

// Get volume statistics
let stats = volume_manager.get_statistics().await;

// Run speed test on volume
volume_manager.run_speed_test(&fingerprint).await?;
```

### Volume Utilities

```rust
use spacedrive_core::volume::util;

// Check if path is on specific volume
let is_on_volume = util::is_path_on_volume(&path, &volume);

// Get relative path on volume
let relative = util::relative_path_on_volume(&path, &volume);

// Find best matching volume for path
let volume = util::find_volume_for_path(&path, volumes.iter());
```

## Copy-on-Write Detection

The system detects filesystems supporting copy-on-write operations:

```rust
// Check if volume supports COW
if volume.supports_cow() {
    // Use fast COW copy operations
    perform_cow_copy(&src, &dst).await?;
} else {
    // Fall back to traditional copy
    perform_regular_copy(&src, &dst).await?;
}

// COW filesystems: APFS, Btrfs, ZFS, ReFS
```

## Performance Monitoring

### Speed Testing

```rust
// Test volume read/write speeds
let (read_speed, write_speed) = volume_manager
    .run_speed_test(&fingerprint).await?;

println!("Volume speeds: {}MB/s read, {}MB/s write", 
    read_speed, write_speed);
```

### Cache Management

```rust
// Clear volume path cache
volume_manager.clear_cache().await;

// Get cache statistics
let cache_stats = volume_manager.get_cache_stats().await;
```

## Configuration

### Detection Configuration

```rust
use spacedrive_core::volume::VolumeDetectionConfig;

let config = VolumeDetectionConfig {
    include_system: false,     // Skip system volumes
    include_virtual: false,    // Skip virtual filesystems
    refresh_interval_secs: 30, // Monitor every 30 seconds
};
```

### Volume Manager Setup

```rust
// Initialize volume manager with custom config
let volume_manager = VolumeManager::new(config, event_bus);
volume_manager.initialize().await?;

// Start background monitoring
volume_manager.start_monitoring().await;
```

## Error Handling

### Volume Errors

```rust
use spacedrive_core::volume::VolumeError;

match volume_manager.track_volume(&fingerprint, &ctx, None).await {
    Ok(()) => println!("Volume tracked successfully"),
    Err(VolumeError::NotFound(fp)) => {
        println!("Volume not found: {}", fp);
    }
    Err(VolumeError::Platform(msg)) => {
        println!("Platform error: {}", msg);
    }
    Err(VolumeError::InvalidData(msg)) => {
        println!("Invalid data: {}", msg);
    }
}
```

## Integration Examples

### File Operations Integration

```rust
// Choose optimal copy method based on volume
async fn smart_copy(src: &Path, dst: &Path, volume_manager: &VolumeManager) -> Result<()> {
    let src_volume = volume_manager.volume_for_path(src).await;
    let dst_volume = volume_manager.volume_for_path(dst).await;
    
    match (src_volume, dst_volume) {
        (Some(src_vol), Some(dst_vol)) if src_vol.fingerprint == dst_vol.fingerprint => {
            if src_vol.supports_cow() {
                // Same volume + COW support = instant copy
                perform_cow_copy(src, dst).await
            } else {
                // Same volume = fast move operation
                perform_move_copy(src, dst).await
            }
        }
        _ => {
            // Cross-volume = traditional copy
            perform_cross_volume_copy(src, dst).await
        }
    }
}
```

### Library Integration

```rust
// Track volumes when creating library locations
async fn add_location(
    path: PathBuf,
    library_ctx: &LibraryContext,
    volume_manager: &VolumeManager,
) -> Result<()> {
    // Find volume containing this path
    if let Some(volume) = volume_manager.volume_for_path(&path).await {
        // Suggest tracking the volume
        if !volume_manager.is_volume_tracked(&volume.fingerprint).await? {
            println!("Would you like to track volume '{}'?", volume.name);
            // User confirms...
            volume_manager.track_volume(
                &volume.fingerprint,
                library_ctx,
                None
            ).await?;
        }
    }
    
    // Create location...
}
```

## Best Practices

### Performance
- Use volume-aware operations when possible
- Cache volume lookups for frequently accessed paths
- Leverage COW capabilities for large file operations
- Monitor volume space before operations

### User Experience  
- Show volume tracking suggestions for new external drives
- Display volume capacity and utilization in UI
- Allow custom volume names and organization
- Provide volume performance metrics

### Reliability
- Handle volume disconnection gracefully
- Retry volume detection on errors
- Validate volume fingerprints before operations
- Monitor volume health and space warnings

## Future Enhancements

- **Database Integration** - Full persistence layer implementation
- **Cloud Volume Support** - Detect and manage cloud storage mounts
- **Volume Health Monitoring** - SMART data integration
- **Advanced Speed Testing** - Random I/O, seek times, queue depth testing
- **Volume Synchronization** - Sync volume metadata across devices
- **Volume Groups** - Logical grouping of related volumes