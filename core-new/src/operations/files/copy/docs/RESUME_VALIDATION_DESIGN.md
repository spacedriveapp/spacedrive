# Resume Validation Design: Destination State Verification

## Overview

This document outlines a design for robust resume validation in the copy module using the existing ephemeral indexing infrastructure to verify destination state before resuming operations.

## Problem Statement

With the current resume logic fix, completed files are tracked in `completed_indices` and skipped on resume. However, there's a potential issue where the destination state might not match our expectations:

### Edge Cases
1. **Manual file deletion**: User deletes destination files after successful copy but before checkpoint
2. **External modification**: Another process modifies/moves destination files
3. **Filesystem corruption**: Destination files become corrupted or inaccessible
4. **Partial writes**: Previous copy was interrupted mid-file, leaving partial data

### Current Approach Limitations
- **Individual file checks**: `file.exists()` for each completed file would be O(n) filesystem calls
- **Performance bottleneck**: Checking thousands of files individually on large operations
- **Race conditions**: File might exist during check but be deleted before actual resume
- **No content verification**: File exists but might be corrupted/incomplete

## Proposed Solution: Ephemeral Indexing

### Core Concept
Use the existing `EphemeralIndex` infrastructure to perform a single, efficient scan of the destination directory and validate all completed files in bulk.

### Architecture Overview

```rust
// Enhanced resume validation flow
async fn validate_completed_files(&mut self, ctx: &JobContext<'_>) -> JobResult<ValidationResult> {
    if self.completed_indices.is_empty() {
        return Ok(ValidationResult::NoValidationNeeded);
    }

    // 1. Run ephemeral indexing on destination
    let destination_index = self.index_destination_ephemeral(ctx).await?;
    
    // 2. Validate completed files against actual state
    let validation_result = self.cross_reference_completed_files(&destination_index).await?;
    
    // 3. Update completed_indices based on findings
    self.apply_validation_result(validation_result, ctx).await?;
    
    Ok(validation_result)
}
```

## Implementation Details

### 1. Destination Indexing

```rust
async fn index_destination_ephemeral(&self, ctx: &JobContext<'_>) -> JobResult<Arc<RwLock<EphemeralIndex>>> {
    let destination_path = self.destination.as_local_path()
        .ok_or_else(|| JobError::execution("Destination must be local for validation"))?;

    ctx.log("Indexing destination for resume validation...");
    
    // Create ephemeral indexer job
    let mut indexer = IndexerJob::new(
        SdPath::new(self.destination.device_id, destination_path.to_path_buf()),
        IndexMode::Shallow,        // Just filesystem metadata - fastest
        IndexScope::Recursive,     // Full directory tree
        IndexPersistence::Ephemeral, // Don't persist to database
    );

    // Configure for validation use case
    indexer.config.skip_hidden = false;        // Include hidden files
    indexer.config.follow_symlinks = true;     // Follow symlinks
    indexer.config.max_depth = None;           // No depth limit
    
    // Run indexing
    let index_result = indexer.run(ctx.clone()).await?;
    
    index_result.ephemeral_results
        .ok_or_else(|| JobError::execution("Ephemeral indexing failed to produce results"))
}
```

### 2. Cross-Reference Validation

```rust
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub validated_files: Vec<usize>,      // Files confirmed to exist correctly
    pub missing_files: Vec<usize>,        // Files missing from destination
    pub corrupted_files: Vec<usize>,      // Files with incorrect size/metadata
    pub validation_duration: Duration,
}

async fn cross_reference_completed_files(
    &self, 
    destination_index: &Arc<RwLock<EphemeralIndex>>
) -> JobResult<ValidationResult> {
    let start_time = Instant::now();
    let index_guard = destination_index.read().await;
    
    let mut validated_files = Vec::new();
    let mut missing_files = Vec::new();
    let mut corrupted_files = Vec::new();
    
    for &completed_index in &self.completed_indices {
        if let Some(source) = self.sources.paths.get(completed_index) {
            let expected_dest_path = self.calculate_destination_path(source);
            
            match index_guard.entries.get(&expected_dest_path) {
                Some(dest_metadata) => {
                    // File exists - validate metadata
                    if let Ok(source_metadata) = std::fs::metadata(source.as_local_path().unwrap()) {
                        if self.validate_file_metadata(&source_metadata, dest_metadata) {
                            validated_files.push(completed_index);
                        } else {
                            corrupted_files.push(completed_index);
                        }
                    } else {
                        // Source file issues
                        missing_files.push(completed_index);
                    }
                }
                None => {
                    // File missing from destination
                    missing_files.push(completed_index);
                }
            }
        }
    }
    
    Ok(ValidationResult {
        validated_files,
        missing_files,
        corrupted_files,
        validation_duration: start_time.elapsed(),
    })
}
```

### 3. Metadata Validation

```rust
fn validate_file_metadata(
    &self,
    source_metadata: &std::fs::Metadata, 
    dest_metadata: &EntryMetadata
) -> bool {
    // Basic validation criteria
    let size_matches = source_metadata.len() == dest_metadata.size;
    
    // Optional: timestamp validation (if preserve_timestamps is enabled)
    let timestamp_valid = if self.options.preserve_timestamps {
        if let (Ok(source_modified), Some(dest_modified)) = (
            source_metadata.modified(),
            dest_metadata.date_modified
        ) {
            // Allow small timestamp differences (filesystem precision)
            let diff = source_modified.duration_since(dest_modified)
                .unwrap_or_else(|_| dest_modified.duration_since(source_modified).unwrap());
            diff < Duration::from_secs(2)
        } else {
            true // Skip timestamp validation if unavailable
        }
    } else {
        true
    };
    
    size_matches && timestamp_valid
}
```

### 4. Apply Validation Results

```rust
async fn apply_validation_result(
    &mut self, 
    result: ValidationResult, 
    ctx: &JobContext<'_>
) -> JobResult<()> {
    // Remove invalid files from completed_indices
    let files_to_remove: Vec<usize> = result.missing_files.iter()
        .chain(result.corrupted_files.iter())
        .copied()
        .collect();
    
    for &invalid_index in &files_to_remove {
        self.completed_indices.retain(|&x| x != invalid_index);
    }
    
    // Log validation results
    ctx.log(format!(
        "Resume validation completed in {:?}: {} validated, {} missing, {} corrupted",
        result.validation_duration,
        result.validated_files.len(),
        result.missing_files.len(),
        result.corrupted_files.len()
    ));
    
    if !result.missing_files.is_empty() {
        ctx.log(format!("Will re-copy {} missing files", result.missing_files.len()));
    }
    
    if !result.corrupted_files.is_empty() {
        ctx.log(format!("Will re-copy {} corrupted files", result.corrupted_files.len()));
    }
    
    Ok(())
}
```

## Performance Analysis

### Benefits

1. **Single directory traversal**: O(n) instead of O(n) individual file checks
2. **Bulk validation**: All files validated in memory after indexing
3. **Leverages optimized code**: Uses proven indexing infrastructure
4. **Efficient data structure**: HashMap lookups are O(1)
5. **Metadata caching**: File metadata already loaded during indexing

### Performance Comparison

| Approach | Time Complexity | Filesystem Calls | Memory Usage |
|----------|----------------|------------------|--------------|
| **Individual checks** | O(n) | n × stat() calls | O(1) |
| **Ephemeral indexing** | O(n) | 1 × directory traversal | O(n) |

For 10,000 files:
- **Individual checks**: 10,000 separate `stat()` syscalls
- **Ephemeral indexing**: 1 directory traversal + in-memory validation

### Expected Performance

```
Small operations (< 100 files):     Overhead ~50-100ms
Medium operations (1K files):       Overhead ~200-500ms  
Large operations (10K+ files):      Overhead ~1-3s (vs 10s+ for individual checks)
```

## Configuration Options

### Validation Modes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResumeValidationMode {
    /// No validation - trust completed_indices (fastest)
    None,
    /// Basic existence check using ephemeral indexing (recommended)
    Basic,
    /// Full metadata validation including size and timestamps
    Full,
    /// Checksum validation for critical operations (slowest)
    Checksum,
}

impl Default for ResumeValidationMode {
    fn default() -> Self {
        ResumeValidationMode::Basic
    }
}
```

### Integration with CopyOptions

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
    pub overwrite: bool,
    pub verify_checksum: bool,
    pub preserve_timestamps: bool,
    pub delete_after_copy: bool,
    pub move_mode: Option<MoveMode>,
    
    // New validation option
    pub resume_validation: ResumeValidationMode,
}
```

## Error Handling

### Validation Failures

```rust
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Destination directory doesn't exist
    DestinationMissing,
    /// Indexing failed due to permissions
    IndexingPermissionDenied,
    /// Indexing timed out
    IndexingTimeout,
    /// Destination contains unexpected files (conflict detection)
    UnexpectedFiles(Vec<PathBuf>),
}
```

### Graceful Degradation

```rust
async fn validate_with_fallback(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
    match self.validate_completed_files(ctx).await {
        Ok(_) => {
            ctx.log("Resume validation completed successfully");
            Ok(())
        }
        Err(e) => {
            ctx.add_warning(format!("Resume validation failed, clearing completed state: {}", e));
            // Fallback: clear completed_indices and restart
            self.completed_indices.clear();
            Ok(())
        }
    }
}
```

## Future Enhancements

### 1. Checksum-Based Validation

For critical operations, validate file integrity using checksums:

```rust
async fn validate_with_checksums(
    &self,
    completed_files: &[usize],
    ctx: &JobContext<'_>
) -> JobResult<Vec<usize>> {
    let mut valid_files = Vec::new();
    
    for &index in completed_files {
        if let Some(source) = self.sources.paths.get(index) {
            let dest_path = self.calculate_destination_path(source);
            
            // Calculate checksums
            let source_hash = calculate_file_checksum(source.as_local_path().unwrap()).await?;
            let dest_hash = calculate_file_checksum(&dest_path).await?;
            
            if source_hash == dest_hash {
                valid_files.push(index);
            } else {
                ctx.log(format!("Checksum mismatch for {}, will re-copy", source.display()));
            }
        }
    }
    
    Ok(valid_files)
}
```

### 2. Conflict Detection

Detect unexpected files in destination that might indicate conflicts:

```rust
fn detect_conflicts(&self, destination_index: &EphemeralIndex) -> Vec<PathBuf> {
    let mut unexpected_files = Vec::new();
    let expected_files: HashSet<PathBuf> = self.calculate_all_destination_paths();
    
    for (path, _metadata) in &destination_index.entries {
        if !expected_files.contains(path) {
            unexpected_files.push(path.clone());
        }
    }
    
    unexpected_files
}
```

### 3. Progress Reporting

```rust
async fn validate_completed_files(&mut self, ctx: &JobContext<'_>) -> JobResult<ValidationResult> {
    ctx.progress(Progress::indeterminate("Indexing destination for validation..."));
    
    let destination_index = self.index_destination_ephemeral(ctx).await?;
    
    ctx.progress(Progress::indeterminate("Validating completed files..."));
    
    let result = self.cross_reference_completed_files(&destination_index).await?;
    
    ctx.progress(Progress::indeterminate(format!(
        "Validation complete: {} files validated, {} will be re-copied",
        result.validated_files.len(),
        result.missing_files.len() + result.corrupted_files.len()
    )));
    
    Ok(result)
}
```

## Integration Points

### 1. Job Initialization

```rust
async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
    ctx.log(format!("Starting copy operation on {} files", self.sources.paths.len()));

    // Validate completed files if resuming
    if !self.completed_indices.is_empty() {
        self.validate_completed_files(&ctx).await?;
    }

    // ... rest of existing copy logic ...
}
```

### 2. CLI Integration

```bash
# Enable resume validation
sd copy --validate-resume=basic source dest

# Disable for performance-critical operations
sd copy --validate-resume=none source dest

# Full validation for critical data
sd copy --validate-resume=checksum source dest
```

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_validation_missing_files() {
    // Create completed_indices with files that don't exist
    // Run validation
    // Assert missing files are removed from completed_indices
}

#[tokio::test]
async fn test_validation_corrupted_files() {
    // Create files with wrong size/metadata
    // Run validation
    // Assert corrupted files are flagged for re-copying
}

#[tokio::test]
async fn test_validation_performance() {
    // Create large directory structure (10K files)
    // Measure validation time
    // Assert performance is reasonable
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_resume_workflow_with_validation() {
    // 1. Start copy operation
    // 2. Interrupt after partial completion
    // 3. Manually delete some destination files
    // 4. Resume operation
    // 5. Assert deleted files are re-copied
    // 6. Assert final state is correct
}
```

## Implementation Timeline

### Phase 1: Basic Implementation
- [ ] Core validation infrastructure
- [ ] Basic existence checking
- [ ] Integration with copy job
- [ ] Unit tests

### Phase 2: Enhanced Validation
- [ ] Metadata validation (size, timestamps)
- [ ] Configuration options
- [ ] Error handling and fallbacks
- [ ] Performance optimization

### Phase 3: Advanced Features
- [ ] Checksum-based validation
- [ ] Conflict detection
- [ ] Progress reporting
- [ ] CLI integration

## Conclusion

This design provides a robust, performant solution for resume validation that:

1. **Leverages existing infrastructure** - Uses proven ephemeral indexing
2. **Scales efficiently** - Single directory scan vs individual file checks
3. **Provides flexibility** - Multiple validation modes for different use cases
4. **Maintains reliability** - Comprehensive error handling and fallbacks
5. **Enables future enhancement** - Extensible architecture for advanced features

The approach transforms resume validation from a potential performance bottleneck into a reliable, efficient operation that scales to large directory structures while maintaining data integrity guarantees.