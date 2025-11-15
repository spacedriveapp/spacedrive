# Reactive Files Implementation Guide

**Status**: Foundation Complete, Ready for Implementation
**Created**: 2025-11-10

---

## What's Been Built

### 1. Generic Resource Mapper System ✅

**Location**: `core/src/domain/resource_manager.rs`

The `ResourceManager` handles virtual resource event mapping:

```rust
// Usage in indexing phases:
let resource_manager = ResourceManager::new(ctx.db());

// When ContentIdentity is created:
resource_manager.emit_resource_events(
    ctx,
    "content_identity",  // Source type
    vec![ci_id1, ci_id2, ci_id3]  // Changed IDs
).await?;

// Automatically:
// 1. Maps content_identity → file dependencies
// 2. Constructs File instances for affected entries
// 3. Emits ResourceChangedBatch { resource_type: "file", resources: [...] }
```

**Key Features**:
- Declarative dependency mapping in `resource.rs`
- Database queries to resolve virtual resource IDs
- Automatic batch event emission
- Extensible to future virtual resources

### 2. Resource Trait System ✅

**Location**: `core/src/domain/resource.rs`

```rust
pub trait Identifiable {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str;
    fn sync_dependencies() -> &'static [&'static str] { &[] }
}

// File declares its dependencies:
impl Identifiable for File {
    fn resource_type() -> &'static str { "file" }

    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar"]
    }
}
```

### 3. ResourceChangedBatch Event ✅

**Location**: `core/src/infra/event/mod.rs`

```rust
pub enum Event {
    // ... existing events ...

    ResourceChangedBatch {
        resource_type: String,
        resources: serde_json::Value,  // Array of resources
    },
}
```

### 4. Frontend Batch Event Handling ✅

**Location**: `packages/ts-client/src/hooks/useNormalizedCache.ts`

```typescript
// Handles both single and batch events:
if ("ResourceChanged" in event) {
    mergeResource(event.ResourceChanged.resource);
} else if ("ResourceChangedBatch" in event) {
    // Efficient batch processing
    event.ResourceChangedBatch.resources.forEach(resource =>
        mergeResource(resource)
    );
}
```

---

## What Needs to Be Implemented

### Phase 1: File Domain Updates

#### 1.1: Implement Identifiable for File

**File**: `core/src/domain/file.rs`

```rust
use crate::domain::resource::Identifiable;

impl Identifiable for File {
    fn id(&self) -> Uuid {
        self.id  // Uses Entry's UUID
    }

    fn resource_type() -> &'static str {
        "file"
    }

    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar"]
    }
}
```

#### 1.2: Add File::from_entry_uuids() Helper

**File**: `core/src/domain/file.rs`

Extract existing logic from `directory_listing.rs`:

```rust
impl File {
    /// Construct Files from Entry UUIDs (used for both queries and events)
    pub async fn from_entry_uuids(
        db: &DatabaseConnection,
        entry_uuids: &[Uuid],
    ) -> Result<Vec<Self>> {
        use crate::infra::db::entities::prelude::*;
        use sea_orm::*;

        // 1. Query entries with content_identity JOIN
        let entries_with_content = Entry::find()
            .filter(entity::entry::Column::Uuid.is_in(entry_uuids.iter().copied()))
            .find_also_related(ContentIdentity)
            .all(db)
            .await?;

        // 2. Batch fetch sidecars
        let content_uuids: Vec<Uuid> = entries_with_content
            .iter()
            .filter_map(|(_, ci)| ci.as_ref().map(|c| c.uuid))
            .collect();

        let sidecars = Sidecar::find()
            .filter(entity::sidecar::Column::ContentUuid.is_in(content_uuids))
            .all(db)
            .await?;

        let sidecar_map: HashMap<Uuid, Vec<Sidecar>> = sidecars
            .into_iter()
            .fold(HashMap::new(), |mut map, s| {
                map.entry(s.content_uuid).or_default().push(s.into());
                map
            });

        // 3. Construct File objects
        let files = entries_with_content
            .into_iter()
            .map(|(entry, content_identity)| {
                let mut file = Self::from_entity_model(entry);
                file.content_identity = content_identity.map(Into::into);

                if let Some(ref ci) = file.content_identity {
                    file.sidecars = sidecar_map
                        .get(&ci.uuid)
                        .cloned()
                        .unwrap_or_default();
                }

                file
            })
            .collect();

        Ok(files)
    }
}
```

### Phase 2: Indexing Phase 2 (Processing) Events

**File**: `core/src/ops/indexing/phases/processing.rs`

Add event emission after Entry creation:

```rust
pub async fn process_batch(
    ctx: &Context,
    batch: Vec<DirEntry>,
) -> Result<()> {
    // ... existing Entry creation logic ...

    // Sync to Transaction Manager
    sync_entries_to_tm(ctx, &created_entries).await?;

    // NEW: Emit events for entries with UUIDs (directories + empty files)
    let entries_with_uuid: Vec<Uuid> = created_entries
        .iter()
        .filter_map(|e| e.uuid)
        .collect();

    if !entries_with_uuid.is_empty() {
        emit_file_events(ctx, &entries_with_uuid).await?;
    }

    Ok(())
}

async fn emit_file_events(ctx: &Context, entry_uuids: &[Uuid]) -> Result<()> {
    use crate::domain::{File, ResourceManager};

    let files = File::from_entry_uuids(ctx.db(), entry_uuids).await?;

    if !files.is_empty() {
        tracing::info!("Emitting {} File ResourceChanged events", files.len());

        ctx.events.emit(Event::ResourceChangedBatch {
            resource_type: "file".to_string(),
            resources: serde_json::to_value(&files)?,
        });
    }

    Ok(())
}
```

### Phase 3: Indexing Phase 4 (Content) Events

**File**: `core/src/ops/indexing/phases/content.rs`

Add event emission after content identity linking:

```rust
pub async fn process_content_batch(
    ctx: &Context,
    batch: Vec<(i32, PathBuf)>,  // (entry_id, path)
) -> Result<()> {
    use crate::domain::ResourceManager;

    let mut link_results = Vec::new();
    let mut content_identity_ids = Vec::new();

    for (entry_id, path) in batch {
        let content_hash = hash_file(&path).await?;

        let result = link_to_content_identity(
            ctx,
            entry_id,
            content_hash,
        ).await?;

        link_results.push(result.clone());

        // Track newly created ContentIdentities
        if result.was_created {
            content_identity_ids.push(result.content_identity_id);
        }
    }

    // Sync models to Transaction Manager
    sync_models_to_tm(ctx, &link_results).await?;

    // NEW: Emit File events via ResourceManager
    // This will automatically map ContentIdentity → File
    let resource_manager = ResourceManager::new(ctx.db().clone());
    resource_manager.emit_batch_resource_events(
        ctx,
        "content_identity",
        content_identity_ids,
    ).await?;

    Ok(())
}
```

### Phase 4: Thumbnail Phase Events

**File**: `core/src/ops/media/thumbnail/action.rs`

Emit File event when Sidecar is created:

```rust
pub async fn save_thumbnail(
    ctx: &Context,
    sidecar: Sidecar,
) -> Result<()> {
    use crate::domain::ResourceManager;

    // Insert sidecar
    let inserted = entity::sidecar::ActiveModel::from(sidecar.clone())
        .insert(ctx.db())
        .await?;

    // Emit File event via ResourceManager
    let resource_manager = ResourceManager::new(ctx.db().clone());
    resource_manager.emit_resource_events(
        ctx,
        "sidecar",
        vec![inserted.uuid],
    ).await?;

    Ok(())
}
```

### Phase 5: Frontend Integration

**File**: `packages/interface/src/Explorer.tsx` (or your file list component)

```typescript
import { useNormalizedCache } from '@sd/ts-client';

function FileList({ locationId, path }: Props) {
    // Use normalized cache for reactive updates
    const filesQuery = useNormalizedCache<DirectoryListingInput, DirectoryListingOutput>({
        wireMethod: "query:files.directory_listing",
        input: { location_id: locationId, path },
        resourceType: "file",  // ← Must match Rust File::resource_type()
    });

    const files = filesQuery.data?.entries || [];

    return (
        <div>
            {files.map(file => (
                <FileCard
                    key={file.id}
                    file={file}
                    // As indexing progresses, file updates automatically:
                    // 1. File appears (no content_identity)
                    // 2. content_identity populated
                    // 3. Thumbnail appears (sidecar added)
                />
            ))}
        </div>
    );
}
```

---

## Key Design Decisions

### 1. Emit Events for ALL Files ✅

**Decision**: Emit ResourceChangedBatch events for all indexed files, not just visible ones.

**Rationale**:
- Simpler implementation (no path filtering logic)
- Normalized cache ignores events for files not in cache (automatic filtering!)
- User only pays for what they see

### 2. Only Cached Pages Update ✅

**Decision**: Don't invalidate entire query, only update resources already in cache.

**Rationale**:
- TanStack Query's `setQueryData` only updates if data exists
- If file not in current page, event is ignored
- User navigates to new page → normal query fetches fresh data
- Keeps memory usage low

### 3. Generic Resource Mapper ✅

**Decision**: Build ResourceManager system now, not later.

**Rationale**:
- Future-proofs for other virtual resources (Album, Smart Collection, etc.)
- Cleaner separation of concerns (indexing code doesn't know about File)
- Easier to test (mock ResourceManager)
- Will be extremely useful going forward

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_file_from_entry_uuids() {
    let db = setup_test_db().await;

    // Create test entries with content identities
    let entry_uuids = create_test_entries(&db, 100).await;

    // Construct Files
    let files = File::from_entry_uuids(&db, &entry_uuids).await.unwrap();

    assert_eq!(files.len(), 100);
    assert!(files.iter().all(|f| f.id != Uuid::nil()));
}

#[tokio::test]
async fn test_resource_mapper_content_identity_to_file() {
    let ctx = setup_test_context().await;
    let resource_manager = ResourceManager::new(ctx.db());

    // Create ContentIdentity
    let content_identity = create_test_content_identity(&ctx).await;

    // Link Entry to ContentIdentity
    let entry = create_test_entry(&ctx, Some(content_identity.id)).await;

    // Emit event via ResourceManager
    resource_manager.emit_resource_events(
        &ctx,
        "content_identity",
        vec![content_identity.uuid],
    ).await.unwrap();

    // Verify File event was emitted
    let events = ctx.events.collect().await;
    assert!(events.iter().any(|e| matches!(e, Event::ResourceChangedBatch { resource_type, .. } if resource_type == "file")));
}
```

### Integration Tests

```typescript
describe('Reactive File Updates', () => {
    test('files appear during indexing', async () => {
        const { result } = renderHook(() =>
            useNormalizedCache({
                wireMethod: 'query:files.directory_listing',
                input: { location_id: testLocationId, path: '/' },
                resourceType: 'file',
            })
        );

        // Initial state: empty
        expect(result.current.data?.entries).toHaveLength(0);

        // Trigger indexing
        await indexLocation(testLocationId);

        // Wait for Phase 2 events (directories)
        await waitFor(() => {
            const dirs = result.current.data?.entries.filter(f => f.kind === 'directory');
            expect(dirs.length).toBeGreaterThan(0);
        });

        // Wait for Phase 4 events (content identities)
        await waitFor(() => {
            const filesWithContent = result.current.data?.entries.filter(
                f => f.content_identity !== null
            );
            expect(filesWithContent.length).toBeGreaterThan(0);
        });
    });

    test('only visible files update cache', async () => {
        // Subscribe to /folder1
        const { result: folder1 } = renderHook(() =>
            useNormalizedCache({
                wireMethod: 'query:files.directory_listing',
                input: { location_id: testLocationId, path: '/folder1' },
                resourceType: 'file',
            })
        );

        // Index /folder2 (different path)
        await indexDirectory('/folder2');

        // folder1 cache should NOT update (events ignored)
        expect(folder1.current.data?.entries).toHaveLength(0);
    });
});
```

---

## Performance Considerations

### Batch Sizes

**Recommended**:
- Processing phase: 500 files/batch (directories are lightweight)
- Content phase: 100 files/batch (matches hashing parallelism)
- Thumbnail phase: 10 files/batch (thumbnails are slow)

### Event Throttling

Optional optimization for huge directories (10,000+ files):

```rust
struct EventBuffer {
    pending_files: Vec<File>,
    last_flush: Instant,
    max_buffer_size: usize,
    max_buffer_time: Duration,
}

impl EventBuffer {
    fn buffer_file(&mut self, file: File) {
        self.pending_files.push(file);

        if self.should_flush() {
            self.flush();
        }
    }

    fn should_flush(&self) -> bool {
        self.pending_files.len() >= self.max_buffer_size
            || self.last_flush.elapsed() > self.max_buffer_time
    }

    fn flush(&mut self) {
        if self.pending_files.is_empty() {
            return;
        }

        // Emit batch event
        self.events.emit(Event::ResourceChangedBatch {
            resource_type: "file".to_string(),
            resources: serde_json::to_value(&self.pending_files).unwrap(),
        });

        self.pending_files.clear();
        self.last_flush = Instant::now();
    }
}
```

### Memory Usage

**Scenario**: Indexing 10,000 files

**Without events**: ~5 MB (Entry records)
**With events**: ~7 MB (Entry records + event buffer)

**Acceptable trade-off** for instant UI updates.

---

## Migration Timeline

### Week 1: Foundation
- [x] Create `resource.rs` trait system
- [x] Create `resource_manager.rs` mapping system
- [x] Add `ResourceChangedBatch` event variant
- [x] Update frontend `useNormalizedCache` for batch events

### Week 2: File Domain
- [ ] Implement `Identifiable` for `File`
- [ ] Extract `File::from_entry_uuids()` helper
- [ ] Write unit tests for File construction
- [ ] Test ResourceManager with mock data

### Week 3: Processing Phase
- [ ] Add event emission in `processing.rs`
- [ ] Test with simple directory (10-100 files)
- [ ] Verify directories appear in UI instantly
- [ ] Verify no events for regular files (UUIDs not assigned yet)

### Week 4: Content Phase
- [ ] Add event emission in `content.rs`
- [ ] Test with regular files (1000+ files)
- [ ] Verify files gain content identities in UI
- [ ] Profile memory usage

### Week 5: Thumbnail Phase
- [ ] Add event emission in `thumbnail/action.rs`
- [ ] Test thumbnail generation flow
- [ ] Verify thumbnails appear progressively
- [ ] End-to-end test: index → content → thumbnails

---

## Success Metrics

### Functional
- Files appear in UI during Processing phase (directories)
- Files update with content identities during Content phase
- Thumbnails appear progressively
- No duplicate events
- Events only update visible/cached files

### Performance
- Event emission adds less than 100ms to indexing per 1000 files
- Frontend processes batch events in less than 10ms
- Memory usage increase less than 50% during indexing
- No UI jank (batching prevents main thread blocking)

### Developer Experience
- Future virtual resources use same pattern (just implement Identifiable)
- ResourceManager handles complexity (indexing code stays simple)
- Clear separation of concerns (domain vs infrastructure)

---

## Future Enhancements

### Phase 6: Optimistic Updates

```typescript
const createFile = useCoreMutation('files.create');

await createFile.mutateAsync(
    { name: 'new.txt', parent_id: currentDir },
    {
        onMutate: (variables) => {
            // Optimistic: Show file immediately
            const tempFile = { id: tempId, ...variables };
            queryClient.setQueryData(queryKey, old => [...old, tempFile]);
        },
        onSuccess: (realFile) => {
            // Replace temp with real (event will also arrive)
            queryClient.setQueryData(queryKey, old =>
                old.map(f => f.id === tempId ? realFile : f)
            );
        },
    }
);
```

### Phase 7: Event Compression

For massive directories (100,000+ files):

```rust
// Instead of: ResourceChangedBatch { resources: [file1, file2, ...file100000] }
// Emit: ResourceBulkOperation { operation: "indexing_complete", affected_count: 100000 }
// Frontend: Invalidate query instead of merging 100,000 resources
```

### Phase 8: Virtual Resource Registry

```rust
// Auto-register virtual resources at compile time
#[derive(Identifiable)]
#[resource_type = "file"]
#[depends_on = ["entry", "content_identity", "sidecar"]]
pub struct File { ... }

// ResourceManager discovers dependencies via registry
```

---

## Summary

**Foundation Complete**: Resource mapping system is built and ready to use.

**Next Steps**:
1. Implement `Identifiable` for `File`
2. Extract `File::from_entry_uuids()` helper
3. Add event emission in Processing phase
4. Test with simple directory
5. Expand to Content and Thumbnail phases

**Key Insight**: Generic ResourceManager makes this pattern reusable for all future virtual resources. Investment pays off immediately and long-term.

**Timeline**: 5 weeks to full reactive file system with progressive enhancement during indexing.

**Risk**: Low - proven pattern (Location), well-tested foundation, clear separation of concerns.
