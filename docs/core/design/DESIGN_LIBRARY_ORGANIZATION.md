# Library Data Organization Design

## Current Problems

The existing Spacedrive library organization has critical flaws:

1. **Scattered Data**: Library data is spread across multiple directories (database, thumbnails, etc.)
2. **Backup Nightmare**: Can't easily backup/restore a complete library
3. **Instance Dependencies**: File paths tied to instance IDs break portability
4. **No Isolation**: Libraries share directories, making management difficult
5. **Migration Hell**: Complex migrations needed for any organizational change

## Design Principles

1. **Self-Contained**: Each library is a complete, portable directory
2. **Location Agnostic**: Libraries can be moved/copied without breaking
3. **Backup Friendly**: Simple directory copy = complete backup
4. **Future Proof**: Extensible structure for new data types
5. **Migration Free**: New features add directories, don't reorganize existing ones

## Proposed Directory Structure

```
~/Spacedrive/Libraries/                    # Default libraries root (configurable)
├── My Photos.sdlibrary/                   # Self-contained library directory
│   ├── library.json                       # Library metadata & config
│   ├── database.db                        # SQLite database
│   ├── database.db-wal                    # Write-ahead log
│   ├── database.db-shm                    # Shared memory
│   ├── thumbnails/                        # All thumbnails for this library
│   │   ├── [0-9a-f]/                     # First char of cas_id (16 dirs)
│   │   │   ├── [0-9a-f]/                 # Second char (256 dirs total)
│   │   │   │   └── {cas_id}.webp        # Actual thumbnail files
│   │   └── metadata.json                 # Thumbnail generation settings
│   ├── previews/                         # Full-size previews (future)
│   │   └── [similar structure]
│   ├── indexes/                          # Search indexes (future)
│   │   ├── text.idx                     # Full-text search index
│   │   └── embeddings.idx               # Vector embeddings
│   ├── exports/                          # Exported data (future)
│   ├── plugins/                          # Library-specific plugins (future)
│   └── .sdlibrary.lock                   # Lock file for concurrent access
```

## Key Design Decisions

### 1. Library Directory Naming
```
{library_name}.sdlibrary/
```
- Human-readable directory names (not UUIDs)
- `.sdlibrary` extension marks it as a Spacedrive library
- Allows users to identify libraries in file explorers
- Internal UUID stored in `library.json`

### 2. Library Metadata File (`library.json`)
```json
{
  "version": 2,
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Photos",
  "description": "Personal photo collection",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "settings": {
    "generate_thumbnails": true,
    "thumbnail_quality": 85,
    "enable_ai_tagging": false
  },
  "statistics": {
    "total_files": 10000,
    "total_size": 50000000000,
    "last_indexed": "2024-01-01T00:00:00Z"
  }
}
```

### 3. Thumbnail Organization
- Two-level sharding using first two characters of cas_id
- Creates maximum 256 directories (16 × 16)
- Balances between too many files per directory and too many directories
- Metadata file stores generation parameters for consistency

### 4. Database Design Changes
- Remove instance_id dependencies from file_path table
- Use relative paths within locations instead of absolute
- Store device information separately from path information
- Enable true portability between devices

### 5. Lock File for Concurrency
- `.sdlibrary.lock` prevents multiple nodes from accessing simultaneously
- Contains node information and process ID
- Automatically cleaned up on graceful shutdown
- Stale lock detection for crash recovery

## Library Locations Configuration

```json
// In node configuration
{
  "library_locations": [
    {
      "path": "~/Spacedrive/Libraries",
      "is_default": true
    },
    {
      "path": "/Volumes/External/SpacedriveLibraries",
      "is_default": false
    }
  ]
}
```

## Migration Strategy

### From v1 to v2

1. **Create New Structure**:
   ```rust
   async fn migrate_library_v1_to_v2(old_lib_id: Uuid) -> Result<()> {
       // Create new directory structure
       let old_config = load_v1_config(old_lib_id)?;
       let new_dir = create_v2_directory(&old_config.name)?;
       
       // Copy and migrate database
       migrate_database(old_lib_id, &new_dir).await?;
       
       // Migrate thumbnails with progress
       migrate_thumbnails(old_lib_id, &new_dir).await?;
       
       // Create v2 config
       create_v2_config(&new_dir, old_config)?;
       
       Ok(())
   }
   ```

2. **Gradual Migration**:
   - Keep v1 libraries functional during migration
   - Migrate one library at a time
   - Verify integrity before removing old data
   - Provide rollback capability

## Implementation Benefits

### 1. Simple Backups
```bash
# Complete library backup
cp -r "My Photos.sdlibrary" /backup/location/

# Works with any backup software
rsync -av "My Photos.sdlibrary" remote:/backup/
```

### 2. Easy Library Management
```bash
# Move library to external drive
mv "My Photos.sdlibrary" /Volumes/External/

# Share library with another user
zip -r photos.zip "My Photos.sdlibrary"
```

### 3. Multi-Library Workflows
- Open libraries from any location
- Different libraries on different drives
- Temporary libraries on removable media
- Archive old libraries without cluttering active workspace

### 4. Cloud Sync Ready
- Self-contained directories work well with cloud storage
- Can sync entire library or just metadata
- Conflict resolution simplified

## API Changes

### Opening Libraries
```rust
// Old: Libraries identified by UUID only
let library = libraries.get(library_id)?;

// New: Libraries identified by path or UUID
let library = libraries.open_path("/path/to/My Photos.sdlibrary")?;
let library = libraries.open_id(library_id)?; // Still supported
```

### Library Discovery
```rust
// Scan for libraries in configured locations
let discovered = libraries.scan_locations().await?;

// Register external library
libraries.register_external("/mnt/nas/Shared Media.sdlibrary")?;
```

## Future Extensions

The structure supports future additions without breaking changes:

1. **Indexes Directory**: For full-text and vector search indexes
2. **Previews Directory**: For full-resolution preview generation
3. **Exports Directory**: For temporary export operations
4. **Plugins Directory**: For library-specific extensions
5. **Sync Directory**: For sync metadata and conflict resolution
6. **Versions Directory**: For file version history (future feature)

## Security Considerations

1. **Permissions**: Library directory permissions restrict access
2. **Encryption**: Optional library encryption at directory level
3. **Lock Files**: Prevent concurrent access corruption
4. **Integrity**: Checksums for critical files

## Conclusion

This design solves the fundamental issues with Spacedrive's current library organization:
- ✅ Complete portability and backup capability
- ✅ No instance/device dependencies
- ✅ Human-friendly organization
- ✅ Future-proof extensibility
- ✅ Simple implementation and maintenance

The self-contained library directory is the foundation for reliable, user-friendly file management across devices.