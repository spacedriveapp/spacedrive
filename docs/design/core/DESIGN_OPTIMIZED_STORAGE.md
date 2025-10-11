# Storage Design

## Problem Statement

File system storage needs to balance several concerns:
- **Space efficiency** - Minimize database size for large collections
- **Query performance** - Fast path-based operations
- **Simplicity** - Avoid complex joins for common operations
- **Cross-device compatibility** - Work consistently across devices

## Solution: Materialized Path Storage

### 1. Integer IDs for Internal Storage
- Use auto-incrementing integers internally (4-8 bytes)
- Keep UUIDs only for external APIs and cross-device sync
- 75% reduction in ID storage size

### 2. Materialized Path Approach
Simple and efficient path storage:

```sql
-- Store paths directly with materialized hierarchy:
entries: location_id=1, relative_path="src", name="main.rs"
entries: location_id=1, relative_path="src", name="lib.rs"
entries: location_id=1, relative_path="", name="Cargo.toml"
```

### 3. Benefits

**Performance:**
- **Simple queries** - No joins needed for most path operations
- **Fast hierarchy queries** - Direct LIKE patterns on relative_path
- **Efficient indexing** - Single index covers most queries

**Simplicity:**
- **No complex relationships** - Avoid recursive parent_id patterns
- **Direct path access** - Build full paths with simple concatenation
- **Easy migrations** - Straightforward schema changes

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

-- Entries (materialized paths)
CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    location_id INTEGER NOT NULL,    -- Reference to location
    relative_path TEXT NOT NULL,     -- Directory path within location
    name TEXT NOT NULL,              -- File/directory name
    metadata_id INTEGER NOT NULL,
    -- ... other fields
);
```

### API Translation Layer

```rust
// External API uses UUIDs and SdPath
pub struct SdPath {
    pub device_id: Uuid,
    pub path: PathBuf,
}

// Internal storage uses integers and materialized paths
pub struct EntryStorage {
    pub id: i64,
    pub location_id: i32,
    pub relative_path: String,
    pub name: String,
}

// Translation happens at API boundary
impl Entry {
    pub fn to_sdpath(&self) -> SdPath {
        let location = self.get_location();
        let device = location.get_device();
        
        let full_path = if self.relative_path.is_empty() {
            PathBuf::from(&self.name)
        } else {
            PathBuf::from(&self.relative_path).join(&self.name)
        };
        
        SdPath {
            device_id: device.uuid,
            path: PathBuf::from(&location.path).join(full_path),
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