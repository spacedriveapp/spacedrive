# Optimized Storage Design

## Problem Statement

Storing UUIDs repeatedly for millions of files creates massive database bloat:
- Each UUID is 16 bytes (binary) or 36 bytes (string)
- With millions of files, device_id and location_id alone can consume gigabytes
- Serialized JSON paths add even more overhead

## Solution: Hybrid ID System

### 1. Integer IDs for Internal Storage
- Use auto-incrementing integers internally (4-8 bytes)
- Keep UUIDs only for external APIs and cross-device sync
- 75% reduction in ID storage size

### 2. Path Prefix Interning
Cross-device compatible path compression:

```sql
-- Instead of storing full paths repeatedly:
/Users/jamie/Documents/Projects/spacedrive/src/main.rs
/Users/jamie/Documents/Projects/spacedrive/src/lib.rs
/Users/jamie/Documents/Projects/spacedrive/Cargo.toml

-- Store prefix once:
path_prefixes: id=1, device_id=1, prefix="/Users/jamie/Documents/Projects/spacedrive"

-- Then store only:
entries: prefix_id=1, relative_path="src/main.rs"
entries: prefix_id=1, relative_path="src/lib.rs"  
entries: prefix_id=1, relative_path="Cargo.toml"
```

### 3. Size Comparison

For 1 million files across 3 devices:

**Before optimization:**
- UUID device_id: 16 bytes × 1M = 16 MB
- UUID location_id: 16 bytes × 1M = 16 MB
- Serialized SdPath JSON: ~120 bytes × 1M = 120 MB
- **Total: ~152 MB**

**After optimization:**
- Integer device_id: 4 bytes × 1M = 4 MB
- Integer location_id: 4 bytes × 1M = 4 MB
- Prefix ID + relative path: ~35 bytes × 1M = 35 MB
- **Total: ~43 MB (72% reduction!)**

## Implementation Details

### Database Schema

```sql
-- Devices table (hybrid)
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,      -- Internal use
    uuid BLOB NOT NULL UNIQUE,   -- External API
    name TEXT NOT NULL,
    -- ... other fields
);

-- Path prefixes (cross-device)
CREATE TABLE path_prefixes (
    id INTEGER PRIMARY KEY,
    device_id INTEGER NOT NULL,
    prefix TEXT NOT NULL,
    UNIQUE(device_id, prefix)
);

-- Entries (optimized)
CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    prefix_id INTEGER NOT NULL,      -- Includes device info!
    relative_path TEXT NOT NULL,     -- Just the relative part
    metadata_id INTEGER NOT NULL,
    -- ... other fields
);
```

### API Translation Layer

```rust
// External API uses UUIDs
pub struct SdPath {
    pub device_id: Uuid,
    pub path: PathBuf,
}

// Internal storage uses integers
pub struct EntryStorage {
    pub id: i64,
    pub prefix_id: i32,
    pub relative_path: String,
}

// Translation happens at API boundary
impl Entry {
    pub fn to_sdpath(&self) -> SdPath {
        let prefix = self.get_prefix();
        let device = self.get_device();
        SdPath {
            device_id: device.uuid,
            path: prefix.join(&self.relative_path),
        }
    }
}
```

## Benefits

1. **Massive space savings** - 70%+ reduction in path storage
2. **Faster queries** - Smaller indexes, better cache utilization
3. **Cross-device compatible** - Prefix includes device information
4. **Backward compatible** - UUIDs preserved for external APIs
5. **Future proof** - Easy to add more optimizations

## Migration Strategy

1. Add integer columns alongside UUID columns
2. Build prefix table from existing data
3. Gradually migrate queries to use integer IDs
4. Keep UUID columns for external compatibility

## Sync Considerations

- UUIDs remain the canonical identifier for sync
- Integer IDs are device-local for performance
- Prefix table syncs as part of device metadata
- No changes to sync protocol required