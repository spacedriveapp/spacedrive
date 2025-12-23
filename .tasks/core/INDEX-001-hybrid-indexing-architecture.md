---
id: INDEX-001
title: Hybrid Indexing Architecture (Ephemeral + Persistent)
status: Done
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, architecture, ephemeral, persistent]
whitepaper: Section 4.3.1
last_updated: 2025-12-16
---

## Description

Implement the dual-layer indexing architecture that enables Spacedrive to act as both a fast file explorer (ephemeral mode) and a managed library system (persistent mode). This architecture allows instant browsing of unmanaged locations while seamlessly upgrading them to fully-indexed locations without UI disruption.

## Architecture

### Ephemeral Layer ("File Manager" Mode)

The ephemeral layer provides instant filesystem browsing without database writes:

- **Memory-Resident**: All data lives in RAM via `EphemeralIndex`
- **Highly Optimized**: NodeArena slab allocator + NameCache string interning (~50 bytes/entry)
- **Massive Scale**: Can index millions of files in memory
- **Zero Database I/O**: Bypasses SQLite entirely
- **Real-Time Updates**: Filesystem events update in-memory structures via `MemoryAdapter`

### Persistent Layer ("Library" Mode)

The persistent layer provides full database-backed indexing with sync and content analysis:

- **SQLite-Backed**: All entries stored in database with closure tables
- **Cross-Device Sync**: Changes propagate via library sync protocol
- **Content Analysis**: BLAKE3 hashing, file type detection, metadata extraction
- **Change Tracking**: Full history via sync log
- **Real-Time Updates**: Filesystem events update database via `DatabaseAdapter`

### Seamless State Promotion

The critical innovation is UUID preservation during ephemeral-to-persistent transitions:

1. User browses external drive in ephemeral mode (UUIDs assigned in RAM)
2. User adds location to library
3. System detects existing ephemeral index for that path
4. Indexer carries over ephemeral UUIDs into database (`state.ephemeral_uuids`)
5. UI remains stable (selections, active tabs, view state preserved)
6. Indexer proceeds from Phase 2 (Processing) onward

## Implementation Files

### Ephemeral Layer

- `core/src/ops/indexing/ephemeral/mod.rs` - Module definitions
- `core/src/ops/indexing/ephemeral/index.rs` - EphemeralIndex main structure
- `core/src/ops/indexing/ephemeral/cache.rs` - EphemeralIndexCache for tracking indexed paths
- `core/src/ops/indexing/ephemeral/arena.rs` - NodeArena slab allocator
- `core/src/ops/indexing/ephemeral/name.rs` - NameCache string interning
- `core/src/ops/indexing/ephemeral/registry.rs` - NameRegistry for name-based lookups
- `core/src/ops/indexing/ephemeral/writer.rs` - MemoryAdapter for writing to ephemeral index
- `core/src/ops/indexing/ephemeral/responder.rs` - Filesystem event handling
- `core/src/ops/indexing/ephemeral/types.rs` - FileNode and related types

### Persistent Layer

- `core/src/ops/indexing/database_storage.rs` - DatabaseStorage low-level CRUD operations
- `core/src/ops/indexing/persistence.rs` - DatabaseAdapter for IndexPersistence trait
- `core/src/ops/indexing/handlers/persistent.rs` - DatabaseAdapter for ChangeHandler trait

### Integration

- `core/src/ops/indexing/state.rs` - IndexerState with `ephemeral_uuids` field
- `core/src/ops/indexing/job.rs` - IndexerJob orchestration
- `core/src/ops/indexing/input.rs` - IndexerJobConfig with ephemeral/persistent modes

## Acceptance Criteria

- [x] EphemeralIndex can index directories entirely in RAM
- [x] NameCache interns duplicate filenames (e.g., "index.js" stored once)
- [x] NodeArena uses 32-bit entry IDs instead of 64-bit pointers
- [x] Memory usage is ~50 bytes per file entry
- [x] MemoryAdapter implements ChangeHandler for real-time ephemeral updates
- [x] DatabaseAdapter implements both IndexPersistence and ChangeHandler
- [x] Ephemeral-to-persistent promotion preserves UUIDs via IndexerState
- [x] UI doesn't flicker or reset state during promotion
- [x] EphemeralIndexCache tracks which paths are indexed/watching
- [x] Multiple directory trees can coexist in same EphemeralIndex
- [x] Filesystem events route to correct adapter (ephemeral vs persistent)

## Testing

### Manual Testing

```bash
# Test ephemeral browsing
spacedrive index browse /media/usb --ephemeral

# Verify in-memory only (no database writes)
spacedrive db query "SELECT COUNT(*) FROM entry WHERE name LIKE '%usb%'"
# Should return 0

# Add location while browsing (test promotion)
spacedrive location add /media/usb

# Verify UUIDs preserved (no UI flicker)
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_ephemeral_indexing` - Memory-only indexing
- `test_ephemeral_to_persistent_promotion` - UUID preservation
- `test_ephemeral_memory_usage` - Verify ~50 bytes/entry
- `test_ephemeral_string_interning` - NameCache deduplication

## Performance Characteristics

| Mode | Storage | Throughput | Memory/File | Sync | Survives Restart |
|------|---------|------------|-------------|------|------------------|
| Ephemeral | RAM | ~50K files/sec | ~50 bytes | No | No |
| Persistent | SQLite | ~10K files/sec | ~200 bytes | Yes | Yes |

## Related Tasks

- INDEX-002 - Five-Phase Indexing Pipeline
- INDEX-006 - Data Structures & Memory Optimizations
- INDEX-004 - Change Detection System (ChangeHandler trait)
