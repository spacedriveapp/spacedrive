# Progressive Copy Design: Hybrid Database + Real-time Discovery

## Overview

This document outlines a design for implementing progressive copy operations that provide real-time preparation feedback, similar to macOS Finder, while leveraging Spacedrive's indexed data for instant initial preparation and real-time discovery for complete file coverage.

This design is implemented in phases, building on Spacedrive's existing job system infrastructure and gradually adding more sophisticated features.

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
- **Cross-device awareness** using SdPath for distributed operations
- **Real-time discovery** for files not in database (filtered or new)
- **Concurrent processing** - copying known files while discovering new ones
- **Complete coverage** ensuring all user-selected files are copied

### Key Insight: Hybrid Approach

Since global filters mean database never contains complete file sets, we use:

- **Database for instant start**: Copy known files immediately
- **Real-time discovery for gaps**: Find and add unindexed files
- **Incremental implementation**: Build complexity gradually

## Implementation Strategy

This design is implemented in three phases, each building on the previous:

### Phase 1: Enhanced Progress (Immediate Implementation)
- Improve existing copy job with detailed progress reporting
- Add database querying for instant estimates
- Use existing job infrastructure

### Phase 2: Database Integration (Medium Term)
- Query indexed files for instant startup
- Pre-compute destination existence maps
- Enhanced progress with real data

### Phase 3: Concurrent Discovery (Advanced)
- Parent-child job communication infrastructure
- Real-time file discovery during copying
- Complete file coverage including filtered content

## Architecture Overview

```rust
┌─────────────────────────────────────────────────────────────────┐
│                    Progressive Copy Architecture                │
├─────────────────────────────────────────────────────────────────┤
│  Phase 1: Enhanced Progress (Using Existing Infrastructure)    │
│  ├─ Database size/count estimates (< 100ms)                    │
│  ├─ Detailed progress using Progress::structured()             │
│  ├─ SdPath-aware processing for cross-device operations       │
│  └─ Enhanced error handling and resume capabilities           │
├─────────────────────────────────────────────────────────────────┤
│  Phase 2: Database Integration                                 │
│  ├─ Query known files from database for instant startup       │
│  ├─ Destination pre-scanning for existence checking           │
│  ├─ Smart progress updates with real file data                │
│  └─ Preparation phase with "Preparing to copy..." feedback    │
├─────────────────────────────────────────────────────────────────┤
│  Phase 3: Concurrent Discovery (Future)                       │
│  ├─ Parent-child job spawning infrastructure                  │
│  ├─ Real-time discovery of unindexed files                    │
│  ├─ Concurrent copying and discovery                          │
│  └─ Complete file coverage including filtered content         │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Enhanced Progress System (Phase 1)

The first phase focuses on dramatically improving the user experience by providing rich, real-time feedback during copy operations. Currently, users see a frustrating "black box" experience where progress jumps from 0% to 100% with no intermediate updates, particularly during the preparation phase where the system is analyzing files.

The enhanced progress system introduces multiple distinct phases that users can understand and follow. When a copy operation begins, users will immediately see a "Database Query" phase where the system quickly checks if any of the selected files are already indexed in Spacedrive's database. This happens almost instantaneously (under 100ms) and provides immediate feedback that something is happening.

Next comes a "Preparation" phase that replaces the current silent file analysis period. Instead of users waiting 20+ seconds with no feedback, they'll see "Preparing to copy..." with real-time updates showing how many files have been discovered and the total size being calculated. This mirrors the excellent user experience provided by macOS Finder.

During the actual "Copying" phase, users see detailed information about which specific file is currently being processed, along with accurate progress percentages based on real file sizes rather than simple file counts. The system tracks both files completed and bytes transferred, providing meaningful progress updates.

Finally, a "Complete" phase gives users clear confirmation that the operation finished successfully, including summary statistics about what was copied.

This enhanced progress system builds entirely on Spacedrive's existing job infrastructure, using the established `Progress::structured()` pattern and `JobProgress` trait. It requires no new infrastructure and can be implemented immediately as an enhancement to the current `FileCopyJob`.

Build on the existing job infrastructure with improved progress reporting:

```rust
/// Enhanced progress data for copy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyProgressData {
    pub phase: CopyPhase,
    pub current_file: Option<String>,
    pub files_copied: u64,
    pub total_files: u64,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub discovery_active: bool,
    pub preparation_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CopyPhase {
    Initializing,
    DatabaseQuery,
    Preparation,
    Copying,
    Complete,
}

impl JobProgress for CopyProgressData {}

/// Enhanced FileCopyJob with preparation phase
impl FileCopyJob {
    async fn run_with_preparation(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Phase 1: Database estimates (if available)
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::DatabaseQuery,
            current_file: None,
            files_copied: 0,
            total_files: 0,
            bytes_copied: 0,
            total_bytes: 0,
            discovery_active: false,
            preparation_complete: false,
        }));

        let estimated_info = self.get_database_estimates(&ctx).await?;
        
        // Phase 2: Preparation with real progress
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::Preparation,
            total_files: estimated_info.estimated_files,
            total_bytes: estimated_info.estimated_size,
            preparation_complete: false,
            ..Default::default()
        }));

        // Phase 3: Enhanced copying with detailed progress
        self.run_enhanced_copy(&ctx, estimated_info).await
    }
}
```

### 2. Database Integration (Phase 2)

The second phase leverages Spacedrive's unique advantage as an indexed file manager to provide instant startup times for copy operations. While traditional file managers must perform expensive filesystem traversals to count files and calculate sizes, Spacedrive can potentially answer these questions instantly by querying its existing database of indexed files.

When a user initiates a copy operation, the system first attempts to gather size and file count estimates from the database. For paths that have been previously indexed, this query completes in milliseconds and provides immediate feedback to the user about the scope of the operation. The user sees realistic estimates like "Preparing to copy 1,234 files (2.1 GB)" almost instantly, rather than waiting for a lengthy filesystem scan.

The database integration is designed to work gracefully with Spacedrive's location-based indexing system. When analyzing source paths, the system first determines which managed locations contain the selected files, then queries the entries table for aggregate statistics. This approach leverages the existing database schema without requiring new tables or complex migrations.

For paths that aren't indexed or are outside managed locations, the system falls back to traditional filesystem scanning, but users get clear feedback about which parts of the operation are instant (from database) versus which require real-time discovery.

Query existing indexed data for instant estimates and known files:

```rust
/// Database query engine for copy preparation
pub struct CopyPreparationEngine {
    database: Arc<DatabaseConnection>,
}

impl CopyPreparationEngine {
    /// Get instant estimates from database
    pub async fn get_database_estimates(&self, sources: &[SdPath]) -> JobResult<EstimateData> {
        let mut total_size = 0;
        let mut total_files = 0;

        for source in sources {
            if source.is_local() {
                // Query local database for this path
                if let Some(estimates) = self.query_path_estimates(source).await? {
                    total_size += estimates.size;
                    total_files += estimates.file_count;
                }
            } else {
                // For remote paths, we'll need cross-device queries (future enhancement)
                ctx.log(format!("Remote source detected: {}", source.display()));
            }
        }

        Ok(EstimateData {
            estimated_size: total_size,
            estimated_files: total_files,
            is_complete: false,
        })
    }

    /// Query indexed files for a path (leverages existing database schema)
    async fn query_path_estimates(&self, source: &SdPath) -> JobResult<Option<PathEstimates>> {
        let source_path = source.as_local_path()
            .ok_or_else(|| JobError::execution("Source must be local"))?;

        // Find location containing this source
        let location = self.find_location_for_path(source_path).await?;
        let Some(location) = location else {
            return Ok(None); // Path not in any indexed location
        };

        // Use existing entries table to get aggregated data
        // This leverages the existing database schema
        let estimates = self.query_location_path_stats(&location, source_path).await?;
        
        Ok(estimates)
    }
}

#[derive(Debug)]
pub struct EstimateData {
    pub estimated_size: u64,
    pub estimated_files: u64,
    pub is_complete: bool,
}

#[derive(Debug)]
pub struct PathEstimates {
    pub size: u64,
    pub file_count: u64,
}
```

### 3. Destination Pre-analysis (Phase 2)

This component introduces intelligent destination analysis to optimize copy operations by avoiding unnecessary work. One of the most frustrating aspects of current copy operations is that they often re-copy files that already exist at the destination, wasting time and bandwidth.

The destination pre-analysis system scans the destination directory before copying begins, building a complete map of existing files. This enables smart skip logic where the system can instantly decide whether each source file needs to be copied or can be skipped because an identical file already exists at the destination.

The analysis leverages Spacedrive's existing IndexerJob infrastructure with ephemeral configuration, meaning it performs a thorough scan without persisting results to the database. This reuses proven, optimized filesystem traversal code rather than implementing custom scanning logic.

The system is designed to be non-blocking - if destination analysis is taking too long, copying can begin with fallback filesystem checks for individual files. However, when destination analysis completes quickly (which it often will for directories with reasonable file counts), it provides massive performance benefits by eliminating redundant copies entirely.

This approach is particularly beneficial when copying to directories that already contain some of the source files, such as backing up a project directory to an existing backup location, or syncing files between devices where partial overlaps are common.

Use existing IndexerJob patterns for destination scanning:

```rust
/// Destination analysis using existing indexer infrastructure
pub struct DestinationAnalyzer {
    destination: SdPath,
}

impl DestinationAnalyzer {
    /// Analyze destination using existing IndexerJob
    pub async fn analyze_destination(&self, ctx: &JobContext<'_>) -> JobResult<DestinationInfo> {
        ctx.log("Analyzing destination for existence checking");

        // Use existing IndexerJob with ephemeral configuration
        let config = IndexerJobConfig {
            location_id: None, // Ephemeral
            path: self.destination.clone(),
            mode: IndexMode::Shallow, // Just metadata
            scope: IndexScope::Recursive,
            persistence: IndexPersistence::Ephemeral,
            max_depth: None,
        };

        let mut indexer = IndexerJob::new(config);
        
        // For now, run synchronously in preparation phase
        // Later: convert to child job when infrastructure is ready
        let result = indexer.run(ctx.clone()).await?;
        
        self.extract_destination_info(result).await
    }

    async fn extract_destination_info(&self, result: IndexerOutput) -> JobResult<DestinationInfo> {
        // Extract file existence information from indexer results
        let existing_files = if let Some(ephemeral_results) = result.ephemeral_results {
            let index = ephemeral_results.read().await;
            index.entries.keys().cloned().collect()
        } else {
            HashSet::new()
        };

        Ok(DestinationInfo {
            existing_files,
            total_files: result.stats.files,
            total_size: result.stats.bytes,
        })
    }
}

#[derive(Debug)]
pub struct DestinationInfo {
    pub existing_files: HashSet<PathBuf>,
    pub total_files: u64,
    pub total_size: u64,
}
```

## Parent-Child Job Communication (Phase 3)

The third phase represents the most sophisticated enhancement, introducing true concurrent processing where file discovery and copying happen simultaneously. This requires significant new infrastructure for parent-child job communication, but delivers the ultimate user experience where copy operations can handle any content with real-time discovery feedback.

The core challenge this phase solves is complete file coverage. Due to Spacedrive's global filters, the database never contains a complete record of all files in a directory - filtered content like .git folders, node_modules, temporary files, and other excluded content exists on disk but not in the index. Traditional copy operations would miss this content or require expensive full filesystem scans.

The parent-child architecture enables a hybrid approach: the parent copy job immediately begins copying files it knows about from the database, while simultaneously spawning child discovery jobs that scan the filesystem for unindexed content. As the child jobs discover new files, they stream this information back to the parent, which dynamically adds them to the copy queue.

This creates a fluid, responsive experience where users see copying begin immediately with known files, then watch in real-time as additional files are discovered and added to the operation. The progress indicators smoothly update to reflect the expanding scope, and users get clear feedback about both the copying progress and discovery activity.

The infrastructure required is substantial but follows established patterns. Child jobs are spawned with priority levels, ensuring discovery doesn't interfere with copying performance. Progress communication uses structured messaging to provide type-safe coordination between parent and child processes. The copy queue becomes thread-safe and dynamically expandable, allowing safe concurrent access from both copying and discovery operations.

This section outlines the infrastructure needed for advanced concurrent discovery features. This is a future enhancement that will be implemented after the foundation is solid.

### Required Infrastructure

```rust
/// Enhanced JobContext with child job spawning capabilities
impl JobContext<'_> {
    /// Spawn a child job with priority (NEEDS IMPLEMENTATION)
    pub async fn spawn_child_with_priority<J>(&self, job: J, priority: JobPriority) -> JobResult<JobHandle>
    where
        J: Job + JobHandler + Send + 'static,
    {
        // Create child job record in database with parent_job_id
        let child_id = self.job_manager.create_child_job(&job, self.id, priority).await?;

        // Spawn in task system with specified priority
        let handle = self.task_system.spawn_with_priority(
            JobExecutor::new(job, child_id, self.job_manager.clone()),
            priority
        ).await?;

        // Track child handle for cleanup
        self.child_handles.lock().await.push(handle.clone());

        Ok(handle)
    }

    /// Subscribe to child job progress updates
    pub fn subscribe_child_progress(&self, child_id: JobId) -> broadcast::Receiver<Progress> {
        self.job_manager.subscribe_job_progress(child_id)
    }
}

/// Job priority levels for task scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}
```

### Delta Discovery Job (Future)

The delta discovery job represents a specialized variant of the existing IndexerJob, optimized for finding only files that aren't already known to the database. Rather than performing a comprehensive indexing operation, it focuses specifically on identifying gaps in Spacedrive's knowledge.

When processing discovered files, the delta job first checks whether each file exists in the database before reporting it to the parent copy job. This prevents duplicate work - if a file is already known from the initial database query, there's no need to "discover" it again. Only truly new files are streamed back to the parent as discovery progress events.

This approach minimizes communication overhead and prevents the copy queue from being flooded with redundant file entries. The parent job receives a clean stream of genuinely new discoveries, which it can immediately add to the copy queue and update progress indicators accordingly.

The delta discovery job maintains the same filesystem traversal efficiency as the regular IndexerJob but adds the database checking layer. This creates some overhead per file, but the trade-off is worthwhile given the alternative of either missing filtered content entirely or performing expensive full filesystem scans without database optimization.

```rust
/// Specialized indexer job that only reports files NOT in database
pub struct DeltaDiscoveryJob {
    sources: Vec<SdPath>,
    parent_job_id: JobId,
}

impl DeltaDiscoveryJob {
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
            ctx.progress(Progress::structured(DiscoveryProgress {
                discovered_file: DiscoveredFile {
                    path: path.to_path_buf(),
                    size: metadata.len(),
                    kind: if metadata.is_dir() { "directory" } else { "file" }.to_string(),
                },
                batch_number: self.current_batch,
            }));
        }

        // If exists in DB, parent already has it - don't report
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryProgress {
    pub discovered_file: DiscoveredFile,
    pub batch_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size: u64,
    pub kind: String,
}

impl JobProgress for DiscoveryProgress {}
```

### Concurrent Copy Architecture (Future)

The concurrent copy architecture enables the dynamic expansion of copy operations while they're running. Unlike traditional copy jobs that have a fixed list of files determined at startup, this system maintains a thread-safe queue that can grow as new files are discovered.

The core challenge is coordinating between the copying process (which removes files from the queue) and the discovery process (which adds files to the queue), while maintaining accurate progress reporting throughout. The queue uses atomic operations for counters and proper locking for the file list to ensure thread safety without sacrificing performance.

Progress reporting becomes more sophisticated, tracking not just files copied but also discovery activity. Users see indicators like "Copying files... (1,247 found so far)" which updates in real-time as both copying and discovery progress. The system must coordinate completion - the copy operation isn't finished until both all known files are copied AND all discovery jobs have completed.

This architecture enables the ultimate user experience where copying begins immediately with known files, smoothly transitions to handle discovered content, and provides complete coverage of all user-selected files regardless of their indexing status.

```rust
/// Thread-safe copy queue that can be expanded during execution
#[derive(Debug)]
pub struct DynamicCopyQueue {
    files: Arc<Mutex<VecDeque<AnalyzedFile>>>,
    completed_count: Arc<AtomicU64>,
    completed_bytes: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
}

impl DynamicCopyQueue {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(VecDeque::new())),
            completed_count: Arc::new(AtomicU64::new(0)),
            completed_bytes: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Add file to queue (called by discovery process)
    pub async fn add_discovered_file(&self, file: AnalyzedFile) {
        self.total_bytes.fetch_add(file.size, Ordering::Relaxed);
        self.files.lock().await.push_back(file);
    }

    /// Get next file to copy (called by copy process)
    pub async fn next_file(&self) -> Option<AnalyzedFile> {
        self.files.lock().await.pop_front()
    }

    /// Mark file as completed
    pub fn mark_completed(&self, size: u64) {
        self.completed_count.fetch_add(1, Ordering::Relaxed);
        self.completed_bytes.fetch_add(size, Ordering::Relaxed);
    }
}
```

## Implementation Examples

### Phase 1: Enhanced Progress Implementation

This example shows how to immediately improve the existing FileCopyJob with minimal changes to the current codebase. The enhancement focuses on providing detailed progress feedback through multiple distinct phases, transforming the current "black box" experience into something that rivals the best file managers.

The key insight is that this can be implemented entirely within the existing job framework without requiring new infrastructure. By using the established `Progress::structured()` pattern, the enhanced copy job can provide rich progress data that front-end applications can display in sophisticated ways.

The implementation maintains full compatibility with the existing copy system while dramatically improving the user experience. Each phase transition provides clear feedback about what the system is doing, eliminating the frustrating periods of no feedback that characterize the current implementation.

```rust
/// Immediate improvement to existing FileCopyJob
impl FileCopyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Phase 1: Database estimates (if available)
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::DatabaseQuery,
            current_file: None,
            files_copied: 0,
            total_files: 0,
            bytes_copied: 0,
            total_bytes: 0,
            discovery_active: false,
            preparation_complete: false,
        }));

        // Try to get estimates from database
        let prep_engine = CopyPreparationEngine::new(ctx.library_db());
        let estimates = prep_engine.get_database_estimates(&self.sources.paths).await
            .unwrap_or_else(|_| EstimateData { 
                estimated_size: 0, 
                estimated_files: 0, 
                is_complete: false 
            });

        // Phase 2: Enhanced preparation
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::Preparation,
            total_files: estimates.estimated_files,
            total_bytes: estimates.estimated_size,
            preparation_complete: false,
            ..Default::default()
        }));

        // Calculate actual totals (existing logic enhanced)
        let actual_total_bytes = self.calculate_total_size(&ctx).await?;
        
        // Phase 3: Copy with detailed progress
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::Copying,
            total_files: self.sources.paths.len() as u64,
            total_bytes: actual_total_bytes,
            preparation_complete: true,
            ..Default::default()
        }));

        // Enhanced copy loop with better progress reporting
        for (index, source) in self.sources.paths.iter().enumerate() {
            ctx.progress(Progress::structured(CopyProgressData {
                phase: CopyPhase::Copying,
                current_file: Some(source.display()),
                files_copied: index as u64,
                total_files: self.sources.paths.len() as u64,
                total_bytes: actual_total_bytes,
                preparation_complete: true,
                ..Default::default()
            }));

            // Existing copy logic...
            let result = self.copy_single_file(source, &ctx).await;
            // ... handle result
        }

        // Phase 4: Complete
        ctx.progress(Progress::structured(CopyProgressData {
            phase: CopyPhase::Complete,
            files_copied: self.sources.paths.len() as u64,
            total_files: self.sources.paths.len() as u64,
            bytes_copied: actual_total_bytes,
            total_bytes: actual_total_bytes,
            preparation_complete: true,
            discovery_active: false,
        }));

        // Return existing output...
        Ok(self.existing_output)
    }
}
```

## Key Benefits

### **Instant User Feedback**

- Database query provides immediate estimates and file list
- User sees copying start within 100ms
- Real progress from the beginning

### **Complete File Coverage**

- Database provides filtered file list (what's indexed)
- Real-time indexer finds unindexed files (what's missing)
- Combined = complete file coverage including .git, node_modules, etc.

### **Optimal Performance**

- No wasted time - copying and discovery happen concurrently
- Leverages existing database infrastructure
- Only discovers files NOT in database (delta approach)

### **Real-time Progress**

- User sees files being discovered in real-time
- Copy progress updates as new files are found
- Smooth progress reporting throughout

### **Scalable Architecture**

- Works for any directory size
- Efficient for both small and large operations
- Graceful handling of mixed indexed/unindexed content

### **Destination Pre-indexing Optimization**

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

### Phase 1: Enhanced Progress (Immediate - 1-2 weeks)

Building on existing infrastructure with immediate improvements:

- [ ] **Enhanced Progress Types**
  - Add `CopyProgressData` struct with detailed phase information
  - Implement `JobProgress` trait for structured progress
  - Update FileCopyJob to use `Progress::structured()`

- [ ] **Database Estimates** 
  - Create `CopyPreparationEngine` for database queries
  - Query existing indexed files for instant size/count estimates
  - Add fallback when database queries fail

- [ ] **Improved UX**
  - Add preparation phase with "Preparing to copy..." messaging
  - Show current file being processed
  - Display realistic progress percentages

**Success Criteria**: Copy operations show meaningful progress from start to finish

### Phase 2: Database Integration (Medium Term - 3-4 weeks)

Leveraging Spacedrive's indexed data for faster preparation:

- [ ] **Database Query Infrastructure**
  - Implement location-aware file querying
  - Add support for cross-device path resolution
  - Create efficient database queries for file aggregation

- [ ] **Destination Analysis**
  - Use IndexerJob for destination pre-scanning
  - Build existence maps for skip logic
  - Optimize copy operations by avoiding existing files

- [ ] **Smart Progress Updates**
  - Use real database file counts instead of estimates
  - Show preparation progress during destination analysis
  - Display skip counts and conflict information

**Success Criteria**: Large directory copies start instantly with accurate estimates

### Phase 3: Concurrent Discovery (Advanced - 8-12 weeks)

Full parent-child job communication for complete file coverage:

- [ ] **Child Job Infrastructure**
  - Implement `spawn_child_with_priority()` in JobContext
  - Add parent-child progress communication
  - Create job priority system and task scheduling

- [ ] **Delta Discovery Jobs**
  - Build `DeltaDiscoveryJob` that only reports unindexed files
  - Add real-time file discovery during copying
  - Implement database existence checking for discoveries

- [ ] **Concurrent Copy Architecture**
  - Create thread-safe `DynamicCopyQueue`
  - Enable copying known files while discovering new ones
  - Coordinate completion between copy and discovery processes

- [ ] **Complete File Coverage**
  - Ensure ALL user-selected files are copied (including filtered)
  - Handle .git, node_modules, and other filtered content
  - Provide real-time discovery feedback

**Success Criteria**: Copy operations include all files with real-time discovery, matching Finder's UX

## File Existence and Overwrite Handling

### Current Behavior Analysis

The existing copy strategy implementations in `strategy.rs:226-348` have a critical gap:

```rust
// Current implementation in copy_file_streaming
if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent).await?;
}
// NO existence check - files are overwritten by default
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
            ctx.log(format!("Resuming copy: {} files already completed", skipped));
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
