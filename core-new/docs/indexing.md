# Spacedrive Indexing System

## Overview

The Spacedrive indexing system is a sophisticated, multi-phase file indexing engine designed for high performance and reliability. It discovers, processes, and categorizes files while supporting incremental updates, change detection, and content-based deduplication.

## Architecture

### Core Components

1. **IndexerJob** - The main job orchestrator that manages the indexing process
2. **IndexerState** - Maintains state across phases for resumability
3. **EntryProcessor** - Handles database operations for file entries
4. **FileTypeRegistry** - Identifies file types through extensions and magic bytes
5. **CasGenerator** - Creates content-addressed storage identifiers

### Key Features

- **Multi-phase Processing**: Discovery → Processing → Aggregation → Content Identification
- **Resumable Operations**: Jobs can be paused and resumed from checkpoints
- **Change Detection**: Efficiently identifies modified files using inode tracking
- **Content Deduplication**: Uses CAS (Content-Addressed Storage) IDs
- **Type Detection**: Sophisticated file type identification with MIME type support
- **Performance Optimized**: Batch processing, caching, and parallel operations

## Indexing Phases

### 1. Discovery Phase
Walks the file system to discover all files and directories.

```rust
// Key operations:
- Recursive directory traversal
- Filter application (skip system files, hidden files based on rules)
- Batch collection for efficient processing
- Progress tracking and reporting
```

**Output**: Batches of `DirEntry` items ready for processing

### 2. Processing Phase
Creates or updates database entries for discovered items.

```rust
// Key operations:
- Change detection using inode/modified time
- Materialized path storage (no parent_id needed)
- Entry creation/update in database
- Path prefix optimization
```

**Output**: Database entries with proper relationships

### 3. Aggregation Phase
Calculates aggregate statistics for directories.

```rust
// Key operations:
- Bottom-up traversal of directory tree
- Calculate total sizes and file counts
- Update directory entries with aggregate data
```

**Output**: Directories with accurate size/count statistics

### 4. Content Identification Phase
Generates content identifiers and detects file types.

```rust
// Key operations:
- CAS ID generation (sampled hashing)
- File type detection (extension + magic bytes)
- MIME type identification
- Content deduplication
```

**Output**: Content identities linked to entries

## Database Schema

### Core Tables

#### `entries`
The main file/directory entry table using materialized paths:
```sql
- id: i32 (primary key)
- uuid: UUID
- prefix_id: i32 (→ path_prefixes)
- relative_path: String (materialized path - parent directory path)
- name: String (filename without extension)
- kind: i32 (0=File, 1=Directory, 2=Symlink)
- extension: String?
- size: i64
- aggregate_size: i64 (for directories)
- child_count: i32
- file_count: i32
- inode: i64? (for change detection)
- location_id: i32? (→ locations)
- content_id: i32? (→ content_identities)
- metadata_id: i32? (→ user_metadata)
```

**Note**: Parent-child relationships are determined by the `relative_path` field. For example:
- A file at `/documents/report.pdf` has `relative_path = "documents"` and `name = "report"`
- Its parent directory has `relative_path = ""` and `name = "documents"`

#### `content_identities`
Stores unique content for deduplication:
```sql
- id: i32 (primary key)
- uuid: UUID
- cas_id: String (content hash)
- cas_version: i16
- kind_id: i32 (→ content_kinds)
- mime_type_id: i32? (→ mime_types)
- total_size: i64
- entry_count: i32 (number of files with this content)
```

#### `content_kinds`
Static lookup table for content types:
```sql
- id: i32 (primary key, matches enum)
- name: String

Values:
0 = unknown
1 = image
2 = video
3 = audio
4 = document
5 = archive
6 = code
7 = text
8 = database
9 = book
10 = font
11 = mesh
12 = config
13 = encrypted
14 = key
15 = executable
16 = binary
```

#### `mime_types`
Dynamic table for discovered MIME types:
```sql
- id: i32 (primary key)
- uuid: UUID (for syncing)
- mime_type: String (unique)
- created_at: DateTime
```

#### `path_prefixes`
Optimizes path storage:
```sql
- id: i32 (primary key)
- device_id: i32 (→ devices)
- prefix: String (e.g., "/Users/john/Documents")
```

## File Type Detection

The system uses a multi-layered approach:

1. **Extension Matching**: Fast initial identification
2. **Magic Bytes**: Verifies file type by reading file headers
3. **Content Analysis**: For text files, analyzes content patterns
4. **MIME Type Detection**: Associates standard MIME types

Example flow:
```rust
let registry = FileTypeRegistry::default();
let result = registry.identify(path).await?;
// Returns: FileType with category, MIME types, and confidence level
```

## Content Addressing (CAS)

The CAS system creates unique identifiers for file content:

1. **Sampled Hashing**: Reads chunks at specific offsets
2. **Blake3 Hashing**: Fast, cryptographically secure
3. **Deduplication**: Same content = same CAS ID

Benefits:
- Instant duplicate detection
- Content verification
- Efficient storage references

## Change Detection

The indexer efficiently detects changes using:

1. **Inode Tracking**: Platform-specific file identifiers
2. **Modified Time**: Fallback for systems without inodes
3. **Size Comparison**: Quick change indicator

Change types detected:
- New files
- Modified files
- Deleted files
- Moved files (same inode, different path)

## Performance Optimizations

### Batch Processing
- Processes files in chunks of 1000
- Reduces database round trips
- Improves memory efficiency

### Caching
- Path prefix cache
- Parent entry ID cache
- Change detection cache

### Parallelization
- Concurrent CAS ID generation
- Parallel file type detection
- Async I/O operations

### Database Optimizations
- Bulk inserts
- Prepared statements
- Strategic indexing

## Usage Examples

### Starting an Indexing Job

```rust
use sd_core_new::operations::indexing::{IndexerJob, IndexMode};

// Create indexer job
let job = IndexerJob::new(
    location_id,
    location_path,
    IndexMode::Deep, // Shallow, Content, or Deep
);

// Dispatch through job manager
let handle = library.jobs().dispatch(job).await?;

// Monitor progress
while let Some(progress) = handle.progress().await {
    println!("Progress: {:?}", progress);
}
```

### Indexing Modes

- **Shallow**: Metadata only (fastest)
- **Content**: Includes CAS ID generation
- **Deep**: Full analysis including thumbnails (future)

## Metrics and Monitoring

The indexer tracks detailed metrics:

```rust
IndexerMetrics {
    total_items: u64,
    items_per_second: f64,
    bytes_per_second: f64,
    phase_durations: HashMap<String, Duration>,
    db_operations: (reads: u64, writes: u64),
    cache_stats: CacheStats,
}
```

## Error Handling

### Critical Errors
Stop indexing immediately:
- Database connection lost
- Filesystem errors
- Permission denied on location root

### Non-Critical Errors
Logged but indexing continues:
- Permission denied on individual files
- Corrupted file metadata
- Unsupported file types

## Future Enhancements

1. **Thumbnail Generation**: Integrated media thumbnail creation
2. **Full-Text Indexing**: Search within documents
3. **AI Tagging**: Automatic content categorization
4. **Cloud Integration**: Index cloud storage locations
5. **Real-time Monitoring**: Instant file change detection
6. **Distributed Indexing**: Multi-device collaborative indexing

## Configuration

### Filter Rules
```rust
IndexerRules {
    skip_hidden: bool,
    skip_system: bool,
    max_file_size: Option<u64>,
    allowed_extensions: Option<Vec<String>>,
    ignored_paths: Vec<PathBuf>,
}
```

### Performance Tuning
```rust
IndexerConfig {
    batch_size: usize,        // Default: 1000
    checkpoint_interval: u64, // Default: 5000 items
    max_concurrent_io: usize, // Default: 100
    enable_content_id: bool,  // Default: true
}
```

## Integration Points

The indexer integrates with:

1. **Location System**: Manages indexed locations
2. **Job System**: Provides resumability and progress
3. **Event System**: Emits progress and completion events
4. **Sync System**: Shares indexed data across devices
5. **Search System**: Powers file search functionality

## Best Practices

1. **Start with Shallow Mode**: For initial quick results
2. **Use Filters**: Skip unnecessary files (node_modules, etc.)
3. **Monitor Progress**: Subscribe to indexing events
4. **Handle Errors Gracefully**: Check non-critical error counts
5. **Regular Re-indexing**: Schedule periodic deep scans

## Technical Details

### State Persistence
The indexer state is serialized using MessagePack for efficient storage and quick resume operations.

### Memory Management
- Streaming file processing (no full file loads)
- Bounded channels for backpressure
- Automatic batch flushing

### Platform Support
- **Windows**: Uses file index for inode equivalent
- **macOS**: Native inode support
- **Linux**: Full inode and permission tracking

## Debugging

Enable detailed logging:
```bash
RUST_LOG=sd_core_new::operations::indexing=debug cargo run
```

Common issues:
1. **Slow indexing**: Check filter rules and batch sizes
2. **High memory usage**: Reduce batch size
3. **Missing files**: Verify permissions and filter rules