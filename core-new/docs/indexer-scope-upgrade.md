# Indexer Scope and Ephemeral Mode Upgrade

## Overview

This document outlines the design for upgrading the Spacedrive indexer to support different indexing scopes and ephemeral modes. The current indexer operates with a single recursive mode within managed locations. This upgrade introduces more granular control for UI responsiveness and support for viewing unmanaged paths.

## Current State

The indexer currently supports:
- **IndexMode**: Shallow, Quick, Content, Deep, Full (determines what data to extract)
- **Location-based**: Only indexes within managed locations
- **Persistent**: All operations write to database
- **Recursive**: Always scans entire directory trees

## Proposed Enhancements

### 1. IndexScope Enum

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexScope {
    /// Index only the current directory (single level)
    Current,
    /// Index recursively through all subdirectories
    Recursive,
}
```

### 2. IndexPersistence Enum

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexPersistence {
    /// Write all results to database (normal operation)
    Persistent,
    /// Keep results in memory only (for unmanaged paths)
    Ephemeral,
}
```

### 3. Enhanced IndexerJob Configuration

```rust
pub struct IndexerJobConfig {
    pub location_id: Option<Uuid>,  // None for ephemeral indexing
    pub path: SdPath,
    pub mode: IndexMode,
    pub scope: IndexScope,
    pub persistence: IndexPersistence,
    pub max_depth: Option<u32>,  // Override for Current scope
}
```

## Use Cases

### Use Case 1: UI Directory Navigation
**Scenario**: User navigates to a folder in the UI and needs current contents displayed immediately.

**Requirements**:
- IndexScope: Current
- IndexMode: Quick (metadata only)
- IndexPersistence: Persistent (update database)
- Fast response time (<500ms for typical directories)

**Implementation**:
```rust
let config = IndexerJobConfig {
    location_id: Some(location_uuid),
    path: current_directory_path,
    mode: IndexMode::Quick,
    scope: IndexScope::Current,
    persistence: IndexPersistence::Persistent,
    max_depth: Some(1),
};
```

### Use Case 2: Ephemeral Path Browsing
**Scenario**: User wants to browse a directory outside of managed locations (e.g., network drive, external device).

**Requirements**:
- IndexScope: Current or Recursive
- IndexMode: Quick or Content
- IndexPersistence: Ephemeral (no database writes)
- Results cached in memory for session

**Implementation**:
```rust
let config = IndexerJobConfig {
    location_id: None,  // Not a managed location
    path: external_path,
    mode: IndexMode::Quick,
    scope: IndexScope::Current,
    persistence: IndexPersistence::Ephemeral,
    max_depth: Some(1),
};
```

### Use Case 3: Background Full Indexing
**Scenario**: Traditional full location indexing for new or updated locations.

**Requirements**:
- IndexScope: Recursive
- IndexMode: Deep or Full
- IndexPersistence: Persistent
- Complete coverage of location

**Implementation**:
```rust
let config = IndexerJobConfig {
    location_id: Some(location_uuid),
    path: location_root_path,
    mode: IndexMode::Deep,
    scope: IndexScope::Recursive,
    persistence: IndexPersistence::Persistent,
    max_depth: None,
};
```

## Technical Implementation

### 1. Enhanced IndexerJob Structure

```rust
pub struct IndexerJob {
    config: IndexerJobConfig,
    // Internal state
    ephemeral_results: Option<Arc<RwLock<EphemeralIndex>>>,
}

pub struct EphemeralIndex {
    entries: HashMap<PathBuf, EntryMetadata>,
    content_identities: HashMap<String, ContentIdentity>,
    created_at: Instant,
    last_accessed: Instant,
}
```

### 2. Modified Discovery Phase

```rust
impl IndexerJob {
    async fn discovery_phase(&mut self, state: &mut IndexerState, ctx: &JobContext<'_>) -> JobResult<()> {
        match self.config.scope {
            IndexScope::Current => {
                // Only scan immediate children
                self.scan_single_level(state, ctx).await?;
            }
            IndexScope::Recursive => {
                // Existing recursive logic
                self.scan_recursive(state, ctx).await?;
            }
        }
        Ok(())
    }

    async fn scan_single_level(&mut self, state: &mut IndexerState, ctx: &JobContext<'_>) -> JobResult<()> {
        let root_path = self.config.path.as_local_path()
            .ok_or_else(|| JobError::execution("Path not accessible locally"))?;

        let mut entries = fs::read_dir(root_path).await
            .map_err(|e| JobError::execution(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| JobError::execution(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            let metadata = entry.metadata().await
                .map_err(|e| JobError::execution(format!("Failed to read metadata: {}", e)))?;

            let dir_entry = DirEntry {
                path: path.clone(),
                kind: if metadata.is_dir() { EntryKind::Directory } 
                      else if metadata.is_symlink() { EntryKind::Symlink }
                      else { EntryKind::File },
                size: metadata.len(),
                modified: metadata.modified().ok(),
                inode: EntryProcessor::get_inode(&metadata),
            };

            state.pending_entries.push(dir_entry);
            
            // Update stats
            match dir_entry.kind {
                EntryKind::File => state.stats.files += 1,
                EntryKind::Directory => state.stats.dirs += 1,
                EntryKind::Symlink => state.stats.symlinks += 1,
            }
        }

        Ok(())
    }
}
```

### 3. Persistence Layer Abstraction

```rust
trait IndexPersistence {
    async fn store_entry(&self, entry: &DirEntry, location_id: Option<i32>) -> JobResult<i32>;
    async fn store_content_identity(&self, cas_id: &str, content_data: &ContentData) -> JobResult<i32>;
    async fn get_existing_entries(&self, path: &Path) -> JobResult<Vec<ExistingEntry>>;
}

struct DatabasePersistence<'a> {
    ctx: &'a JobContext<'a>,
    location_id: i32,
}

struct EphemeralPersistence {
    index: Arc<RwLock<EphemeralIndex>>,
}
```

### 4. Enhanced Progress Reporting

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerProgress {
    pub phase: IndexPhase,
    pub scope: IndexScope,
    pub persistence: IndexPersistence,
    pub current_path: String,
    pub total_found: IndexerStats,
    pub processing_rate: f32,
    pub estimated_remaining: Option<Duration>,
    pub is_ephemeral: bool,
}
```

## CLI Integration

### New CLI Commands

```bash
# Quick scan of current directory only
spacedrive index quick-scan /path/to/directory --scope current

# Ephemeral browse of external path
spacedrive browse /media/external-drive --ephemeral

# Traditional full location indexing
spacedrive index location /managed/location --scope recursive --mode deep
```

### CLI Implementation

```rust
#[derive(Subcommand)]
pub enum IndexCommands {
    /// Quick scan of a directory
    QuickScan {
        path: PathBuf,
        #[arg(long, default_value = "current")]
        scope: String,
        #[arg(long)]
        ephemeral: bool,
    },
    /// Browse external paths without persistence
    Browse {
        path: PathBuf,
        #[arg(long, default_value = "current")]
        scope: String,
    },
}
```

## Performance Considerations

### 1. Current Scope Optimization
- **Target**: <500ms response time for typical directories
- **Techniques**:
  - Parallel metadata extraction
  - Async I/O with tokio
  - Batch database operations
  - Skip content analysis for Quick mode

### 2. Ephemeral Index Management
- **Memory Management**: LRU cache with configurable size limits
- **Session Persistence**: Keep ephemeral results for UI session duration
- **Cleanup**: Automatic cleanup of old ephemeral indexes

### 3. Database Impact
- **Current Scope**: Minimal database writes (only changed entries)
- **Batch Operations**: Group database operations for efficiency
- **Indexing Strategy**: Optimized queries for single-level scans

## Error Handling

### Scope-Specific Errors
```rust
#[derive(Debug, thiserror::Error)]
pub enum IndexScopeError {
    #[error("Directory not accessible for current scope scan: {path}")]
    CurrentScopeAccessDenied { path: PathBuf },
    
    #[error("Ephemeral index limit exceeded (max: {max}, current: {current})")]
    EphemeralIndexLimitExceeded { max: usize, current: usize },
    
    #[error("Cannot perform recursive scan on ephemeral path: {path}")]
    EphemeralRecursiveNotAllowed { path: PathBuf },
}
```

## Migration Strategy

### Phase 1: Core Infrastructure
1. Add new enums (IndexScope, IndexPersistence)
2. Extend IndexerJobConfig
3. Create persistence abstraction layer
4. Implement Current scope scanning

### Phase 2: Ephemeral Support
1. Implement EphemeralIndex structure
2. Add ephemeral persistence layer
3. Create memory management for ephemeral indexes
4. Add session-based cleanup

### Phase 3: UI Integration
1. Modify location browser to use Current scope
2. Add ephemeral path browsing capabilities
3. Implement progress indicators for different scopes
4. Add user preferences for scope selection

### Phase 4: CLI Enhancement
1. Add new CLI commands
2. Extend existing commands with scope options
3. Add ephemeral browsing commands
4. Update help documentation

## Testing Strategy

### Unit Tests
- IndexScope enum conversions
- EphemeralIndex operations
- Persistence layer implementations
- Current scope discovery logic

### Integration Tests
- End-to-end Current scope indexing
- Ephemeral index lifecycle
- CLI command variations
- Performance benchmarks

### Performance Tests
- Current scope response time targets
- Memory usage of ephemeral indexes
- Database operation efficiency
- Concurrent indexing scenarios

## Future Enhancements

### 1. Smart Scope Selection
Automatically choose optimal scope based on:
- Directory size
- User access patterns
- System resources
- Network latency (for remote paths)

### 2. Incremental Current Scope Updates
- Watch filesystem events for current directories
- Incrementally update UI without full re-scan
- Batch updates for efficiency

### 3. Cross-Device Ephemeral Browsing
- Browse remote device paths
- Network-aware ephemeral caching
- Offline capability for cached paths

### 4. Machine Learning Integration
- Predict optimal IndexMode based on file types
- Learn user browsing patterns
- Optimize scope selection automatically

## Conclusion

This upgrade provides the foundation for more responsive UI interactions while maintaining the robust indexing capabilities of Spacedrive. The separation of concerns between scope, mode, and persistence allows for flexible combinations that serve different use cases without compromising performance or functionality.

The implementation maintains backward compatibility while opening new possibilities for user experience improvements and system efficiency gains.