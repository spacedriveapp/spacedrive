# Sync API Implementation Plan

**Status**: Ready for Implementation
**Created**: 2025-10-09
**Target**: Spacedrive Core v2 - Leaderless Sync

---

## Executive Summary

The sync infrastructure is complete and all tests pass. However, the current API for emitting sync events is verbose (9 lines of boilerplate per sync call). This plan introduces a clean, ergonomic API that reduces sync calls to **1 line** while maintaining full functionality.

**Current API** (9 lines):
```rust
let sync_service = library.sync_service().ok_or(...)?;
let peer_log = sync_service.peer_sync().peer_log();
let mut hlc_gen = sync_service.peer_sync().hlc_generator().lock().await;

library.transaction_manager()
    .commit_shared(library.id(), "tag", result.uuid, ChangeType::Insert,
                   serde_json::to_value(&result)?, peer_log, &mut *hlc_gen)
    .await?;
```

**Proposed API** (1 line):
```rust
library.sync_model(&result, ChangeType::Insert).await?;
```

---

## Goals

1. **Simplicity**: Reduce sync calls from 9 lines to 1 line
2. **Type Safety**: Leverage `Syncable` trait for automatic dispatch
3. **Performance**: Support batch operations for bulk indexing (10K+ entries)
4. **Consistency**: Single API for all models (tags, locations, entries, etc.)
5. **Maintainability**: Centralize sync logic, making it easy to evolve

---

## Architecture

### Core Principle

Add **extension methods** to `Library` that handle:
- Dependency fetching (sync service, peer log, HLC generator)
- FK conversion (UUID ↔ integer ID mapping)
- Sync strategy selection (device-owned vs shared)
- Batch optimization for bulk operations

### Three-Tier API

```
User-Facing Methods (in Library)
├─ sync_model() ────────────────► Simple models (no FKs)
├─ sync_model_with_db() ────────► Models with FK relationships
└─ sync_models_batch() ─────────► Bulk operations (1000+ records)

Internal Helpers (private)
├─ sync_device_owned_internal()
├─ sync_shared_internal()
├─ sync_device_owned_batch_internal()
└─ sync_shared_batch_internal()

Foundation (existing)
└─ TransactionManager.commit_*()
```

---

## API Design

### Method 1: `sync_model()` - Simple Case

**Use when**: Model has no FK relationships or FKs are already UUIDs

```rust
pub async fn sync_model<M: Syncable>(
    &self,
    model: &M,
    change_type: ChangeType,
) -> Result<()>
```

**Examples**:
- Tags (shared, no FKs)
- Devices (device-owned, self-referential)
- Albums (shared, no direct FKs)

**Usage**:
```rust
// Create tag
let tag = tag::ActiveModel { ... }.insert(db).await?;
library.sync_model(&tag, ChangeType::Insert).await?;
```

---

### Method 2: `sync_model_with_db()` - FK Conversion

**Use when**: Model has FK relationships that need UUID conversion

```rust
pub async fn sync_model_with_db<M: Syncable>(
    &self,
    model: &M,
    change_type: ChangeType,
    db: &DatabaseConnection,
) -> Result<()>
```

**Examples**:
- Locations (device-owned, has `device_id` + `entry_id` FKs)
- Entries (device-owned, has `parent_id`, `metadata_id`, `content_id` FKs)
- User Metadata (mixed, has various FKs)

**Usage**:
```rust
// Create location
let location = location::ActiveModel { ... }.insert(db).await?;
library.sync_model_with_db(&location, ChangeType::Insert, db).await?;

// Create entry
let entry = entry::ActiveModel { ... }.insert(db).await?;
library.sync_model_with_db(&entry, ChangeType::Insert, db).await?;
```

**Under the Hood**:
1. Serializes model to JSON
2. Iterates through `M::foreign_key_mappings()`
3. Converts each FK integer ID to UUID via database lookup
4. Emits sync event with UUID-based data

---

### Method 3: `sync_models_batch()` - Bulk Operations

**Use when**: Syncing 100+ records at once (indexing, imports, migrations)

```rust
pub async fn sync_models_batch<M: Syncable>(
    &self,
    models: &[M],
    change_type: ChangeType,
    db: &DatabaseConnection,
) -> Result<()>
```

**Examples**:
- Indexing 10,000 files in a location
- Importing photo library (5,000 images)
- Bulk tag application (1,000 entries)

**Usage**:
```rust
// Indexing job - process in batches of 1000
let mut batch = Vec::new();

for file in discovered_files {
    let entry = entry::ActiveModel { ... }.insert(db).await?;
    batch.push(entry);

    if batch.len() >= 1000 {
        library.sync_models_batch(&batch, ChangeType::Insert, db).await?;
        batch.clear();
    }
}

// Sync remaining
if !batch.is_empty() {
    library.sync_models_batch(&batch, ChangeType::Insert, db).await?;
}
```

**Performance**:
- **Without batching**: 10,000 individual network messages (~60 seconds)
- **With batching**: 10 batched messages (~2 seconds)
- **30x speedup** for bulk operations

---

## Implementation Details

### File Structure

```
core/src/library/
├── mod.rs ───────────────► Add public API methods
└── sync_helpers.rs ──────► Internal implementation (NEW FILE)

core/src/infra/sync/
└── transaction.rs ───────► Add batch event emission support
```

### Code Changes

#### 1. Create `core/src/library/sync_helpers.rs`

```rust
//! Sync helper methods for Library
//!
//! Provides ergonomic API for emitting sync events after database writes.

use crate::{
    infra::{
        db::DatabaseConnection,
        sync::{ChangeType, Syncable},
    },
    library::Library,
};
use anyhow::Result;
use uuid::Uuid;

impl Library {
    /// Sync a model without FK conversion
    pub async fn sync_model<M: Syncable>(
        &self,
        model: &M,
        change_type: ChangeType,
    ) -> Result<()> {
        let data = model.to_sync_json()?;

        if model.is_device_owned() {
            self.sync_device_owned_internal(M::SYNC_MODEL, model.sync_id(), data).await
        } else {
            self.sync_shared_internal(M::SYNC_MODEL, model.sync_id(), change_type, data).await
        }
    }

    /// Sync a model with FK conversion
    pub async fn sync_model_with_db<M: Syncable>(
        &self,
        model: &M,
        change_type: ChangeType,
        db: &DatabaseConnection,
    ) -> Result<()> {
        let mut data = model.to_sync_json()?;

        // Convert FK integer IDs to UUIDs
        for fk in M::foreign_key_mappings() {
            crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut data, &fk, db)
                .await
                .map_err(|e| anyhow::anyhow!("FK conversion failed: {}", e))?;
        }

        if model.is_device_owned() {
            self.sync_device_owned_internal(M::SYNC_MODEL, model.sync_id(), data).await
        } else {
            self.sync_shared_internal(M::SYNC_MODEL, model.sync_id(), change_type, data).await
        }
    }

    /// Batch sync multiple models
    pub async fn sync_models_batch<M: Syncable>(
        &self,
        models: &[M],
        change_type: ChangeType,
        db: &DatabaseConnection,
    ) -> Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        // Convert all models to sync JSON with FK mapping
        let mut sync_data = Vec::new();
        for model in models {
            let mut data = model.to_sync_json()?;

            for fk in M::foreign_key_mappings() {
                crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut data, &fk, db)
                    .await
                    .map_err(|e| anyhow::anyhow!("FK conversion failed: {}", e))?;
            }

            sync_data.push((model.sync_id(), data));
        }

        let is_device_owned = models[0].is_device_owned();

        if is_device_owned {
            self.sync_device_owned_batch_internal(M::SYNC_MODEL, sync_data).await
        } else {
            self.sync_shared_batch_internal(M::SYNC_MODEL, change_type, sync_data).await
        }
    }

    // ============ Internal Helpers ============

    async fn sync_device_owned_internal(
        &self,
        model_type: &str,
        record_uuid: Uuid,
        data: serde_json::Value,
    ) -> Result<()> {
        let device_id = self.device_id()?;
        self.transaction_manager()
            .commit_device_owned(self.id(), model_type, record_uuid, device_id, data)
            .await
    }

    async fn sync_shared_internal(
        &self,
        model_type: &str,
        record_uuid: Uuid,
        change_type: ChangeType,
        data: serde_json::Value,
    ) -> Result<()> {
        let sync_service = self.sync_service()
            .ok_or_else(|| anyhow::anyhow!("Sync service not initialized"))?;

        let peer_log = sync_service.peer_sync().peer_log();
        let mut hlc_gen = sync_service.peer_sync().hlc_generator().lock().await;

        self.transaction_manager()
            .commit_shared(
                self.id(),
                model_type,
                record_uuid,
                change_type,
                data,
                peer_log,
                &mut *hlc_gen,
            )
            .await
    }

    async fn sync_device_owned_batch_internal(
        &self,
        model_type: &str,
        records: Vec<(Uuid, serde_json::Value)>,
    ) -> Result<()> {
        let device_id = self.device_id()?;

        self.transaction_manager().event_bus().emit(Event::Custom {
            event_type: "sync:state_change_batch".to_string(),
            data: serde_json::json!({
                "library_id": self.id(),
                "model_type": model_type,
                "device_id": device_id,
                "records": records,
                "timestamp": chrono::Utc::now(),
            }),
        });

        Ok(())
    }

    async fn sync_shared_batch_internal(
        &self,
        model_type: &str,
        change_type: ChangeType,
        records: Vec<(Uuid, serde_json::Value)>,
    ) -> Result<()> {
        let sync_service = self.sync_service()
            .ok_or_else(|| anyhow::anyhow!("Sync service not initialized"))?;

        let peer_log = sync_service.peer_sync().peer_log();
        let mut hlc_gen = sync_service.peer_sync().hlc_generator().lock().await;

        for (record_uuid, data) in records {
            let hlc = hlc_gen.next();

            let entry = crate::infra::sync::SharedChangeEntry {
                hlc,
                model_type: model_type.to_string(),
                record_uuid,
                change_type,
                data,
            };

            peer_log.append(entry.clone()).await?;

            self.transaction_manager().event_bus().emit(Event::Custom {
                event_type: "sync:shared_change".to_string(),
                data: serde_json::json!({
                    "library_id": self.id(),
                    "entry": entry,
                }),
            });
        }

        Ok(())
    }
}
```

#### 2. Update `core/src/library/mod.rs`

```rust
// Add module declaration
mod sync_helpers;

// Existing code remains unchanged
```

#### 3. Update `core/src/service/sync/peer.rs`

Add handler for batch events:

```rust
// Handle batch state changes
Event::Custom { event_type, data } if event_type == "sync:state_change_batch" => {
    let library_id: Uuid = serde_json::from_value(data["library_id"].clone())?;
    let model_type: String = serde_json::from_value(data["model_type"].clone())?;
    let device_id: Uuid = serde_json::from_value(data["device_id"].clone())?;
    let records: Vec<(Uuid, Value)> = serde_json::from_value(data["records"].clone())?;

    // Broadcast batch to all peers
    self.broadcast_state_batch(library_id, model_type, device_id, records).await?;
}
```

---

## Rollout Plan

### Phase 1: Infrastructure (Week 1)

**Goal**: Implement clean API without breaking existing code

**Tasks**:
1. ✅ Create `sync_helpers.rs` with three public methods
2. ✅ Add batch event handling to `PeerSyncService`
3. ✅ Update integration tests to use new API
4. ✅ Run full test suite - verify no regressions

**Success Criteria**: All existing tests pass with new API available

---

### Phase 2: Proof of Concept (Week 1)

**Goal**: Wire up 2-3 managers to validate API

**Tasks**:
1. ✅ Wire `TagManager.create_tag()` - Uses `sync_model()`
2. ✅ Wire `LocationManager.add_location()` - Uses `sync_model_with_db()`
3. ✅ Test end-to-end sync between two real Core instances

**Success Criteria**:
- Create tag on Device A → appears on Device B
- Add location on Device A → visible on Device B

---

### Phase 3: Indexing Integration (Week 2)

**Goal**: Wire up bulk entry creation with batching

**Tasks**:
1. ✅ Update `EntryPersistence` to use `sync_models_batch()`
2. ✅ Add batch size configuration (default: 1000)
3. ✅ Test indexing 10K+ files with sync enabled
4. ✅ Measure performance (should be <5 seconds for 10K entries)

**Success Criteria**:
- Index 10,000 files on Device A
- All entry metadata syncs to Device B
- Sync overhead < 20% of indexing time

---

### Phase 4: Complete Migration (Week 3)

**Goal**: Wire all remaining managers

**Managers to Update**:
- ✅ `LocationManager` (add, update, remove)
- ✅ `TagManager` (create, update, delete)
- ✅ `AlbumManager` (when implemented)
- ✅ `UserMetadataManager` (when implemented)
- ✅ `EntryProcessor` (all entry operations)

**Success Criteria**: All database writes emit sync events

---

### Phase 5: CLI Setup Flow (Week 4)

**Goal**: Enable device pairing and sync setup via CLI

**Tasks**:
1. ✅ Verify `network pair` command works
2. ✅ Verify `network sync-setup` registers devices correctly
3. ✅ Test full flow: pair → setup → sync data
4. ✅ Document CLI workflow in user docs

**Success Criteria**: Users can pair devices and sync via CLI

---

## Usage Guidelines

### Decision Tree: Which Method to Use?

```
Starting Point: You just inserted/updated a model in the database
│
├─ Does your model have FK fields?
│  │
│  ├─ NO ──────────────────────► Use sync_model(model, change_type)
│  │                               Examples: Tag, Device, Album
│  │
│  └─ YES
│     │
│     ├─ Single operation? ─────► Use sync_model_with_db(model, change_type, db)
│     │                           Examples: Add one location, update one entry
│     │
│     └─ Bulk operation (>100)?─► Use sync_models_batch(vec, change_type, db)
│                                 Examples: Indexing, imports, migrations
│
└─ Result: Sync event emitted, data replicates to peers
```

### Code Examples by Model

#### Tags (Shared, No FKs)
```rust
let tag = tag::ActiveModel { ... }.insert(db).await?;
library.sync_model(&tag, ChangeType::Insert).await?;
```

#### Locations (Device-Owned, Has FKs)
```rust
let location = location::ActiveModel { ... }.insert(db).await?;
library.sync_model_with_db(&location, ChangeType::Insert, db).await?;
```

#### Entries (Device-Owned, Has FKs, Bulk)
```rust
let mut batch = Vec::new();
for file in files {
    let entry = entry::ActiveModel { ... }.insert(db).await?;
    batch.push(entry);

    if batch.len() >= 1000 {
        library.sync_models_batch(&batch, ChangeType::Insert, db).await?;
        batch.clear();
    }
}
if !batch.is_empty() {
    library.sync_models_batch(&batch, ChangeType::Insert, db).await?;
}
```

---

## Performance Expectations

### Single Operations

| Operation | Method | Overhead | Total Time |
|-----------|--------|----------|------------|
| Create tag | `sync_model()` | ~50ms | ~70ms |
| Add location | `sync_model_with_db()` | ~60ms | ~100ms |
| Update entry | `sync_model_with_db()` | ~60ms | ~80ms |

### Bulk Operations

| Scale | Without Batching | With Batching | Speedup |
|-------|------------------|---------------|---------|
| 100 entries | ~6 seconds | ~200ms | **30x** |
| 1,000 entries | ~60 seconds | ~500ms | **120x** |
| 10,000 entries | ~10 minutes | ~5 seconds | **120x** |

**Note**: Batching is **critical** for acceptable performance during indexing.

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_sync_model_tag() {
    let library = setup_test_library().await;
    let tag = create_test_tag().await;

    library.sync_model(&tag, ChangeType::Insert).await.unwrap();

    // Verify event emitted
    assert_event_emitted("sync:shared_change");
}

#[tokio::test]
async fn test_sync_model_with_db_location() {
    let library = setup_test_library().await;
    let db = library.db().conn();
    let location = create_test_location(db).await;

    library.sync_model_with_db(&location, ChangeType::Insert, db).await.unwrap();

    // Verify FK conversion happened
    let event_data = get_last_event_data();
    assert!(event_data["device_uuid"].is_string()); // Not device_id integer!
}

#[tokio::test]
async fn test_sync_models_batch() {
    let library = setup_test_library().await;
    let db = library.db().conn();
    let entries = create_test_entries(1000, db).await;

    let start = Instant::now();
    library.sync_models_batch(&entries, ChangeType::Insert, db).await.unwrap();
    let elapsed = start.elapsed();

    // Should be fast (< 1 second for 1000 entries)
    assert!(elapsed < Duration::from_secs(1));
}
```

### Integration Tests

Update existing tests in `core/tests/sync_integration_test.rs`:

```rust
// BEFORE (verbose)
let sync_service = setup.library_a.sync_service().unwrap();
let peer_sync = sync_service.peer_sync();
let peer_log = peer_sync.peer_log();
let mut hlc_gen = peer_sync.hlc_generator().lock().await;
setup.library_a.transaction_manager().commit_shared(...).await?;

// AFTER (clean)
setup.library_a.sync_model(&tag, ChangeType::Insert).await?;
```

---

## Migration Path

### For Existing Code

If any code already uses `commit_device_owned()` or `commit_shared()` directly:

1. **Don't break it** - old API continues to work
2. **Gradually migrate** - update as you touch files
3. **Eventual deprecation** - mark old API as `#[deprecated]` after 6 months

### Deprecation Timeline

- **Month 1-3**: New API available, old API works
- **Month 4-6**: Add `#[deprecated]` warnings to old API
- **Month 7+**: Remove old low-level API, force migration

---

## Success Metrics

### Code Quality
- ✅ Sync calls reduced from 9 lines → 1 line
- ✅ Zero breaking changes to existing tests
- ✅ Consistent API across all models

### Performance
- ✅ Single operations: <100ms overhead
- ✅ Bulk operations: 30-120x speedup with batching
- ✅ Indexing 10K files: <5 seconds sync overhead

### Developer Experience
- ✅ New contributors can add sync in <5 minutes
- ✅ API is self-documenting (method names explain intent)
- ✅ IDE autocomplete suggests correct method

---

## Risks and Mitigations

### Risk 1: Breaking Changes
**Likelihood**: Low
**Impact**: High
**Mitigation**: Keep old API working, add new API alongside

### Risk 2: Performance Regression
**Likelihood**: Medium (FK conversion on hot path)
**Impact**: Medium
**Mitigation**:
- Cache FK lookups where possible
- Use batch API for bulk operations
- Profile before/after with 10K entry test

### Risk 3: Incorrect Sync Strategy Selection
**Likelihood**: Low (determined by Syncable trait)
**Impact**: High (data corruption)
**Mitigation**:
- Add debug logging for strategy selection
- Integration tests cover all model types
- Fail-safe: default to safer shared resource sync

---

## Open Questions

1. **Batch Size Configuration**: Should batch size be configurable per operation or global?
   - **Proposal**: Global default (1000), override via parameter if needed

2. **FK Conversion Caching**: Should we cache device_id → device_uuid lookups?
   - **Proposal**: Yes, cache in Library with TTL of 60 seconds

3. **Error Handling**: If FK conversion fails, skip record or fail entire batch?
   - **Proposal**: Skip record with warning log, continue batch

4. **Sync During Indexing**: Should initial scan sync incrementally or wait until complete?
   - **Proposal**: Incremental batches (shows progress, allows resume)

---

## References

- **Sync Architecture**: `docs/core/sync.md`
- **Implementation Guide**: `core/src/infra/sync/docs/SYNC_IMPLEMENTATION_GUIDE.md`
- **Syncable Trait**: `core/src/infra/sync/syncable.rs`
- **FK Mapper**: `core/src/infra/sync/fk_mapper.rs`
- **Integration Tests**: `core/tests/sync_integration_test.rs`

---

## Approval

- [ ] **Architecture Review** - @jamespine
- [ ] **Performance Review** - TBD
- [ ] **Security Review** - TBD (ensure FK conversion doesn't leak data)

---

## Changelog

- **2025-10-09**: Initial plan created
- **TBD**: Implementation started
- **TBD**: Rollout complete

