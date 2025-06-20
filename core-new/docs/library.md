# Library System

The library system is the foundation of Spacedrive's data organization. Each library is a self-contained directory that includes all its data, making it portable and easy to manage.

## Architecture

### Library Structure

```
My Photos.sdlibrary/
├── library.json          # Configuration and metadata
├── database.db          # SQLite database
├── thumbnails/          # Thumbnail storage
│   ├── metadata.json    # Thumbnail generation settings
│   └── [sharded dirs]   # Two-level sharding for performance
├── previews/            # Full-size previews (future)
├── indexes/             # Search indexes (future)
├── exports/             # Temporary exports (future)
└── .sdlibrary.lock      # Lock file (when open)
```

### Key Components

1. **LibraryManager**: Handles creation, opening, and discovery of libraries
2. **Library**: Represents an open library with its database and configuration
3. **LibraryLock**: Prevents concurrent access to the same library
4. **LibraryConfig**: Stores library settings and metadata

## Usage

### Creating a Library

```rust
let library = core.libraries
    .create_library("My Photos", None)
    .await?;
```

### Opening a Library

```rust
let library = core.libraries
    .open_library("/path/to/My Photos.sdlibrary")
    .await?;
```

### Discovering Libraries

```rust
let discovered = core.libraries
    .scan_for_libraries()
    .await?;

for lib in discovered {
    println!("Found: {} at {}", lib.config.name, lib.path.display());
}
```

### Working with Thumbnails

```rust
// Save a thumbnail
library.save_thumbnail(cas_id, thumbnail_data).await?;

// Check if thumbnail exists
if library.has_thumbnail(cas_id).await {
    // Get thumbnail data
    let data = library.get_thumbnail(cas_id).await?;
}
```

## Benefits

1. **Portability**: Copy a library folder to backup or move it
2. **Isolation**: Each library is completely independent
3. **Simplicity**: No complex path resolution or scattered data
4. **Flexibility**: Libraries can live anywhere (external drives, cloud folders)
5. **Safety**: Lock files prevent corruption from concurrent access

## Configuration

Libraries store their configuration in `library.json`:

```json
{
  "version": 2,
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Photos",
  "description": "Personal photo collection",
  "settings": {
    "generate_thumbnails": true,
    "thumbnail_quality": 85,
    "thumbnail_sizes": [128, 256, 512],
    "sync_enabled": false
  },
  "statistics": {
    "total_files": 10000,
    "total_size": 50000000000,
    "thumbnail_count": 10000
  }
}
```

## Future Extensions

The self-contained structure allows easy addition of new features:

- **Search Indexes**: Add `indexes/` for full-text and vector search
- **Version History**: Add `versions/` for file versioning
- **Plugins**: Add `plugins/` for library-specific extensions
- **Sync Metadata**: Add `sync/` for multi-device sync data

Each new feature just adds a new directory - no complex migrations needed!