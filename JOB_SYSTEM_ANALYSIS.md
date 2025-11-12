# Spacedrive Job System Analysis

## Core Job Architecture

### Job Trait Hierarchy

The job system is built on a layered trait model:

1. **Job Trait** (Base)
   - Serialization/Deserialization support
   - Metadata: NAME, RESUMABLE, VERSION, DESCRIPTION
   - Schema generation for runtime introspection

2. **JobHandler Trait** (Execution)
   - `run()` - Main async job execution
   - `on_pause()` - Pause handler (optional)
   - `on_resume()` - Resume handler (optional)
   - `on_cancel()` - Cancellation handler (optional)
   - Associated type: `Output` (converts to JobOutput)

3. **DynJob Trait** (Type Erasure)
   - `job_name()` - Returns static job name
   - Enables dynamic dispatch of jobs

4. **SerializableJob Trait** (State Persistence)
   - `serialize_state()` - Msgpack serialization
   - `deserialize_state()` - Msgpack deserialization
   - Blanket implementation for all Jobs

### JobContext (Execution Environment)

Provides jobs with access to:
- `library()` - Access to library instance
- `check_interrupt()` - Pause/cancel checking
- `progress()` - Report progress updates
- `log()` / `log_error()` - Logging
- `checkpoint()` - Save resumable state
- `add_non_critical_error()` - Non-fatal error tracking
- `volume_manager()` - Volume backend access
- `library_db()` - Database connection

---

## All Jobs Found

### 1. **IndexerJob** Most Complex
**Location**: `core/src/ops/indexing/job.rs`
**Purpose**: Discover and index files in locations with content analysis

#### Configuration
- `IndexerJobConfig` with:
  - `location_id`: Optional (None for ephemeral)
  - `path`: SdPath to index
  - `mode`: Shallow | Content | Deep
  - `scope`: Current (single level) | Recursive
  - `persistence`: Persistent | Ephemeral
  - `max_depth`: Optional depth limit
  - `rule_toggles`: Filtering rules

#### Phases (5 total)
1. **Discovery** - Walks directory tree collecting entries
   - Outputs: `Vec<Vec<DirEntry>>` in batches
   - Atomic unit: Directory entry (file/dir/symlink)
   - Handles scope-aware discovery (current vs recursive)

2. **Processing** - Creates/updates database records
   - Input: Entry batches from discovery
   - Atomic unit: Single entry creation
   - Distinguishes persistent vs ephemeral

3. **Aggregation** - Calculates directory sizes
   - Input: All processed entries
   - Atomic unit: Directory aggregation
   - Skip for ephemeral jobs

4. **ContentIdentification** - Generates content hashes (CAS IDs)
   - Input: Entries needing content analysis
   - Atomic unit: Single file content hash
   - Conditional: Only if mode >= Content

5. **Complete** - Finished

#### Processing Patterns
- **Batch processing**: Entry discovery -> batch creation -> batch processing
- **Resumable state**: IndexerState carries current phase + all intermediate data
- **Ephemeral support**: In-memory Arc<RwLock<EphemeralIndex>> for non-persistent jobs
- **Volume abstraction**: Uses VolumeBackend for cloud/local paths
- **Mode-driven execution**: Shallow/Content/Deep modes skip phases

#### Key Characteristics
- **Single item unit**: DirEntry (one file/dir discovery)
- **Batch unit**: Vec<DirEntry> (up to batch_size entries)
- **Operations**: DB writes (creation), CAS ID generation (hashing)
- **Resumability**: Full state preservation including batches and phase

---

### 2. **FileCopyJob**
**Location**: `core/src/ops/files/copy/job.rs`
**Purpose**: Copy or move files with progress tracking

#### Configuration
- `sources`: SdPathBatch (multiple paths)
- `destination`: SdPath (single target)
- `options`: CopyOptions
  - `copy_method`: CopyMethod (Auto/LocalMove/Stream)
  - `delete_after_copy`: Move vs copy
  - `verify_checksum`: Optional integrity check

#### Phases (5 total)
1. **Initializing** - Setup
2. **DatabaseQuery** - Gather file size estimates from DB
3. **Preparation** - Calculate actual total size
4. **Copying** - Execute copy/move for each source
5. **Complete** - Finished

#### Processing Patterns
- **Strategy pattern**: CopyStrategyRouter selects strategy (LocalMove, Stream, etc.)
- **Progress aggregator**: Tracks bytes/files across sources
- **Callback-based**: Strategy reports progress via callback
- **Resumable**: Tracks `completed_indices` to skip finished sources

#### Key Characteristics
- **Single item unit**: SdPath (one source file/directory)
- **Batch unit**: None (processes sources sequentially)
- **Operations**: File I/O (read/write), strategy execution
- **Atomic operation**: Per-source copy (entire dir recursively)

---

### 3. **DeleteJob**
**Location**: `core/src/ops/files/delete/job.rs`
**Purpose**: Delete files with multiple modes

#### Configuration
- `targets`: SdPathBatch
- `mode`: Trash | Permanent | Secure
- `confirm_permanent`: Safety flag

#### Phases (1 main + strategy)
- **Validation** - Check targets exist
- **Strategy execution** - Uses DeleteStrategyRouter
  - Different strategies for same-device vs cross-device operations

#### Processing Patterns
- **Strategy pattern**: DeleteStrategyRouter selects strategy
- **Modes affect strategy**: Trash vs permanent deletion strategies differ
- **Atomic unit**: Individual path deletion

#### Key Characteristics
- **Single item unit**: SdPath
- **Batch unit**: SdPathBatch (but processed via strategy)
- **Operations**: File deletion
- **Atomic operation**: Per-path deletion (recursive for directories)

---

### 4. **DuplicateDetectionJob**
**Location**: `core/src/ops/files/duplicate_detection/job.rs`
**Purpose**: Find duplicate files using various comparison modes

#### Configuration
- `search_paths`: SdPathBatch
- `mode`: SizeOnly | ContentHash | NameAndSize | DeepScan
- `min_file_size`, `max_file_size`: Size filters
- `file_extensions`: Optional extension filter

#### Phases (2 main)
1. **GroupBySize** - Hash all files by size
   - Intermediate storage: `HashMap<u64, Vec<FileInfo>>`
   
2. **AnalysisPhase** (mode-specific)
   - SizeOnly: Group by size only
   - ContentHash: Generate hashes for same-size files
   - NameAndSize: Group by filename + size
   - DeepScan: Full content verification

#### Processing Patterns
- **Two-phase filtering**: Size grouping followed by deep analysis
- **Lazy hashing**: Only hash files in size groups with duplicates
- **Resumable state**: Maintains size_groups and processed_files

#### Key Characteristics
- **Single item unit**: FileInfo (one file to analyze)
- **Batch unit**: Files in same size group
- **Operations**: File hashing, content comparison
- **Atomic operation**: Content hash generation for single file

---

### 5. **ValidationJob**
**Location**: `core/src/ops/files/validation/job.rs`
**Purpose**: Validate file integrity and detect corruption

#### Configuration
- `targets`: SdPathBatch
- `mode`: Basic | Integrity | Corruption | Complete
- `verify_against_index`: Cross-check with database
- `check_permissions`: Validate file permissions

#### Phases (4 parallel analyses)
- **Basic**: File existence, size, modification time
- **Integrity**: CAS ID verification against database
- **Corruption**: Pattern detection, extension validation
- **Complete**: All of above

#### Processing Patterns
- **Per-file validation**: Each file gets full validation suite
- **Issue tracking**: Accumulates ValidationIssue objects
- **Severity levels**: Info, Warning, Error, Critical
- **Recursive collection**: Walks directories to find all files

#### Key Characteristics
- **Single item unit**: FileValidationInfo (one file to validate)
- **Batch unit**: All files in target path
- **Operations**: Metadata validation, hash verification
- **Atomic operation**: Single file validation

---

### 6. **ThumbnailJob**
**Location**: `core/src/ops/media/thumbnail/job.rs`
**Purpose**: Generate thumbnail images for media files

#### Configuration
- `entry_ids`: Optional (None = all suitable entries)
- `config`:
  - `variants`: Vec<ThumbnailVariantConfig> (multiple sizes/formats)
  - `regenerate`: Force regeneration
  - `batch_size`: Entries per batch (default 50)
  - `max_concurrent`: Concurrent generations (default 4)

#### Phases (3 total)
1. **Discovery** - Find entries needing thumbnails
   - Query database for entries with content
   - Check if sidecars already exist
   - Create batches from pending entries

2. **Processing** - Generate thumbnails in batches
   - Process each batch concurrently
   - For each variant: generate thumbnail, record sidecar
   - Emit ResourceChanged events after batch

3. **Cleanup** - Remove orphaned thumbnails (TODO)

#### Processing Patterns
- **Batch processing**: Entries grouped into batches
- **Per-variant generation**: Each entry generates multiple variants
- **Concurrent within batch**: Tasks spawned with join_all
- **Sidecar recording**: Each thumbnail recorded in database

#### Key Characteristics
- **Single item unit**: ThumbnailEntry (one entry to process)
- **Batch unit**: Vec<ThumbnailEntry> (batch_size entries)
- **Sub-unit**: Per variant (multiple generations per entry)
- **Operations**: Thumbnail generation, sidecar recording
- **Atomic operation**: Single variant generation for single entry

---

## Job Characteristics Summary

### Atomic Work Units by Job

| Job | Atomic Unit | Scale | Item Count |
|-----|-------------|-------|-----------|
| Indexer | DirEntry discovery + DB write | 1 entry | Hundreds of thousands |
| FileCopy | Source path copy (recursive) | 1-N files | Typically 1-10 sources |
| Delete | Path deletion (recursive) | 1-N files | Typically 1-100 targets |
| DuplicateDetection | Content hash generation | 1 file | Thousands |
| Validation | File validation | 1 file | Thousands |
| Thumbnail | Single variant generation | 1 variant per entry | Hundreds per entry |

### Phase Patterns

**Pattern 1: Linear Phases**
- Indexer: Discovery → Processing → Aggregation → ContentID → Complete
- ThumbnailJob: Discovery → Processing → Cleanup → Complete
- FileCopyJob: Init → DBQuery → Preparation → Copying → Complete

**Pattern 2: Mode-Driven Phases**
- Indexer skips phases based on mode (Shallow skips Content/Aggregation)
- Validation mode determines which checks run
- Duplicate detection mode determines grouping strategy

**Pattern 3: Batch Processing**
- Indexer: Batches entries during discovery
- ThumbnailJob: Chunks entries into configurable batches
- FileCopyJob: Processes sources sequentially with resume points

---

## Configuration Patterns

### By Scope
1. **Location-based**: Indexer, ThumbnailJob (work on library locations)
2. **Path-based**: FileCopyJob, DeleteJob, ValidationJob, DuplicateDetectionJob
3. **Entry-based**: ThumbnailJob (optional entry_ids filter)

### By Execution Model
1. **Sequential per item**: FileCopyJob (sources), DeleteJob (targets)
2. **Batch collection then processing**: Indexer, ThumbnailJob
3. **Analysis then grouping**: DuplicateDetectionJob, ValidationJob

### By Checkpointing Strategy
1. **Every N items**: Indexer (per batch), DuplicateDetectionJob (per 100), ValidationJob (per 50)
2. **Every N completed sources**: FileCopyJob (every 20 files)
3. **Every N batches**: ThumbnailJob (every 10 batches)

---

## Progress Reporting Patterns

### Generic Progress Structure
All jobs use either:
1. **Structured Progress** - Custom struct implementing JobProgress trait
2. **Generic Progress** - GenericProgress with percentage, phase, message, completion

### Metrics Tracked
- **Simple**: files_processed, total_files
- **Advanced**: bytes_processed, total_bytes, items_processed, duration
- **Detailed**: error_count, skip_count, warning_count

---

## Database Operations Patterns

### Read Patterns
- Indexer: Looks up parent directory IDs, existing entries for change detection
- ThumbnailJob: Queries entries with content, checks sidecar existence
- ValidationJob: Would verify against stored CAS IDs (placeholder in current code)

### Write Patterns
- Indexer: Creates/updates entries, generates content identities, calculates aggregations
- ThumbnailJob: Records sidecars (thumbnail metadata)
- DuplicateDetectionJob: Read-only (no writes)
- ValidationJob: Read-only (no writes)
- DeleteJob: Deletes entries (via deletion strategies)
- FileCopyJob: Updates file metadata if tracking completed copies

---

## Error Handling Patterns

### Critical vs Non-Critical
- **Critical**: Stops entire job (JobError::execution)
- **Non-Critical**: Accumulated in job context (add_non_critical_error)

### Per-Job Error Tracking
- **Indexer**: Tracks IndexError enum (ReadDir, CreateEntry, ContentId, FilterCheck)
- **FileCopyJob**: Tracks CopyError (source, destination, error message)
- **DeleteJob**: Tracks DeleteError (path, error message)
- **ValidationJob**: Tracks ValidationIssue (type, severity, suggested action)
- **DuplicateDetectionJob**: Logs errors, continues scanning
- **ThumbnailJob**: Stores error messages in state, continues processing

---

## Resumability Implementation

### State Serialization
- Msgpack binary format for compact state
- Full state saved on job pause/shutdown
- State restored on resume before next phase

### State Contents by Job
1. **Indexer**: Current phase, all discovery batches, pending entries, seen paths, DB cache
2. **FileCopyJob**: completed_indices to skip finished sources
3. **ThumbnailJob**: Current phase, pending/processed entries, stats
4. **DuplicateDetectionJob**: size_groups, processed_files
5. **ValidationJob**: validated_files list
6. **DeleteJob**: completed_deletions list

### Checkpoint Strategy
- **Automatic**: On interrupt/pause
- **Manual**: Periodic checkpoints to avoid re-doing work
- **Triggered by**: Phase completion, batch completion, item count threshold

---

## Volume Backend Integration

### Usage Patterns
- **Indexer**: 
  - Resolves volume for SdPath
  - Gets backend for directory discovery
  - Passes backend to phases for metadata extraction

- **Other jobs**: 
  - Optional volume_manager check
  - Fallback to local filesystem operations

### Volume Backend Methods Used
- `resolve_volume_for_sdpath()` - Find volume for path
- `backend_for_volume()` - Get backend implementation
- `same_volume()` - Check if paths on same volume (FileCopyJob)

---

## Key Design Insights for PhaseProcessor

1. **Phases are not always sequential** - Mode or config can skip phases
2. **Atomic units vary** - From single entry to batch to entire source path
3. **Batching is not universal** - Some jobs process items one-by-one with checkpoints
4. **Progress is heterogeneous** - Different granularity (items, bytes, variants, groups)
5. **State preservation is critical** - All intermediate data must serialize
6. **Error accumulation** - Non-critical errors continue, critical errors stop job
7. **Database integration** - Some phases query DB, others write, some are read-only
8. **Concurrency varies** - From sequential to batch-concurrent to per-variant concurrent
9. **Strategy pattern** - Copy and Delete use routers to select strategies
10. **Scope-driven behavior** - Same job (Indexer) behaves differently based on configuration

