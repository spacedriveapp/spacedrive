---
id: INDEX-002
title: Five-Phase Indexing Pipeline
status: Done
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, pipeline, phases]
whitepaper: Section 4.3.2
last_updated: 2025-12-16
---

## Description

Implement the multi-phase indexing pipeline that breaks filesystem discovery and processing into atomic, resumable stages. The ephemeral engine runs only Phase 1 (Discovery), while the persistent engine runs all five phases with full database writes and content analysis.

## Phase Architecture

### Phase 1: Discovery
**Used by: Ephemeral & Persistent**

Parallel filesystem walk optimized for raw speed:

- **Work-Stealing Parallelism**: Multiple threads scan concurrently, communicating via channels
- **Rules Engine Integration**: IndexerRuler filters at discovery edge (`.git`, `node_modules`, `.gitignore`)
- **Lightweight Output**: Stream of `DirEntry` objects
- **Progress Tracking**: Measured by directories discovered
- **Batching**: Collects 1,000 entries before moving to processing

**Implementation**: `core/src/ops/indexing/phases/discovery.rs`

### Phase 2: Processing
**Used by: Persistent Only**

Converts discovered entries into database records:

- **Topology Sorting**: Entries sorted by depth (parents before children)
- **Batch Transactions**: 1,000 items per transaction to minimize SQLite locking
- **Change Detection**: ChangeDetector compares filesystem vs database (New/Modified/Moved/Deleted)
- **UUID Preservation**: Carries over ephemeral UUIDs via `state.ephemeral_uuids`
- **Boundary Validation**: Ensures indexing path stays within location boundaries
- **Closure Table Updates**: Inserts ancestor-descendant pairs for hierarchy
- **Directory Path Cache**: Updates `directory_paths` table for O(1) lookups

**Implementation**: `core/src/ops/indexing/phases/processing.rs`

### Phase 3: Aggregation
**Used by: Persistent Only**

Bottom-up recursive statistics calculation:

- **Closure Table Queries**: O(1) descendant lookups
- **Leaf-to-Root Traversal**: Calculates sizes from deepest directories upward
- **Aggregates Stored**:
  - `aggregate_size` - Total bytes including subdirectories
  - `child_count` - Direct children only
  - `file_count` - Recursive file count

Enables instant "True Size" sorting without traversing descendants.

**Implementation**: `core/src/ops/indexing/phases/aggregation.rs`

### Phase 4: Content Identification
**Used by: Persistent Only**

Content addressable storage via BLAKE3 hashing:

- **BLAKE3 Hashing**: Generates content hashes for deduplication
- **Globally Deterministic UUIDs**: v5 UUIDs from content hash (offline duplicate detection)
- **Sync Ordering**: Content identities synced before entries (foreign key safety)
- **File Type Detection**: FileTypeRegistry populates `kind_id` and `mime_type_id`
- **Link to Content Records**: Entries reference shared `content_identity` table

**Implementation**: `core/src/ops/indexing/phases/content.rs`

### Phase 5: Finalizing
**Used by: Persistent Only**

Post-processing and processor dispatch:

- **Directory Aggregation Updates**: Final aggregate calculations
- **Processor Dispatch**: Triggers thumbnail generation for Deep Mode
- **Cleanup**: Marks indexing as complete

**Implementation**: Handled in `core/src/ops/indexing/job.rs`

## Implementation Files

### Phase Implementations
- `core/src/ops/indexing/phases/discovery.rs` - Phase 1
- `core/src/ops/indexing/phases/processing.rs` - Phase 2
- `core/src/ops/indexing/phases/aggregation.rs` - Phase 3
- `core/src/ops/indexing/phases/content.rs` - Phase 4
- `core/src/ops/indexing/phases/mod.rs` - Phase enum and orchestration

### Orchestration
- `core/src/ops/indexing/job.rs` - IndexerJob runs phases sequentially
- `core/src/ops/indexing/state.rs` - IndexerState tracks current phase and progress
- `core/src/ops/indexing/progress.rs` - Progress reporting per phase

## Acceptance Criteria

- [x] Phase 1 (Discovery) runs in both ephemeral and persistent modes
- [x] Phases 2-5 only run for persistent indexing
- [x] Each phase is resumable (state preserved in IndexerState)
- [x] Discovery uses work-stealing parallelism (8+ threads on capable systems)
- [x] Processing sorts entries by depth (parents before children)
- [x] Processing batches database writes (1,000 items/transaction)
- [x] ChangeDetector detects New/Modified/Moved/Deleted during processing
- [x] Aggregation uses closure table for O(1) descendant queries
- [x] Content phase generates BLAKE3 hashes
- [x] Content phase creates globally deterministic v5 UUIDs
- [x] FileTypeRegistry identifies file types during content phase
- [x] Progress tracking works for all phases
- [x] Job can pause/resume at any phase boundary
- [x] Ephemeral UUID preservation works in Phase 2

## Indexing Modes

The pipeline supports three depth modes:

| Mode | Phases Run | Speed | Use Case |
|------|-----------|-------|----------|
| Shallow | 1, 2, 3 | Fast | UI navigation, quick scan |
| Content | 1, 2, 3, 4 | Medium | Normal indexing with dedup |
| Deep | 1, 2, 3, 4, 5 | Slow | Media libraries with thumbnails |

## Indexing Scopes

| Scope | Behavior | Use Case |
|-------|----------|----------|
| Current | Index immediate directory only | Responsive UI navigation |
| Recursive | Index entire tree | Full location indexing |

## Performance Characteristics

| Configuration | Performance | Notes |
|--------------|-------------|-------|
| Current + Shallow | <500ms | No subdirectories |
| Recursive + Shallow | ~10K files/sec | Metadata only |
| Recursive + Content | ~1K files/sec | With BLAKE3 hashing |
| Recursive + Deep | ~100 files/sec | Full analysis + thumbnails |

## Resumability

Each phase stores sufficient state in `IndexerState` to resume:

```rust
pub struct IndexerState {
    pub phase: Phase,
    pub dirs_to_walk: VecDeque<PathBuf>,
    pub entry_batches: Vec<Vec<DirEntry>>,
    pub entry_id_cache: HashMap<PathBuf, i32>,
    pub ephemeral_uuids: HashMap<PathBuf, Uuid>,
    pub stats: IndexerStats,
}
```

When interrupted:
1. State serialized to jobs database (MessagePack)
2. On resume, job loads state and continues from saved phase
3. No work is lost

## Related Tasks

- INDEX-001 - Hybrid Architecture (defines ephemeral vs persistent)
- INDEX-003 - Database Architecture (closure tables used in Phase 3)
- INDEX-004 - Change Detection (ChangeDetector used in Phase 2)
- INDEX-005 - Indexer Rules (filters in Phase 1)
- JOB-000 - Job System (provides resumability infrastructure)
