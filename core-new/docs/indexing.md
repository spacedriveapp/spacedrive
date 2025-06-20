# Spacedrive Indexing System

## Overview

The Spacedrive indexing system is a sophisticated, multi-phase file indexing engine designed for high performance and reliability. It discovers, processes, and categorizes files while supporting incremental updates, change detection, and content-based deduplication. The system now supports multiple indexing scopes and ephemeral modes for different use cases.

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
- **Flexible Scoping**: Current (single-level) vs Recursive (full tree) indexing
- **Ephemeral Mode**: In-memory indexing for browsing external paths
- **Persistence Options**: Database storage vs memory-only for different use cases

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
- Direct path storage for efficient queries
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

## Indexing Scopes and Persistence

### Index Scopes

The indexing system supports two different scopes for different use cases:

#### Current Scope
- **Description**: Index only the specified directory (single level)
- **Use Cases**: UI navigation, quick directory browsing, instant feedback
- **Performance**: <500ms for typical directories
- **Implementation**: Direct directory read without recursion

```rust
let config = IndexerJobConfig::ui_navigation(location_id, path);
// Results in single-level scan optimized for UI responsiveness
```

#### Recursive Scope  
- **Description**: Index the directory and all subdirectories
- **Use Cases**: Full location indexing, comprehensive file discovery
- **Performance**: Depends on directory tree size
- **Implementation**: Traditional recursive tree traversal

```rust
let config = IndexerJobConfig::new(location_id, path, mode);
// Default recursive behavior for complete coverage
```

### Persistence Modes

#### Persistent Mode
- **Storage**: Database (SQLite/PostgreSQL)
- **Use Cases**: Managed locations, permanent indexing
- **Features**: Full change detection, resumability, sync support
- **Lifecycle**: Permanent until explicitly removed

```rust
let config = IndexerJobConfig::new(location_id, path, mode);
config.persistence = IndexPersistence::Persistent;
```

#### Ephemeral Mode
- **Storage**: Memory (EphemeralIndex)
- **Use Cases**: External path browsing, temporary exploration
- **Features**: No database writes, session-based caching
- **Lifecycle**: Exists only during application session

```rust
let config = IndexerJobConfig::ephemeral_browse(path, scope);
// Results stored in memory, automatic cleanup
```

### Enhanced Configuration

The new `IndexerJobConfig` provides fine-grained control:

```rust
pub struct IndexerJobConfig {
    pub location_id: Option<Uuid>,      // None for ephemeral
    pub path: SdPath,                   // Path to index
    pub mode: IndexMode,                // Shallow/Content/Deep
    pub scope: IndexScope,              // Current/Recursive
    pub persistence: IndexPersistence,  // Persistent/Ephemeral
    pub max_depth: Option<u32>,         // Depth limiting
}
```

### Use Case Examples

#### UI Directory Navigation
```rust
// Fast current directory scan for UI
let config = IndexerJobConfig::ui_navigation(location_id, path);
// - Scope: Current (single level)
// - Mode: Shallow (metadata only)  
// - Persistence: Persistent
// - Target: <500ms response time
```

#### External Path Browsing
```rust
// Browse USB drive without adding to library
let config = IndexerJobConfig::ephemeral_browse(usb_path, IndexScope::Current);
// - Scope: Current or Recursive
// - Mode: Shallow (configurable)
// - Persistence: Ephemeral
// - Target: Exploration without database pollution
```

#### Background Location Indexing
```rust
// Traditional full location scan
let config = IndexerJobConfig::new(location_id, path, IndexMode::Deep);
// - Scope: Recursive (default)
// - Mode: Deep (full analysis)
// - Persistence: Persistent
// - Target: Complete coverage
```

### Ephemeral Index Structure

The `EphemeralIndex` provides temporary storage:

```rust
pub struct EphemeralIndex {
    pub entries: HashMap<PathBuf, EntryMetadata>,
    pub content_identities: HashMap<String, EphemeralContentIdentity>,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub root_path: PathBuf,
    pub stats: IndexerStats,
}
```

Features:
- **LRU Behavior**: Automatic cleanup based on access time
- **Memory Efficient**: Lightweight metadata storage
- **Session Scoped**: Cleared on application restart
- **Fast Access**: Direct HashMap lookups

## Database Schema

### Core Tables

#### `entries`
The main file/directory entry table using materialized paths:
```sql
- id: i32 (primary key)
- uuid: UUID
- location_id: i32 (→ locations)
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

### Scope Optimizations
- **Current Scope**: Direct directory read without recursion (<500ms target)
- **Recursive Scope**: Efficient tree traversal with depth control
- **Ephemeral Mode**: Memory-only storage for external path browsing
- **Early Termination**: Configurable max_depth limiting

### Caching
- Entry ID cache for parent lookups
- Change detection cache for inode/timestamp comparisons
- Ephemeral index LRU cache for session-based storage
- Content identity cache for deduplication

### Parallelization
- Concurrent CAS ID generation
- Parallel file type detection
- Async I/O operations
- Batch processing across multiple threads

### Database Optimizations
- Bulk inserts with transaction batching
- Prepared statements for repeated operations
- Strategic indexing on location_id and relative_path
- Persistence abstraction for database vs memory storage

## Usage Examples

### Enhanced Indexing Jobs

```rust
use sd_core_new::operations::indexing::{
    IndexerJob, IndexerJobConfig, IndexMode, IndexScope, IndexPersistence
};

// UI Navigation - Fast current directory scan
let config = IndexerJobConfig::ui_navigation(location_id, path);
let job = IndexerJob::new(config);
let handle = library.jobs().dispatch(job).await?;

// Ephemeral Browsing - External path exploration
let config = IndexerJobConfig::ephemeral_browse(external_path, IndexScope::Current);
let job = IndexerJob::new(config);
let handle = library.jobs().dispatch(job).await?;

// Traditional Location Indexing - Full recursive scan
let config = IndexerJobConfig::new(location_id, path, IndexMode::Deep);
let job = IndexerJob::new(config);
let handle = library.jobs().dispatch(job).await?;

// Custom Configuration - Fine-grained control
let mut config = IndexerJobConfig::new(location_id, path, IndexMode::Content);
config.scope = IndexScope::Current;
config.max_depth = Some(2);
let job = IndexerJob::new(config);
```

### Legacy API (Backward Compatibility)

```rust
// Old API still works for simple cases
let job = IndexerJob::from_location(location_id, path, IndexMode::Deep);
let job = IndexerJob::shallow(location_id, path);
let job = IndexerJob::with_content(location_id, path);
```

### Indexing Modes

- **Shallow**: Metadata only (fastest, <500ms for UI)
- **Content**: Includes CAS ID generation (moderate performance)
- **Deep**: Full analysis including thumbnails (comprehensive)

### Indexing Scopes

- **Current**: Single directory level (UI navigation, quick browsing)
- **Recursive**: Full directory tree (complete location indexing)

### Persistence Options

- **Persistent**: Database storage (managed locations, permanent data)
- **Ephemeral**: Memory storage (external browsing, temporary exploration)

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

// Enhanced configuration with scope and persistence
IndexerJobConfig {
    location_id: Option<Uuid>,         // None for ephemeral jobs
    path: SdPath,                      // Target path
    mode: IndexMode,                   // Shallow/Content/Deep
    scope: IndexScope,                 // Current/Recursive
    persistence: IndexPersistence,     // Persistent/Ephemeral
    max_depth: Option<u32>,            // Depth limiting for performance
}

// Ephemeral index settings
EphemeralConfig {
    max_entries: usize,                // Default: 10000
    cleanup_interval: Duration,        // Default: 5 minutes
    max_idle_time: Duration,           // Default: 30 minutes
    enable_content_analysis: bool,     // Default: false
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

## CLI Usage

The indexing system provides comprehensive CLI access with enhanced scope and persistence options:

### Enhanced Index Commands

```bash
# Start the daemon first
spacedrive start

# Quick scan for UI navigation (fast, current directory only)
spacedrive index quick-scan ~/Documents --scope current

# Quick scan with ephemeral mode (no database writes)
spacedrive index quick-scan /external/drive --scope current --ephemeral

# Browse external paths without adding to managed locations
spacedrive index browse /media/usb-drive --scope current
spacedrive index browse /network/share --scope recursive --content

# Index managed locations with specific scope and mode
spacedrive index location ~/Pictures --scope current --mode shallow
spacedrive index location <location-uuid> --scope recursive --mode deep
```

### Location Management

```bash
# Add locations with different indexing modes
spacedrive location add ~/Documents --mode shallow    # Fast metadata only
spacedrive location add ~/Pictures --mode content     # With content hashing 
spacedrive location add ~/Videos --mode deep          # Full media analysis

# Force re-indexing of a location
spacedrive location rescan <location-id> --force
```

### Legacy Commands (Backward Compatibility)

```bash
# Traditional indexing (creates location and starts full scan)
spacedrive scan ~/Desktop --mode content --watch
```

### Monitoring and Status

```bash
# Monitor indexing progress in real-time
spacedrive job monitor

# Check job status with scope/persistence info
spacedrive job list --status running

# Get detailed job information
spacedrive job info <job-id>
```

### Command Comparison

| Command | Scope | Persistence | Use Case |
|---------|-------|-------------|----------|
| `index quick-scan` | Current/Recursive | Persistent/Ephemeral | UI navigation, quick browsing |
| `index browse` | Current/Recursive | Ephemeral | External path exploration |
| `index location` | Current/Recursive | Persistent | Managed location updates |
| `scan` (legacy) | Recursive | Persistent | Traditional full indexing |
| `location add` | Recursive | Persistent | Add new managed locations |

For complete CLI documentation, see [CLI Documentation](./cli.md).

## Debugging

Enable detailed logging:
```bash
# For CLI daemon
spacedrive start --foreground -v

# For development
RUST_LOG=sd_core_new::operations::indexing=debug cargo run
```

Common issues:
1. **Slow indexing**: Check filter rules and batch sizes
2. **High memory usage**: Reduce batch size  
3. **Missing files**: Verify permissions and filter rules
4. **No progress shown**: Ensure daemon is running and use `spacedrive job monitor`