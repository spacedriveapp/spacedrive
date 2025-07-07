# Progressive Copy Design: Hybrid Database + Real-time Discovery

## Overview

This document outlines a design for implementing progressive copy operations that provide real-time preparation feedback, similar to macOS Finder, while leveraging Spacedrive's indexed data for instant initial preparation and real-time discovery for complete file coverage.

## Problem Statement

Current copy operations suffer from poor user experience due to:

1. **Black box behavior**: Progress jumps from 0% to 100% with no intermediate feedback
2. **Preparation delays**: Large directories take 20+ seconds to analyze with no visible progress
3. **Inefficient directory traversal**: Each copy operation performs expensive filesystem walks

## Design Philosophy

### The Finder Advantage

macOS Finder provides excellent UX by showing:

- "Preparing to copy..." with item counting
- Real-time discovery of total files and bytes
- Smooth progress transitions from preparation to execution

### The Spacedrive Advantage

Spacedrive can go beyond Finder by:

- **Instant initial estimates** using existing indexed data
- **Real-time discovery** for files not in database (filtered or new)
- **Concurrent processing** - copying known files while discovering new ones
- **Complete coverage** ensuring all user-selected files are copied

### Key Insight: Hybrid Approach

Since global filters mean database never contains complete file sets, we use:

- **Database for instant start**: Copy known files immediately
- **Real-time indexer for gaps**: Discover and add unindexed files concurrently
- **Delta-only communication**: Child indexer only reports files NOT in database

## Architecture

```rust
┌─────────────────────────────────────────────────────────────────┐
│           Progressive Copy with Destination Pre-indexing       │
├─────────────────────────────────────────────────────────────────┤
│  Phase 0: Destination Pre-indexing (highest priority)          │
│  ├─ Spawn destination indexer FIRST with filter bypass         │
│  ├─ Build complete map of existing files at destination        │
│  ├─ Non-blocking: continue while indexer runs in background    │
│  └─ Provides instant "skip" decisions when indexer completes   │
├─────────────────────────────────────────────────────────────────┤
│  Phase 1: Instant Database Start (< 100ms)                     │
│  ├─ Query existing indexed files for all sources               │
│  ├─ Get aggregate estimates (size, count) from database        │
│  ├─ Filter against destination index (if ready) or filesystem  │
│  ├─ Start copying known files immediately                      │
│  └─ Show initial progress: "Copying 1,234 files (2.1 GB)..."   │
├─────────────────────────────────────────────────────────────────┤
│  Phase 2: Concurrent Real-time Discovery                       │
│  ├─ Spawn source indexer with filter bypass                    │
│  ├─ Child scans filesystem and finds NEW files (not in DB)     │
│  ├─ Child streams discoveries back to parent as it finds them  │
│  ├─ Parent filters against destination index before adding     │
│  └─ Progress updates: "Found 56 more files, copying..."        │
├─────────────────────────────────────────────────────────────────┤
│  Phase 3: Complete When All Done                               │
│  ├─ Copy job continues until all files (known + discovered)    │
│  ├─ Both indexers report completion                            │
│  ├─ Parent ensures all discovered files are copied             │
│  └─ Final completion when copying and both indexers done       │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Destination Pre-indexing Strategy

The most critical optimization: index the destination FIRST to build a complete "what already exists" map.

```rust
/// Destination indexer that builds complete existence map
pub struct DestinationIndexer {
    destination_root: PathBuf,
    existing_files: Arc<RwLock<HashSet<PathBuf>>>,
    indexing_complete: Arc<AtomicBool>,
}

impl DestinationIndexer {
    /// Start destination indexing immediately (highest priority)
    pub async fn start_indexing(&self, ctx: &JobContext<'_>) -> JobResult<JobHandle<IndexerJobOutput>> {
        ctx.log("🎯 Starting destination pre-indexing for skip optimization");

        // Create ephemeral indexer config for destination with filter bypass
        let dest_batch = SdPathBatch::new(vec![SdPath::from_path(&self.destination_root)]);
        let config = IndexerJobConfig::destination_analysis(dest_batch);

        let mut dest_indexer = IndexerJob::from_config(config);
        dest_indexer.set_parent_job_id(ctx.job_id());

        // Spawn with HIGHEST priority - this blocks nothing but gets CPU priority
        let handle = ctx.spawn_child_with_priority(dest_indexer, JobPriority::CRITICAL).await?;

        // Monitor progress and populate our existence map
        self.monitor_indexer_progress(handle.clone()).await;

        Ok(handle)
    }

    /// Monitor indexer progress and populate existence map
    async fn monitor_indexer_progress(&self, handle: JobHandle<IndexerJobOutput>) {
        let progress_stream = handle.subscribe_progress();
        let existing_files = self.existing_files.clone();
        let indexing_complete = self.indexing_complete.clone();

        tokio::spawn(async move {
            while let Ok(progress) = progress_stream.recv().await {
                if let Progress::Structured(data) = progress {
                    // Extract file paths from indexer progress
                    if let Ok(file_discovered) = serde_json::from_value::<FileDiscoveredProgress>(data) {
                        existing_files.write().await.insert(file_discovered.path);
                    }
                }
            }

            // Mark complete when indexer finishes
            indexing_complete.store(true, Ordering::Release);
        });
    }
}

    /// Check if file exists at destination (non-blocking)
    pub async fn file_exists(&self, relative_path: &Path) -> ExistenceCheck {
        let full_destination = self.destination_root.join(relative_path);

        if self.indexing_complete.load(Ordering::Acquire) {
            // Indexing complete - use pre-computed results (instant)
            let exists = self.existing_files.read().await.contains(&full_destination);
            ExistenceCheck::Known(exists)
        } else {
            // Indexing still running - fall back to filesystem check (non-blocking)
            let exists = tokio::fs::metadata(&full_destination).await.is_ok();
            ExistenceCheck::FilesystemFallback(exists)
        }
    }
}

#[derive(Debug)]
pub enum ExistenceCheck {
    /// Result from completed destination index (instant, accurate)
    Known(bool),
    /// Fallback filesystem check while indexing runs (slower, but non-blocking)
    FilesystemFallback(bool),
}

/// Progress type for file discovery from indexer
#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiscoveredProgress {
    pub path: PathBuf,
    pub size: u64,
    pub kind: String, // "file" or "directory"
}

/// IndexerJobConfig extensions for destination analysis
impl IndexerJobConfig {
    /// Create configuration for destination analysis (existence mapping)
    pub fn destination_analysis(destination: SdPathBatch) -> Self {
        Self {
            location_id: None,
            sources: destination,
            mode: IndexMode::CopyPreparation, // Use same mode to bypass filters
            scope: IndexScope::Recursive,
            persistence: IndexPersistence::Ephemeral, // Don't save to database
            max_depth: None,
            bypass_filters: true, // Index ALL files at destination for complete existence map
        }
    }
}
```

### 2. Hybrid Preparation Engine

```rust
/// Preparation engine that combines database queries with real-time discovery
pub struct HybridPreparationEngine {
    database: Arc<Database>,
}

impl HybridPreparationEngine {
    /// Get existing files from database for instant copying start
    pub async fn get_database_files(&self, sources: &[SdPath]) -> JobResult<Vec<AnalyzedFile>> {
        let mut all_files = Vec::new();

        for source in sources {
            if source.is_file() {
                // Check if single file is in database
                if let Some(file_data) = self.query_single_file(source).await? {
                    all_files.push(file_data);
                }
                // If not in DB, child indexer will find it
            } else {
                // Query all indexed files under this directory
                let indexed_files = self.query_directory_files(source).await?;
                all_files.extend(indexed_files);
            }
        }

        Ok(all_files)
    }

    /// Query database for files under a directory (leverages existing indexed data)
    async fn query_directory_files(&self, source: &SdPath) -> JobResult<Vec<AnalyzedFile>> {
        let source_path = source.as_local_path()
            .ok_or_else(|| JobError::execution("Source must be local"))?;

        // Find location containing this source
        let location = self.find_location_for_path(source_path).await?;
        let location_root = Path::new(&location.path);

        // Calculate relative path for database query
        let relative_path = if let Ok(rel_path) = source_path.strip_prefix(location_root) {
            rel_path.to_string_lossy().to_string()
        } else {
            return Ok(Vec::new()); // Path not in any indexed location
        };

        // Query all files under this path (what's already indexed)
        let files = entries::Entity::find()
            .filter(entries::Column::LocationId.eq(location.id))
            .filter(
                entries::Column::RelativePath.eq(&relative_path)
                    .or(entries::Column::RelativePath.like(format!("{}/%", relative_path)))
                    .or(entries::Column::RelativePath.like(format!("{}\\%", relative_path))) // Windows
            )
            .filter(entries::Column::Kind.eq(0)) // Files only
            .all(&self.database.connection)
            .await?;

        // Convert to AnalyzedFile format
        let analyzed_files = files.into_iter().map(|entry| {
            let full_path = if entry.relative_path.is_empty() {
                location_root.join(&entry.name)
            } else {
                location_root.join(&entry.relative_path).join(&entry.name)
            };

            AnalyzedFile {
                source_path: full_path,
                destination_path: PathBuf::new(), // Will be calculated later
                size: entry.size as u64,
                is_completed: false,
            }
        }).collect();

        Ok(analyzed_files)
    }

    /// Get quick size estimates from database aggregates
    pub async fn get_database_estimates(&self, sources: &[SdPath]) -> JobResult<EstimateData> {
        let mut total_size = 0;
        let mut total_files = 0;

        for source in sources {
            if source.is_file() {
                if let Some(file_entry) = self.query_single_file_entry(source).await? {
                    total_size += file_entry.size as u64;
                    total_files += 1;
                }
            } else {
                // Use aggregate_size and file_count from directory entries
                if let Some(dir_entry) = self.query_directory_entry(source).await? {
                    total_size += dir_entry.aggregate_size.unwrap_or(0) as u64;
                    total_files += dir_entry.file_count.unwrap_or(0) as u64;
                }
            }
        }

        Ok(EstimateData {
            estimated_size: total_size,
            estimated_files: total_files,
            is_complete: false, // These are estimates - discovery may find more
        })
    }
}

#[derive(Debug)]
pub struct EstimateData {
    pub estimated_size: u64,
    pub estimated_files: u64,
    pub is_complete: bool,
}
```

### 2. Enhanced Progress System for Real-time Communication

```rust
/// Enhanced progress types for parent-child job communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CopyProgress {
    /// Initial database estimates
    InitialEstimate {
        known_files: u64,
        known_size: u64,
        discovery_starting: bool,
    },

    /// Child indexer discovered new file not in database
    FileDiscovered {
        path: PathBuf,
        size: u64,
        discovery_batch: u32,
    },

    /// Child indexer completed discovery phase
    DiscoveryComplete {
        total_discovered: u64,
        discovery_size: u64,
    },

    /// File copy progress
    FileCopied {
        path: PathBuf,
        size: u64,
        was_discovered: bool, // true if from indexer, false if from database
    },

    /// Overall copy progress
    OverallProgress {
        copied_files: u64,
        total_files: u64,
        copied_bytes: u64,
        total_bytes: u64,
        skipped_files: u64,
        discovery_active: bool,
    },

    /// Destination analysis progress
    DestinationAnalysis {
        files_found: u64,
        total_size: u64,
        path: PathBuf,
    },
}
```

### 3. Child Indexer Job with Delta Discovery

```rust
/// Indexer job that only reports files NOT in database
pub struct DeltaIndexerJob {
    sources: SdPathBatch,
    database: Arc<Database>,
    parent_job_id: JobId,
}

impl DeltaIndexerJob {
    /// Process discovered file - only report if not in database
    async fn process_discovered_file(
        &self,
        ctx: &JobContext<'_>,
        path: &Path,
        metadata: &std::fs::Metadata
    ) -> JobResult<()> {
        // Check if this file exists in database
        let exists_in_db = self.check_file_in_database(path).await?;

        if !exists_in_db {
            // This is a NEW discovery - report to parent
            ctx.progress(CopyProgress::FileDiscovered {
                path: path.to_path_buf(),
                size: metadata.len(),
                discovery_batch: self.current_batch,
            });

            // Also add to ephemeral index for complete record
            self.add_to_ephemeral_index(path, metadata).await?;
        }

        // If exists in DB, parent already has it - don't report
        Ok(())
    }

    /// Check if file exists in database (any indexed location)
    async fn check_file_in_database(&self, path: &Path) -> JobResult<bool> {
        // Find which location this path belongs to
        let location = self.find_location_for_path(path).await?;
        let Some(loc) = location else {
            return Ok(false); // Not in any indexed location
        };

        // Calculate relative path
        let location_root = Path::new(&loc.path);
        let relative_path = path.strip_prefix(location_root)
            .map_err(|_| JobError::execution("Path not in location"))?;

        let (rel_dir, filename) = if let Some(parent) = relative_path.parent() {
            (parent.to_string_lossy().to_string(), relative_path.file_name().unwrap().to_string_lossy().to_string())
        } else {
            (String::new(), relative_path.to_string_lossy().to_string())
        };

        // Query database
        let exists = entries::Entity::find()
            .filter(entries::Column::LocationId.eq(loc.id))
            .filter(entries::Column::RelativePath.eq(rel_dir))
            .filter(entries::Column::Name.eq(filename))
            .one(&self.database.connection)
            .await?
            .is_some();

        Ok(exists)
    }
}
```

### 4. Enhanced Copy Job with Destination Pre-indexing

```rust
/// Copy job with destination pre-indexing and hybrid execution
impl FileCopyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log("🚀 Starting progressive copy with destination pre-indexing");

        // Phase 0: Start destination indexing FIRST (critical priority)
        let destination_indexer = DestinationIndexer::new(self.destination_root.clone());
        let dest_handle = destination_indexer.start_indexing(&ctx).await?;

        // Phase 1: Instant database start (while destination indexes)
        let prep_engine = HybridPreparationEngine::new(ctx.library_db());

        // Get quick estimates for immediate user feedback
        let estimates = prep_engine.get_database_estimates(&self.sources.paths).await?;
        ctx.progress(CopyProgress::InitialEstimate {
            known_files: estimates.estimated_files,
            known_size: estimates.estimated_size,
            discovery_starting: true,
        });

        // Get existing files from database
        let existing_files = prep_engine.get_database_files(&self.sources.paths).await?;
        ctx.log(format!("📊 Found {} known files from database", existing_files.len()));

        // Filter files through destination indexer (skip existing)
        let filtered_files = self.filter_files_by_existence(
            existing_files,
            &destination_indexer
        ).await?;

        ctx.log(format!("📋 {} files to copy after existence filtering", filtered_files.len()));

        // Initialize copy queue with filtered files
        let copy_queue = Arc::new(Mutex::new(CopyQueue::new()));
        copy_queue.lock().await.add_files(filtered_files);

        // Phase 2: Spawn source indexer for real-time discovery
        let indexer_config = IndexerJobConfig::delta_discovery(self.sources.clone());
        let mut source_indexer = DeltaIndexerJob::from_config(indexer_config);
        source_indexer.set_parent_job_id(ctx.job_id());

        ctx.log("🔍 Spawning real-time source discovery indexer");
        let source_handle = ctx.spawn_child_with_priority(source_indexer, JobPriority::HIGH).await?;

        // Phase 3: Concurrent execution - copy known files while discovering new ones
        let copy_queue_clone = copy_queue.clone();
        let copy_handle = tokio::spawn(async move {
            self.execute_copy_queue(copy_queue_clone, &ctx).await
        });

        // Listen for discoveries and add to queue in real-time
        let mut discoveries = indexer_handle.subscribe_progress();
        let mut discovery_active = true;

        while discovery_active {
            tokio::select! {
                // Handle discovery updates
                progress_result = discoveries.recv() => {
                    match progress_result {
                        Ok(CopyProgress::FileDiscovered { path, size, .. }) => {
                            ctx.log(format!("📁 Discovered new file: {}", path.display()));

                            let discovered_file = AnalyzedFile {
                                source_path: path,
                                destination_path: PathBuf::new(),
                                size,
                                is_completed: false,
                            };

                            copy_queue.lock().await.add_file(discovered_file);
                        }
                        Ok(CopyProgress::DiscoveryComplete { total_discovered, discovery_size }) => {
                            ctx.log(format!("✅ Discovery complete: {} new files ({} bytes)",
                                total_discovered, discovery_size));
                            discovery_active = false;
                        }
                        _ => {} // Other progress types
                    }
                }

                // Check if copy is done (but continue until discovery complete)
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Periodic progress updates
                    let queue_stats = copy_queue.lock().await.get_stats();
                    ctx.progress(CopyProgress::OverallProgress {
                        copied_files: queue_stats.completed_files,
                        total_files: queue_stats.total_files,
                        copied_bytes: queue_stats.completed_bytes,
                        total_bytes: queue_stats.total_bytes,
                        discovery_active,
                    });
                }
            }
        }

        // Wait for both copy completion and indexer completion
        let copy_result = copy_handle.await??;
        let indexer_result = indexer_handle.await?;

        ctx.log("🎉 Progressive copy completed - all files copied including discoveries");

        Ok(FileCopyOutput {
            copied_files: copy_result.copied_files,
            total_size: copy_result.total_size,
            discovered_files: indexer_result.discovered_count,
        })
    }
}

/// Thread-safe copy queue that can be expanded during execution
#[derive(Debug)]
pub struct CopyQueue {
    files: VecDeque<AnalyzedFile>,
    completed_count: u64,
    completed_bytes: u64,
    total_bytes: u64,
}

impl CopyQueue {
    pub fn new() -> Self {
        Self {
            files: VecDeque::new(),
            completed_count: 0,
            completed_bytes: 0,
            total_bytes: 0,
        }
    }

    pub fn add_files(&mut self, files: Vec<AnalyzedFile>) {
        for file in files {
            self.total_bytes += file.size;
            self.files.push_back(file);
        }
    }

    pub fn add_file(&mut self, file: AnalyzedFile) {
        self.total_bytes += file.size;
        self.files.push_back(file);
    }

    pub fn next_file(&mut self) -> Option<AnalyzedFile> {
        self.files.pop_front()
    }

    pub fn mark_completed(&mut self, size: u64) {
        self.completed_count += 1;
        self.completed_bytes += size;
    }

    pub fn get_stats(&self) -> QueueStats {
        QueueStats {
            total_files: self.completed_count + self.files.len() as u64,
            completed_files: self.completed_count,
            total_bytes: self.total_bytes,
            completed_bytes: self.completed_bytes,
            remaining_files: self.files.len() as u64,
        }
    }
}
```

### 5. Required Job System Enhancements

Based on the job system research, we need these implementations:

```rust
/// Enhanced JobContext with child job spawning
impl JobContext<'_> {
    /// Spawn a child job with priority (NEEDS IMPLEMENTATION)
    pub async fn spawn_child_with_priority<J>(&self, job: J, priority: JobPriority) -> JobResult<JobHandle<J::Output>>
    where
        J: Job + Send + 'static,
        J::Output: Send + 'static,
    {
        // Create child job record in database with parent_job_id
        let child_id = self.job_manager.create_child_job(&job, self.job_id, priority).await?;

        // Spawn in task system with specified priority
        let handle = self.task_system.spawn_with_priority(
            JobExecutor::new(job, child_id, self.job_manager.clone()),
            priority
        ).await?;

        // Track child handle
        self.child_handles.lock().unwrap().push(handle.clone());

        Ok(handle)
    }

    /// Subscribe to child job progress updates
    pub fn subscribe_child_progress(&self, child_id: JobId) -> broadcast::Receiver<Progress> {
        self.job_manager.subscribe_job_progress(child_id)
    }
}

/// Enhanced Progress types for structured communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Progress {
    Percentage(f64),
    Message(String),
    Structured(serde_json::Value), // For type-safe communication between jobs
}
```

## Key Benefits

### ✅ **Instant User Feedback**

- Database query provides immediate estimates and file list
- User sees copying start within 100ms
- Real progress from the beginning

### ✅ **Complete File Coverage**

- Database provides filtered file list (what's indexed)
- Real-time indexer finds unindexed files (what's missing)
- Combined = complete file coverage including .git, node_modules, etc.

### ✅ **Optimal Performance**

- No wasted time - copying and discovery happen concurrently
- Leverages existing database infrastructure
- Only discovers files NOT in database (delta approach)

### ✅ **Real-time Progress**

- User sees files being discovered in real-time
- Copy progress updates as new files are found
- Smooth progress reporting throughout

### ✅ **Scalable Architecture**

- Works for any directory size
- Efficient for both small and large operations
- Graceful handling of mixed indexed/unindexed content

### ✅ **Destination Pre-indexing Optimization**

- Pre-compute complete "what already exists" map before any copying
- Instant skip decisions for files that already exist at destination
- Eliminates per-file existence checks during copy operations
- Non-blocking fallback to filesystem checks while destination indexing runs
- Massive performance gain for copy operations to existing directories

## Example Flow

```rust
// User copies directory with mixed content
sources = ["/home/projects/myapp/"]

// Phase 1: Instant Database Start (< 100ms)
database_files = [
    "/home/projects/myapp/src/main.rs",      // Was indexed
    "/home/projects/myapp/README.md",       // Was indexed
    "/home/projects/myapp/package.json",    // Was indexed
    // Missing: .git/, node_modules/, .env (were filtered)
]
→ Start copying 3 known files immediately
→ Show: "Copying 3 files (15 KB), discovering more..."

// Phase 2: Real-time Discovery (concurrent with copying)
indexer discovers:
    "/home/projects/myapp/.git/config"      // Not in DB - report to parent
    "/home/projects/myapp/node_modules/..."  // Not in DB - report to parent
    "/home/projects/myapp/.env"             // Not in DB - report to parent
→ Add to copy queue as discovered
→ Update: "Found 1,247 more files, copying..."

// Phase 3: Complete Coverage
→ All files copied (database + discovered)
→ Final: "Copied 1,250 files (125 MB) including all hidden/filtered files"
```

## Performance Characteristics

| Scenario                                            | Current Behavior                   | With Hybrid Design                           |
| --------------------------------------------------- | ---------------------------------- | -------------------------------------------- |
| **Small directory (100 files, all indexed)**        | 5s filesystem scan                 | ~50ms database + instant start               |
| **Large directory (10K files, 80% indexed)**        | 20s filesystem scan + 0%→100% jump | ~200ms database start + smooth progress      |
| **Mixed indexed/new content**                       | 20s full scan                      | Database files copying + real-time discovery |
| **Heavily filtered directory (.git, node_modules)** | 20s scan → partial copy            | Database start + complete discovery          |

## Implementation Roadmap

### Phase 1: Foundation

- [ ] Implement `spawn_child_with_priority()` in JobContext
- [ ] Add structured Progress types for parent-child communication
- [ ] Create HybridPreparationEngine with database query methods
- [ ] Build thread-safe CopyQueue with dynamic expansion

### Phase 2: Delta Discovery

- [ ] Implement DeltaIndexerJob that only reports files not in database
- [ ] Add database existence checking for discovered files
- [ ] Create IndexMode::DeltaDiscovery for child indexer jobs
- [ ] Test parent-child progress communication

### Phase 3: Copy Integration

- [ ] Update FileCopyJob to use hybrid execution model
- [ ] Implement concurrent copying while discovery runs
- [ ] Add real-time queue expansion as files are discovered
- [ ] Ensure completion coordination between copy and discovery

### Phase 4: Polish & Optimization

- [ ] Add comprehensive error handling and fallbacks
- [ ] Implement progress reporting with discovery status
- [ ] Add performance monitoring and metrics
- [ ] Test with various directory sizes and content types

## File Existence and Overwrite Handling

### Current Behavior Analysis

The existing copy strategy implementations in `strategy.rs:226-348` have a critical gap:

```rust
// Current implementation in copy_file_streaming
if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent).await?;
}
// ❌ NO existence check - files are overwritten by default
```

**Issues with Current Approach:**

1. **Silent Overwrites**: Files are overwritten without user awareness
2. **Wasted Work**: Re-copying files that already exist at destination
3. **No Resume Support**: Failed copy operations restart from scratch
4. **Race Conditions**: Progressive discovery could attempt to copy same file twice

### Enhanced File Existence Strategy

Our progressive copy design needs intelligent existence checking:

```rust
/// Enhanced copy queue with existence checking
impl CopyQueue {
    /// Add file to queue only if it doesn't exist at destination (or needs overwrite)
    pub async fn add_file_with_check(&mut self, file: AnalyzedFile, overwrite_mode: OverwriteMode) -> bool {
        // Calculate destination path
        let destination = self.calculate_destination_path(&file.source_path);

        match overwrite_mode {
            OverwriteMode::Skip => {
                if destination.exists() {
                    // File exists - skip it entirely
                    self.skipped_count += 1;
                    return false;
                }
            }
            OverwriteMode::PromptUser => {
                if destination.exists() {
                    // Add to conflict resolution queue
                    self.conflicts.push(FileConflict {
                        source: file.source_path.clone(),
                        destination: destination.clone(),
                        source_size: file.size,
                        dest_size: destination.metadata().unwrap().len(),
                        // ... timestamps, etc.
                    });
                    return false;
                }
            }
            OverwriteMode::Replace => {
                // Always copy, overwriting if needed
            }
            OverwriteMode::SkipIfNewer => {
                if let Ok(dest_meta) = destination.metadata() {
                    if let Ok(source_meta) = file.source_path.metadata() {
                        if dest_meta.modified().unwrap() >= source_meta.modified().unwrap() {
                            self.skipped_count += 1;
                            return false;
                        }
                    }
                }
            }
        }

        self.add_file(file);
        true
    }
}

#[derive(Debug, Clone)]
pub enum OverwriteMode {
    /// Skip files that already exist
    Skip,
    /// Prompt user for each conflict
    PromptUser,
    /// Always replace existing files
    Replace,
    /// Skip if destination is newer
    SkipIfNewer,
}

#[derive(Debug)]
pub struct FileConflict {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub source_size: u64,
    pub dest_size: u64,
    pub source_modified: SystemTime,
    pub dest_modified: SystemTime,
}
```

### Integration with Progressive Discovery

The hybrid approach needs careful deduplication:

```rust
impl FileCopyJob {
    async fn add_discovered_file(&self, discovered_file: AnalyzedFile) -> JobResult<()> {
        let destination = self.calculate_destination_path(&discovered_file.source_path);

        // Check if we've already processed this file from database
        if self.processed_files.contains(&discovered_file.source_path) {
            // Already copied from database - skip discovery
            return Ok(());
        }

        // Check existence at destination with user's overwrite preference
        let should_add = self.copy_queue.lock().await
            .add_file_with_check(discovered_file, self.overwrite_mode).await;

        if should_add {
            self.processed_files.insert(discovered_file.source_path.clone());
        }

        Ok(())
    }
}
```

### Resume/Restart Scenarios

Progressive copy operations should support resumption:

```rust
/// Resume-aware preparation that skips already completed files
impl HybridPreparationEngine {
    pub async fn get_database_files_with_resume(
        &self,
        sources: &[SdPath],
        resume_state: Option<&CopyResumeState>
    ) -> JobResult<Vec<AnalyzedFile>> {
        let mut all_files = self.get_database_files(sources).await?;

        if let Some(resume) = resume_state {
            // Filter out files that were already successfully copied
            all_files.retain(|file| {
                let destination = self.calculate_destination_path(&file.source_path);
                !resume.completed_files.contains(&destination) ||
                !destination.exists() ||
                // Check if file was modified since last copy
                self.file_needs_refresh(&file.source_path, &destination)
            });

            // Log resume info
            let skipped = resume.completed_files.len();
            ctx.log(format!("📋 Resuming copy: {} files already completed", skipped));
        }

        Ok(all_files)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopyResumeState {
    pub completed_files: HashSet<PathBuf>,
    pub total_bytes_copied: u64,
    pub last_checkpoint: SystemTime,
}
```

### Performance Optimizations

Batch existence checking for better performance:

```rust
/// Batch check multiple files for existence (async I/O optimization)
async fn batch_check_existence(files: &[PathBuf]) -> Vec<bool> {
    let futures = files.iter().map(|path| async move {
        tokio::fs::metadata(path).await.is_ok()
    });

    futures::future::join_all(futures).await
}

/// Use filesystem-specific optimizations where available
impl ExistenceChecker {
    #[cfg(target_os = "linux")]
    async fn fast_existence_check(&self, paths: &[PathBuf]) -> Vec<bool> {
        // Use statx() system call for batch metadata queries
    }

    #[cfg(target_os = "macos")]
    async fn fast_existence_check(&self, paths: &[PathBuf]) -> Vec<bool> {
        // Use batch getattrlist() calls
    }
}
```

### User Experience Considerations

```rust
/// Progress updates that include conflict information
#[derive(Debug, Clone)]
pub enum CopyProgress {
    // ... existing variants ...

    /// File conflicts detected requiring user decision
    ConflictsDetected {
        conflicts: Vec<FileConflictSummary>,
        pending_files: u64,
    },

    /// Files being skipped due to existence
    FilesSkipped {
        skipped_count: u64,
        reason: SkipReason,
    },

    /// Resume operation detected
    ResumeDetected {
        already_completed: u64,
        remaining_files: u64,
    },
}

#[derive(Debug, Clone)]
pub enum SkipReason {
    AlreadyExists,
    DestinationNewer,
    UserChoice,
}
```

### Integration Points

1. **CLI Integration**: Add `--overwrite` flag with options (skip, replace, prompt, skip-newer)
2. **Job Checkpointing**: Save completed files to enable resume
3. **Progress Reporting**: Show skipped files and conflicts in progress updates
4. **Error Handling**: Graceful handling of permission errors during existence checks

This existence checking strategy ensures our progressive copy design is both efficient and user-friendly, preventing wasted work while giving users control over overwrite behavior.

## Conclusion

This hybrid design transforms copy operations by combining the best of both approaches:

- **Database optimization** for instant startup with known files
- **Real-time discovery** for complete coverage including filtered files
- **Concurrent execution** for optimal performance and user experience
- **Intelligent existence checking** for efficient resume and conflict handling

**Key insight**: Instead of choosing between database OR filesystem, use database THEN filesystem with real-time coordination. This provides immediate user feedback while ensuring complete file coverage, delivering Finder-class UX with Spacedrive's unique advantages.
