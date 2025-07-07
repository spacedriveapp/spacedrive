# Progressive Copy Design: Smart Preparation with Database Integration

## Overview

This document outlines a comprehensive design for implementing progressive copy operations that provide real-time preparation feedback, similar to macOS Finder, while leveraging Spacedrive's unique advantage of having pre-indexed directory data.

## Problem Statement

Current copy operations suffer from poor user experience due to:

1. **Black box behavior**: Progress jumps from 0% to 100% with no intermediate feedback
2. **Preparation delays**: Large directories take 20+ seconds to analyze with no visible progress
3. **Inefficient directory traversal**: Each copy operation performs expensive filesystem walks
4. **Missed optimization opportunities**: Existing indexed data is ignored, leading to redundant work

## Design Philosophy

### The Finder Advantage
macOS Finder provides excellent UX by showing:
- "Preparing to copy..." with item counting
- Real-time discovery of total files and bytes
- Smooth progress transitions from preparation to execution

### The Spacedrive Advantage
Spacedrive can go beyond Finder by:
- **Instant preparation** for indexed directories (database lookup vs filesystem scan)
- **Hybrid approach** combining cached data with ephemeral scanning
- **Staleness detection** to ensure data freshness
- **Progressive enhancement** from fast estimates to accurate totals

## Architecture Overview

```rust
┌─────────────────────────────────────────────────────────────────┐
│                     Progressive Copy Pipeline                    │
├─────────────────────────────────────────────────────────────────┤
│  Phase 1: Quick Assessment (< 100ms)                           │
│  ├─ Check database for cached directory info                   │
│  ├─ Provide instant estimates for indexed paths                │
│  └─ Show immediate "Preparing..." for unknown paths            │
├─────────────────────────────────────────────────────────────────┤
│  Phase 2: Staleness Validation (< 500ms)                       │
│  ├─ Check index timestamps vs filesystem modification          │
│  ├─ Detect if cached data is stale                             │
│  └─ Mark paths needing ephemeral scanning                      │
├─────────────────────────────────────────────────────────────────┤
│  Phase 3: Progressive Scanning (streaming)                     │
│  ├─ Run ephemeral indexing on stale/unknown paths              │
│  ├─ Stream results to update totals in real-time               │
│  └─ Combine with cached data for final preparation             │
├─────────────────────────────────────────────────────────────────┤
│  Phase 4: Validated Execution (with accurate progress)         │
│  ├─ Execute copy with precise progress tracking                 │
│  ├─ Update progress based on bytes copied vs total bytes       │
│  └─ Handle resume scenarios with validation                    │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Progressive Preparation State Machine

```rust
#[derive(Debug, Clone)]
pub enum PreparationPhase {
    /// Initial assessment using cached data
    QuickAssessment {
        estimated_files: u64,
        estimated_bytes: u64,
        cached_paths: Vec<PathBuf>,
        unknown_paths: Vec<PathBuf>,
    },
    /// Validating freshness of cached data
    StalenessValidation {
        validating_paths: Vec<PathBuf>,
        progress: f64, // 0.0 to 1.0
    },
    /// Scanning unknown/stale paths
    ProgressiveScanning {
        scanned_files: u64,
        scanned_bytes: u64,
        scanning_path: PathBuf,
        completion_ratio: f64,
    },
    /// Final preparation complete
    Ready {
        total_files: u64,
        total_bytes: u64,
        file_manifest: Vec<AnalyzedFile>,
    },
}

#[derive(Debug, Clone)]
pub struct PreparationState {
    pub phase: PreparationPhase,
    pub overall_progress: f64,
    pub current_operation: String,
    pub estimated_duration: Option<Duration>,
}
```

### 2. Smart Index Integration

```rust
/// Intelligent preparation engine that combines database and filesystem data
pub struct SmartPreparationEngine {
    database_cache: Arc<IndexCache>,
    ephemeral_scanner: Arc<EphemeralScanner>,
    staleness_detector: Arc<StalenessDetector>,
}

impl SmartPreparationEngine {
    /// Phase 1: Quick assessment using database cache
    pub async fn quick_assessment(
        &self,
        sources: &[SdPath],
        ctx: &JobContext<'_>,
    ) -> JobResult<PreparationPhase> {
        let mut estimated_files = 0u64;
        let mut estimated_bytes = 0u64;
        let mut cached_paths = Vec::new();
        let mut unknown_paths = Vec::new();

        for source in sources {
            match self.database_cache.get_cached_info(source).await? {
                Some(cached_info) => {
                    estimated_files += cached_info.file_count;
                    estimated_bytes += cached_info.total_size;
                    cached_paths.push(source.path.clone());
                    
                    ctx.progress(Progress::indeterminate(format!(
                        "Found cached data for {} ({} files, {})",
                        source.display(),
                        cached_info.file_count,
                        format_bytes(cached_info.total_size)
                    )));
                }
                None => {
                    unknown_paths.push(source.path.clone());
                    
                    ctx.progress(Progress::indeterminate(format!(
                        "Will scan: {}", source.display()
                    )));
                }
            }
        }

        Ok(PreparationPhase::QuickAssessment {
            estimated_files,
            estimated_bytes,
            cached_paths,
            unknown_paths,
        })
    }

    /// Phase 2: Validate staleness of cached data
    pub async fn validate_staleness(
        &self,
        cached_paths: &[PathBuf],
        ctx: &JobContext<'_>,
    ) -> JobResult<Vec<PathBuf>> {
        let mut stale_paths = Vec::new();
        
        for (i, path) in cached_paths.iter().enumerate() {
            ctx.progress(Progress::percentage(
                i as f64 / cached_paths.len() as f64,
                format!("Validating freshness: {}", path.display())
            ));

            if self.staleness_detector.is_stale(path).await? {
                stale_paths.push(path.clone());
                
                ctx.log(format!(
                    "Detected stale index for {}, will rescan", 
                    path.display()
                ));
            }
        }

        Ok(stale_paths)
    }

    /// Phase 3: Progressive scanning with real-time updates
    pub async fn progressive_scan(
        &self,
        paths_to_scan: &[PathBuf],
        ctx: &JobContext<'_>,
    ) -> JobResult<Vec<AnalyzedFile>> {
        let mut all_files = Vec::new();
        let mut scanned_files = 0u64;
        let mut scanned_bytes = 0u64;

        for (path_index, path) in paths_to_scan.iter().enumerate() {
            ctx.progress(Progress::structured(PreparationProgress {
                phase: "Progressive Scanning".to_string(),
                current_path: path.display().to_string(),
                scanned_files,
                scanned_bytes,
                completion_ratio: path_index as f64 / paths_to_scan.len() as f64,
            }));

            // Use streaming ephemeral indexer for real-time updates
            let path_files = self.scan_path_with_streaming_progress(path, ctx).await?;
            
            for file in &path_files {
                scanned_files += 1;
                scanned_bytes += file.size;
                
                // Update progress every 100 files for responsiveness
                if scanned_files % 100 == 0 {
                    ctx.progress(Progress::structured(PreparationProgress {
                        phase: "Progressive Scanning".to_string(),
                        current_path: path.display().to_string(),
                        scanned_files,
                        scanned_bytes,
                        completion_ratio: path_index as f64 / paths_to_scan.len() as f64,
                    }));
                }
            }
            
            all_files.extend(path_files);
        }

        Ok(all_files)
    }
}
```

### 3. Staleness Detection System

```rust
/// Detects when cached index data is outdated
pub struct StalenessDetector {
    database: Arc<Database>,
    filesystem_cache: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
}

impl StalenessDetector {
    /// Check if a path's index data is stale
    pub async fn is_stale(&self, path: &Path) -> JobResult<bool> {
        // Get index timestamp from database
        let index_timestamp = self.database
            .get_location_last_indexed(path)
            .await?
            .ok_or_else(|| JobError::execution("Path not found in index"))?;

        // Get filesystem modification time (with caching)
        let fs_mtime = self.get_cached_mtime(path).await?;

        // Consider stale if filesystem is newer than index
        // Add small buffer for filesystem timestamp precision
        let staleness_threshold = Duration::from_secs(1);
        
        match fs_mtime.duration_since(index_timestamp) {
            Ok(diff) => Ok(diff > staleness_threshold),
            Err(_) => {
                // Filesystem is older than index - definitely not stale
                Ok(false)
            }
        }
    }

    /// Check directory tree for any stale subdirectories
    pub async fn check_tree_staleness(&self, root_path: &Path) -> JobResult<Vec<PathBuf>> {
        let mut stale_paths = Vec::new();

        // Get all indexed subdirectories under this path
        let indexed_subdirs = self.database
            .get_indexed_subdirectories(root_path)
            .await?;

        for subdir in indexed_subdirs {
            if self.is_stale(&subdir).await? {
                stale_paths.push(subdir);
            }
        }

        Ok(stale_paths)
    }

    /// Advanced staleness detection using directory modification times
    pub async fn detect_stale_subtrees(&self, path: &Path) -> JobResult<StalenessReport> {
        // Check if the directory itself is stale
        let root_stale = self.is_stale(path).await?;

        if root_stale {
            // If root is stale, everything under it needs rescanning
            return Ok(StalenessReport {
                is_fully_stale: true,
                stale_subtrees: vec![path.to_path_buf()],
                fresh_subtrees: vec![],
                confidence: StalenessConfidence::High,
            });
        }

        // Root is fresh, check for stale subtrees
        let stale_subtrees = self.check_tree_staleness(path).await?;
        
        if stale_subtrees.is_empty() {
            Ok(StalenessReport {
                is_fully_stale: false,
                stale_subtrees: vec![],
                fresh_subtrees: vec![path.to_path_buf()],
                confidence: StalenessConfidence::High,
            })
        } else {
            Ok(StalenessReport {
                is_fully_stale: false,
                stale_subtrees,
                fresh_subtrees: vec![], // TODO: Calculate fresh subtrees
                confidence: StalenessConfidence::Medium,
            })
        }
    }
}

#[derive(Debug)]
pub struct StalenessReport {
    pub is_fully_stale: bool,
    pub stale_subtrees: Vec<PathBuf>,
    pub fresh_subtrees: Vec<PathBuf>,
    pub confidence: StalenessConfidence,
}

#[derive(Debug)]
pub enum StalenessConfidence {
    High,   // Directory mtimes are reliable indicators
    Medium, // Some uncertainty due to filesystem behavior
    Low,    // Fallback to conservative rescanning
}
```

### 4. Hybrid Data Combining

```rust
/// Combines cached database data with fresh ephemeral scan results
pub struct HybridDataCombiner;

impl HybridDataCombiner {
    /// Merge cached and scanned data into unified file manifest
    pub async fn combine_sources(
        &self,
        cached_data: HashMap<PathBuf, CachedDirectoryInfo>,
        scanned_data: HashMap<PathBuf, Vec<AnalyzedFile>>,
        stale_paths: &[PathBuf],
    ) -> JobResult<Vec<AnalyzedFile>> {
        let mut combined_files = Vec::new();

        // Process cached data (excluding stale paths)
        for (path, cached_info) in cached_data {
            if !stale_paths.contains(&path) {
                // Convert cached database entries to AnalyzedFile format
                let files = self.convert_cached_to_analyzed(cached_info).await?;
                combined_files.extend(files);
            }
        }

        // Add fresh scan results
        for files in scanned_data.values() {
            combined_files.extend(files.clone());
        }

        // Sort by source path for consistent processing order
        combined_files.sort_by(|a, b| a.source_path.cmp(&b.source_path));

        Ok(combined_files)
    }

    /// Convert database cached info to AnalyzedFile format
    async fn convert_cached_to_analyzed(
        &self,
        cached_info: CachedDirectoryInfo,
    ) -> JobResult<Vec<AnalyzedFile>> {
        let mut files = Vec::new();

        for entry in cached_info.entries {
            if entry.is_file {
                files.push(AnalyzedFile {
                    source_path: entry.path,
                    destination_path: PathBuf::new(), // Will be calculated later
                    size: entry.size.unwrap_or(0) as u64,
                    is_completed: false,
                    cached_metadata: Some(entry.metadata),
                });
            }
        }

        Ok(files)
    }
}
```

### 5. Streaming Progress Updates

```rust
/// Progress reporting for the preparation phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationProgress {
    pub phase: String,
    pub current_path: String,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub completion_ratio: f64,
}

impl JobProgress for PreparationProgress {}

/// Enhanced copy job with progressive preparation
impl JobHandler for FileCopyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log("Starting progressive copy preparation...");

        // Progressive preparation with real-time updates
        let preparation_result = self.progressive_preparation(&ctx).await?;

        ctx.log(format!(
            "Preparation complete: {} files ({}) ready for copy",
            preparation_result.total_files,
            format_bytes(preparation_result.total_bytes)
        ));

        // Execute copy with accurate progress
        self.execute_with_manifest(&ctx, preparation_result).await
    }

    async fn progressive_preparation(
        &mut self,
        ctx: &JobContext<'_>,
    ) -> JobResult<PreparationResult> {
        let engine = SmartPreparationEngine::new(ctx).await?;

        // Phase 1: Quick assessment
        ctx.progress(Progress::indeterminate("Checking cached directory data..."));
        let quick_assessment = engine.quick_assessment(&self.sources.paths, ctx).await?;

        // Show immediate estimates if we have cached data
        if let PreparationPhase::QuickAssessment { estimated_files, estimated_bytes, .. } = &quick_assessment {
            if *estimated_files > 0 {
                ctx.progress(Progress::percentage(
                    0.25, // 25% complete after quick assessment
                    format!(
                        "Found cached data: ~{} files (~{})",
                        estimated_files,
                        format_bytes(*estimated_bytes)
                    )
                ));
            }
        }

        // Phase 2: Staleness validation
        ctx.progress(Progress::percentage(0.25, "Validating data freshness..."));
        let stale_paths = if let PreparationPhase::QuickAssessment { cached_paths, .. } = &quick_assessment {
            engine.validate_staleness(cached_paths, ctx).await?
        } else {
            vec![]
        };

        // Phase 3: Progressive scanning
        let paths_to_scan = self.get_paths_needing_scan(&quick_assessment, &stale_paths);
        
        if !paths_to_scan.is_empty() {
            ctx.progress(Progress::percentage(
                0.5,
                format!("Scanning {} paths for complete analysis...", paths_to_scan.len())
            ));
            
            let scanned_files = engine.progressive_scan(&paths_to_scan, ctx).await?;
            
            // Combine with cached data
            let combiner = HybridDataCombiner;
            let final_manifest = combiner.combine_all_sources(
                &quick_assessment,
                &scanned_files,
                &stale_paths,
            ).await?;

            return Ok(PreparationResult {
                total_files: final_manifest.len(),
                total_bytes: final_manifest.iter().map(|f| f.size).sum(),
                file_manifest: final_manifest,
            });
        }

        // All data was cached and fresh
        ctx.progress(Progress::percentage(1.0, "Preparation complete using cached data"));
        
        Ok(self.create_result_from_cache(&quick_assessment).await?)
    }
}
```

## Database Schema Enhancements

### Enhanced Location Tracking

```sql
-- Add staleness tracking to locations table
ALTER TABLE locations ADD COLUMN last_full_scan TIMESTAMP;
ALTER TABLE locations ADD COLUMN last_modification_check TIMESTAMP;
ALTER TABLE locations ADD COLUMN scan_status ENUM('fresh', 'partial', 'stale', 'unknown');

-- Directory-level indexing metadata
CREATE TABLE directory_scan_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    location_id INTEGER REFERENCES locations(id),
    directory_path TEXT NOT NULL,
    last_scanned TIMESTAMP NOT NULL,
    file_count INTEGER NOT NULL,
    total_size BIGINT NOT NULL,
    last_modified TIMESTAMP, -- Directory's actual mtime
    scan_depth INTEGER DEFAULT 0,
    is_complete BOOLEAN DEFAULT FALSE,
    UNIQUE(location_id, directory_path)
);

-- Quick lookup index for staleness checks
CREATE INDEX idx_directory_scan_path ON directory_scan_metadata(directory_path);
CREATE INDEX idx_directory_scan_modified ON directory_scan_metadata(last_modified);
```

### Cached Summary Queries

```rust
/// Database queries optimized for copy preparation
impl Database {
    /// Get quick directory summary for instant estimates
    pub async fn get_directory_summary(
        &self,
        path: &Path,
    ) -> Result<Option<DirectorySummary>, DatabaseError> {
        let result = sqlx::query_as!(
            DirectorySummary,
            r#"
            SELECT 
                COUNT(CASE WHEN kind = 'file' THEN 1 END) as file_count,
                COALESCE(SUM(CASE WHEN kind = 'file' THEN size END), 0) as total_size,
                MAX(date_modified) as last_modified,
                MIN(date_indexed) as oldest_index,
                MAX(date_indexed) as newest_index
            FROM file_entries 
            WHERE path LIKE ?1 || '%'
            "#,
            path.to_string_lossy()
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get directory scan metadata for staleness checking
    pub async fn get_scan_metadata(
        &self,
        directory_path: &Path,
    ) -> Result<Option<DirectoryScanMetadata>, DatabaseError> {
        let result = sqlx::query_as!(
            DirectoryScanMetadata,
            r#"
            SELECT last_scanned, file_count, total_size, last_modified, is_complete
            FROM directory_scan_metadata
            WHERE directory_path = ?1
            "#,
            directory_path.to_string_lossy()
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Bulk fetch entries for multiple directories (optimized)
    pub async fn bulk_fetch_directory_entries(
        &self,
        directory_paths: &[PathBuf],
    ) -> Result<HashMap<PathBuf, Vec<FileEntry>>, DatabaseError> {
        let path_list = directory_paths
            .iter()
            .map(|p| format!("'{}'", p.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(",");

        let query = format!(
            r#"
            SELECT path, name, size, kind, date_modified, date_indexed
            FROM file_entries 
            WHERE parent_path IN ({})
            ORDER BY parent_path, name
            "#,
            path_list
        );

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;
        
        // Group results by directory
        let mut result = HashMap::new();
        for row in rows {
            let parent_path = PathBuf::from(row.get::<String, _>("parent_path"));
            let entry = FileEntry::from_row(&row)?;
            
            result.entry(parent_path)
                .or_insert_with(Vec::new)
                .push(entry);
        }

        Ok(result)
    }
}
```

## Performance Characteristics

### Expected Performance Improvements

| Scenario | Current Behavior | With Progressive Design |
|----------|------------------|------------------------|
| **Indexed directory (fresh)** | 20s filesystem scan | ~100ms database lookup |
| **Indexed directory (stale)** | 20s filesystem scan | ~2s staleness check + partial scan |
| **Mixed indexed/new** | 20s full scan | ~500ms cached + streaming for new |
| **Large directory (10K files)** | 0% → 100% jump | Smooth 0% → 100% with streaming |
| **Resume validation** | Individual file checks | Bulk ephemeral validation |

### Memory Usage

```rust
// Memory-efficient streaming approach
pub struct StreamingPreparation {
    // Small working set - process files as they're discovered
    current_batch: Vec<AnalyzedFile>, // Max 1000 files
    total_discovered: u64,
    bytes_discovered: u64,
    
    // Stream results to job progress without storing everything
    progress_callback: Box<dyn Fn(PreparationProgress) + Send + Sync>,
}
```

## Integration with Existing Systems

### Connection to IDEA_FOR_BETTER_INDEXING.md

```rust
// TODO: Integration point with broader indexing improvements
// Reference: docs/design/IDEA_FOR_BETTER_INDEXING.md
//
// Future enhancements should consider:
// 1. Offline period detection → automatic staleness marking
// 2. Background freshness checking → proactive cache warming  
// 3. Location-level staleness → intelligent re-indexing triggers
// 4. Performance-aware staleness → balance accuracy vs speed

pub struct IndexIntegrationTodos;

impl IndexIntegrationTodos {
    /// TODO: Implement location staleness tracking from IDEA_FOR_BETTER_INDEXING.md
    /// When locations are marked stale due to offline periods, copy preparation
    /// should automatically trigger selective re-indexing of affected paths.
    pub fn handle_offline_staleness() {
        todo!("Integrate with location staleness detection system")
    }

    /// TODO: Background staleness detection
    /// Proactively check directory modification times in background to warm
    /// staleness cache, reducing preparation latency.
    pub fn background_staleness_check() {
        todo!("Implement background staleness validation")
    }

    /// TODO: Smart re-indexing triggers  
    /// When copy preparation detects stale data, coordinate with indexing
    /// system to update persistent index, not just ephemeral data.
    pub fn coordinate_persistent_indexing() {
        todo!("Bridge ephemeral scanning with persistent index updates")
    }
}
```

### File Watcher Integration

```rust
/// Integration with file watcher for real-time staleness updates
pub struct FilewatcherStalenessSync {
    staleness_cache: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
    database: Arc<Database>,
}

impl FilewatcherStalenessSync {
    /// Update staleness cache when file watcher detects changes
    pub async fn handle_filesystem_event(&self, event: FilesystemEvent) {
        match event {
            FilesystemEvent::Modified { path, timestamp } => {
                // Mark directory as potentially stale
                if let Some(parent) = path.parent() {
                    let mut cache = self.staleness_cache.write().await;
                    cache.insert(parent.to_path_buf(), timestamp);
                }
                
                // Update database scan metadata
                self.database.mark_directory_stale(&path).await.ok();
            }
            FilesystemEvent::Deleted { path, .. } => {
                // Definitely stale - needs rescan
                self.database.mark_directory_stale(&path).await.ok();
            }
            _ => {}
        }
    }
}
```

## Error Handling & Fallbacks

### Graceful Degradation Strategy

```rust
/// Robust error handling with fallback strategies
impl SmartPreparationEngine {
    async fn prepare_with_fallbacks(
        &self,
        sources: &[SdPath],
        ctx: &JobContext<'_>,
    ) -> JobResult<PreparationResult> {
        // Strategy 1: Try smart hybrid approach
        match self.progressive_preparation(sources, ctx).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                ctx.add_warning(format!(
                    "Smart preparation failed ({}), falling back to standard scan", 
                    e
                ));
            }
        }

        // Strategy 2: Fall back to pure ephemeral scanning
        match self.ephemeral_scan_fallback(sources, ctx).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                ctx.add_warning(format!(
                    "Ephemeral scan failed ({}), using basic preparation", 
                    e
                ));
            }
        }

        // Strategy 3: Basic preparation (current approach)
        self.basic_preparation_fallback(sources, ctx).await
    }

    async fn handle_database_unavailable(&self, ctx: &JobContext<'_>) -> JobResult<()> {
        ctx.log("Database unavailable for copy preparation, using filesystem-only approach");
        
        // Disable all database-dependent optimizations
        self.config.use_database_cache = false;
        self.config.staleness_checking = false;
        
        Ok(())
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cached_directory_instant_preparation() {
        // Setup: Create indexed directory in test database
        let test_db = create_test_database().await;
        populate_test_directory_index(&test_db, "/test/cached", 1000, 50_000_000).await;

        // Test: Preparation should be instant for cached directory
        let engine = SmartPreparationEngine::new_with_db(test_db);
        let start = Instant::now();
        
        let result = engine.quick_assessment(&[SdPath::local("/test/cached")]).await.unwrap();
        
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(result.estimated_files, 1000);
        assert_eq!(result.estimated_bytes, 50_000_000);
    }

    #[tokio::test]
    async fn test_staleness_detection() {
        let detector = StalenessDetector::new_test();
        
        // Setup: Directory indexed 1 hour ago, modified 30 minutes ago
        let path = PathBuf::from("/test/stale");
        detector.set_index_time(&path, SystemTime::now() - Duration::from_secs(3600)).await;
        detector.set_fs_mtime(&path, SystemTime::now() - Duration::from_secs(1800)).await;
        
        // Test: Should detect as stale
        assert!(detector.is_stale(&path).await.unwrap());
    }

    #[tokio::test]
    async fn test_progressive_scanning_updates() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let progress_callback = move |progress: PreparationProgress| {
            tx.try_send(progress).ok();
        };

        let engine = SmartPreparationEngine::new_with_callback(progress_callback);
        
        // Start scanning large directory
        let scan_task = tokio::spawn(async move {
            engine.progressive_scan(&[PathBuf::from("/large/directory")]).await
        });

        // Verify we receive streaming progress updates
        let mut update_count = 0;
        let mut last_file_count = 0;
        
        while let Some(progress) = rx.recv().await {
            assert!(progress.scanned_files >= last_file_count);
            last_file_count = progress.scanned_files;
            update_count += 1;
            
            if update_count >= 10 {
                break; // Received enough progress updates
            }
        }

        assert!(update_count >= 10, "Should receive frequent progress updates");
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_progressive_copy() {
    // Setup: Mixed scenario with cached and uncached directories
    let test_env = TestEnvironment::new().await;
    
    // Create source with mixed cached/uncached content
    test_env.create_indexed_directory("/source/cached", 500).await;
    test_env.create_unindexed_directory("/source/new", 300).await;
    
    // Start copy operation
    let copy_job = FileCopyJob::new(
        SdPathBatch::new(vec![SdPath::local("/source")]),
        SdPath::local("/dest"),
    );

    // Track progress updates
    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = progress_updates.clone();
    
    let progress_handler = move |progress: Progress| {
        updates_clone.lock().unwrap().push(progress);
    };

    // Execute copy with progress tracking
    let result = copy_job.run_with_progress_handler(progress_handler).await.unwrap();

    // Verify results
    assert_eq!(result.copied_count, 800);
    
    let updates = progress_updates.lock().unwrap();
    
    // Should have preparation phase updates
    assert!(updates.iter().any(|p| p.message.contains("Checking cached")));
    assert!(updates.iter().any(|p| p.message.contains("Progressive Scanning")));
    
    // Should have smooth progression (no 0% → 100% jumps)
    let percentages: Vec<f64> = updates.iter()
        .filter_map(|p| match p {
            Progress::Percentage { percentage, .. } => Some(*percentage),
            _ => None,
        })
        .collect();
        
    // Verify smooth progression
    for window in percentages.windows(2) {
        let jump = window[1] - window[0];
        assert!(jump < 0.5, "Progress jump too large: {}", jump);
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Implement basic `SmartPreparationEngine`
- [ ] Add database summary queries
- [ ] Create `StalenessDetector` with simple mtime checking
- [ ] Update `FileCopyJob` to use progressive preparation

### Phase 2: Advanced Features (Week 3-4)  
- [ ] Implement streaming ephemeral indexing
- [ ] Add `HybridDataCombiner` for merging cached/scanned data
- [ ] Enhance progress reporting with structured updates
- [ ] Add comprehensive error handling and fallbacks

### Phase 3: Optimization (Week 5-6)
- [ ] Add background staleness checking
- [ ] Implement memory-efficient streaming
- [ ] Add performance monitoring and metrics
- [ ] Optimize database queries for bulk operations

### Phase 4: Integration (Week 7-8)
- [ ] Integrate with file watcher for real-time staleness
- [ ] Connect with location staleness system (IDEA_FOR_BETTER_INDEXING.md)
- [ ] Add CLI options for controlling preparation behavior
- [ ] Comprehensive testing and documentation

## Conclusion

This progressive copy design transforms Spacedrive's copy operations from a black-box process into a transparent, efficient system that provides excellent user experience while leveraging Spacedrive's unique indexed data advantage.

### Key Benefits

1. **Instant Feedback**: Users see immediate progress for cached directories
2. **Smart Optimization**: Leverages existing indexed data to avoid redundant work  
3. **Graceful Degradation**: Falls back gracefully when optimizations aren't available
4. **Future-Proof**: Integrates with planned indexing improvements
5. **Finder-Class UX**: Matches or exceeds macOS Finder's preparation experience

### Spacedrive's Competitive Advantage

Unlike traditional file managers that must scan filesystems for every operation, Spacedrive can provide:
- **Sub-second preparation** for previously indexed directories
- **Hybrid intelligence** combining cached and real-time data
- **Predictive staleness detection** using file watcher integration
- **Background optimization** for future operations

This design positions Spacedrive as not just a file manager, but an intelligent file operations system that gets faster and smarter over time.