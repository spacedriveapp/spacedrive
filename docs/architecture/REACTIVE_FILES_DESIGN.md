# Reactive Files: Virtual Resource Event Mapping

**Status**: Design Phase
**Goal**: Enable real-time UI updates as files progress through indexing phases
**Complexity**: High - requires virtual resource event mapping

---

## Problem Statement

### Current State
- Locations have normalized cache and emit `ResourceChanged` events
- Files do NOT emit events during indexing
- File is a **virtual/computed resource** (Entry + ContentIdentity + Sidecar)
- Frontend receives stale data until manual refetch

### Desired State
Users should see files appear and update in real-time during indexing:
1. **Phase 2 (Processing)**: Directories appear immediately with no content identity
2. **Phase 4 (Content ID)**: Files gain content identities, UI shows hash/dedup info
3. **Phase 5 (Thumbnails)**: Thumbnails appear as generated

---

## Core Challenge: Virtual Resource Mapping

### The Virtual Resource Problem

**File is NOT a database table:**
```rust
// File is constructed from 3+ sources:
struct File {
    // From Entry table:
    id: Uuid,          // entry.uuid
    name: String,      // entry.name
    size: u64,         // entry.size

    // From ContentIdentity table (via FK):
    content_identity: Option<ContentIdentity>,  // JOIN

    // From Sidecar table:
    sidecars: Vec<Sidecar>,  // GROUP BY content_uuid

    // Computed:
    alternate_paths: Vec<SdPath>,  // Other entries with same content_id
}
```

**When ContentIdentity is created, what event do we emit?**
- `ResourceChanged { resource_type: "content_identity" }` - Frontend doesn't know about this
- `ResourceChanged { resource_type: "file" }` - Frontend expects File structs
- **How do we map ContentIdentity changes to File events?**

---

## Research Findings

### 1. UUID Assignment Timeline

**Critical Discovery**: Entry UUIDs are assigned at different times:

| File Type | UUID Assignment Phase | Content Identity Phase |
|-----------|----------------------|------------------------|
| **Directories** | Processing (Phase 2) | N/A (no content) |
| **Empty files (0 bytes)** | Processing (Phase 2) | N/A (no content) |
| **Regular files** | Content ID (Phase 4) | ContentIdentity created |

**Implication**: We can't emit File events for regular files until Phase 4 completes!

### 2. Indexing Phase Flow

```
Phase 1: Discovery
├─ Walk filesystem (1000 files per batch)
└─ Store in memory: Vec<DirEntry>

Phase 2: Processing
├─ Sort by depth (parents first)
├─ Insert Entry records (transaction per batch)
├─ Assign UUIDs:
│  ├─ Directories: Uuid::new_v4() ✅
│  ├─ Empty files: Uuid::new_v4() ✅
│  └─ Regular files: None (deferred) ⏳
├─ Sync to Transaction Manager (dirs + empty files)
└─ Track files needing content: entries_for_content

Phase 3: Aggregation
├─ Calculate directory sizes
└─ No events needed

Phase 4: Content Identification
├─ Parallel hash generation (100 files per batch)
├─ For each file:
│  ├─ Find/create ContentIdentity (deterministic UUID v5)
│  ├─ Update entry.content_id FK
│  ├─ Assign entry.uuid = Uuid::new_v4() ✅
│  └─ Sync both ContentIdentity + Entry models
└─ This is where we need File events!

Phase 5: Thumbnails (separate job)
├─ Generate thumbnails asynchronously
├─ Create Sidecar records
└─ Emit events for thumbnail completion
```

### 3. Current Event Emission

**Only Location emits ResourceChanged:**
```rust
// core/src/location/manager.rs:234
events.emit(Event::ResourceChanged {
    resource_type: "location".to_string(),
    resource: serde_json::to_value(&location_info).unwrap(),
});
```

**No File-level events exist** during indexing phases.

### 4. Batching Strategy

**Processing Phase** (from `phases/processing.rs`):
- Batch size: 1000 entries
- Single transaction per batch
- Could emit 1000 File events per batch

**Content Phase** (from `phases/content.rs`):
- Batch size: 100 files (parallel hashing)
- Individual updates per file (no explicit transaction wrapper)
- Syncs models in batches of 100

**Performance Concern**: 10,000 files = 100+ events. Need batching!

---

## Proposed Solution: Resource Dependency Mapping

### Architecture Overview

```
Backend (Rust)                          Frontend (TypeScript)
────────────────────────────────────    ────────────────────────────

1. Database Update
   Entry.content_id = 42 ─────┐
                               │
2. Event System                │
   ResourceChanged {           │
     resource_type: "file",    │
     resource_id: entry_uuid,  │
     changed_fields: ["content_identity"]
   } ─────────────────────────┼─────→ 3. Normalized Cache
                               │           ├─ Resource matches "file"?
                               │           ├─ Find File in cache by ID
                               │           └─ Refetch File from backend
                               │
4. Resource Mapper             │
   ├─ Detects: ContentIdentity│created
   ├─ Looks up: Which Files?  │       4. Query: files.directory_listing
   ├─ Finds: Entry with       │          ├─ Constructs File from:
   │   content_id = 42        │          │  Entry + ContentIdentity + Sidecar
   └─ Emits: File event       │          └─ Returns: Updated File struct
                               │
                               └─────→ 5. UI Re-renders
                                          File now shows content hash!
```

### Implementation Strategy

#### Option A: Eager File Construction (Recommended)

**Emit fully-constructed File structs in events:**

```rust
// core/src/ops/indexing/phases/content.rs

async fn complete_batch(
    ctx: &Context,
    batch: Vec<ContentLinkResult>,
) -> Result<()> {
    // Sync models (existing code)
    sync_models_to_transaction_manager(ctx, &batch).await?;

    // NEW: Construct File objects and emit events
    let files = construct_files_for_batch(ctx, &batch).await?;

    if !files.is_empty() {
        ctx.events.emit(Event::ResourceChanged {
            resource_type: "file".to_string(),
            resources: serde_json::to_value(&files).unwrap(), // Batch!
        });
    }

    Ok(())
}

async fn construct_files_for_batch(
    ctx: &Context,
    batch: &[ContentLinkResult],
) -> Result<Vec<File>> {
    let entry_ids: Vec<i32> = batch.iter().map(|r| r.entry_id).collect();

    // Reuse directory_listing logic - same SQL query!
    let entries = query_entries_with_content_identity(ctx.db(), &entry_ids).await?;
    let sidecars = fetch_sidecars_for_entries(ctx.db(), &entries).await?;

    let files = entries.into_iter()
        .map(|entry| File::from_entity_model(entry, sidecars))
        .collect();

    Ok(files)
}
```

**Pros**:
- Frontend receives complete File structs (matches directory_listing output)
- No additional mapping logic needed on frontend
- Normalized cache works out-of-the-box (already expects File structs)
- Reuses existing SQL join logic from directory_listing

**Cons**:
- ️ Backend does extra work (constructs Files that may not be visible)
- ️ Event payload larger (~500-1000 bytes per File vs ~100 bytes for just ID)

#### Option B: Lazy Mapping (Complex)

**Emit Entry/ContentIdentity changes, map to File IDs later:**

```rust
// New: Resource dependency registry
struct ResourceMapper {
    // Maps "content_identity" → ["file"]
    dependencies: HashMap<&'static str, Vec<&'static str>>,
}

impl ResourceMapper {
    fn register_dependency(&mut self, source: &'static str, dependent: &'static str) {
        self.dependencies.entry(source).or_default().push(dependent);
    }

    async fn emit_dependent_events(
        &self,
        ctx: &Context,
        source_type: &str,
        source_id: Uuid,
    ) -> Result<()> {
        if let Some(dependents) = self.dependencies.get(source_type) {
            for dependent_type in dependents {
                match *dependent_type {
                    "file" => {
                        // Look up which Entry has this content_id
                        let entry = query_entry_by_content_identity(ctx.db(), source_id).await?;

                        // Emit file change event
                        ctx.events.emit(Event::ResourceChanged {
                            resource_type: "file".to_string(),
                            resource_id: entry.uuid,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
```

**Pros**:
- Minimal event payload (just IDs)
- Generic system for all virtual resources

**Cons**:
- Complex: Requires new ResourceMapper system
- Frontend must refetch File after receiving event (extra round-trip)
- Doesn't leverage normalized cache's `setQueryData` optimization
- More database queries (look up Entry by content_id, then fetch full File)

---

## Recommended Approach: Option A (Eager File Construction)

### Why Option A Wins

1. **Normalized cache expects full resources:**
   ```typescript
   // useNormalizedCache.ts:103
   queryClient.setQueryData<O>(queryKey, (oldData) => {
       // Merges resource directly - expects File struct!
       const existingIndex = oldData.findIndex(item => item.id === resource.id);
       newData[existingIndex] = resource;  // ← Needs complete File
   });
   ```

2. **Already have the logic:**
   - `directory_listing.rs` already constructs Files efficiently
   - Just extract to reusable function: `construct_files_from_entry_ids()`

3. **Performance is acceptable:**
   - Event size: 100 files × ~800 bytes = 80 KB per batch
   - Network overhead: negligible on local Unix socket
   - Trade-off: Slightly larger events vs simpler architecture

4. **Matches Location pattern:**
   - Location emits full `LocationInfo` structs (not just IDs)
   - Keeps mental model consistent

### Implementation Plan

#### Step 1: Extract File Construction Logic

```rust
// core/src/domain/file.rs

impl File {
    /// Construct Files from Entry IDs (reusable for both queries and events)
    pub async fn from_entry_ids(
        db: &DatabaseConnection,
        entry_ids: &[i32],
    ) -> Result<Vec<Self>> {
        // 1. Query entries with content_identity JOIN
        let entries = entity::prelude::Entry::find()
            .filter(entity::entry::Column::Id.is_in(entry_ids.iter().copied()))
            .find_with_related(entity::prelude::ContentIdentity)
            .all(db)
            .await?;

        // 2. Fetch sidecars (batch)
        let content_uuids: Vec<Uuid> = entries.iter()
            .filter_map(|(e, ci)| ci.as_ref().map(|c| c.uuid))
            .collect();

        let sidecars = entity::prelude::Sidecar::find()
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
        let files = entries.into_iter()
            .map(|(entry, content_identity)| {
                let mut file = Self::from_entity_model(entry);
                file.content_identity = content_identity.map(Into::into);
                if let Some(ref ci) = file.content_identity {
                    file.sidecars = sidecar_map.get(&ci.uuid).cloned().unwrap_or_default();
                }
                file
            })
            .collect();

        Ok(files)
    }
}
```

#### Step 2: Emit Events in Processing Phase (Directories)

```rust
// core/src/ops/indexing/phases/processing.rs

async fn process_batch(
    ctx: &Context,
    batch: Vec<DirEntry>,
) -> Result<()> {
    // ... existing Entry creation logic ...

    // Sync to Transaction Manager
    sync_entries_to_tm(ctx, &created_entries).await?;

    // NEW: Emit events for entries with UUIDs (directories + empty files)
    let entries_with_uuid: Vec<i32> = created_entries.iter()
        .filter(|e| e.uuid.is_some())
        .map(|e| e.id)
        .collect();

    if !entries_with_uuid.is_empty() {
        emit_file_events(ctx, &entries_with_uuid).await?;
    }

    Ok(())
}

async fn emit_file_events(ctx: &Context, entry_ids: &[i32]) -> Result<()> {
    let files = File::from_entry_ids(ctx.db(), entry_ids).await?;

    if !files.is_empty() {
        info!("Emitting {} File ResourceChanged events", files.len());

        ctx.events.emit(Event::ResourceChanged {
            resource_type: "file".to_string(),
            resources: serde_json::to_value(&files).unwrap(),
        });
    }

    Ok(())
}
```

#### Step 3: Emit Events in Content Phase (Regular Files)

```rust
// core/src/ops/indexing/phases/content.rs

async fn process_content_batch(
    ctx: &Context,
    batch: Vec<PathBuf>,
) -> Result<()> {
    // ... existing content hashing logic ...

    let mut link_results = Vec::new();

    for path in batch {
        let content_hash = hash_file(&path).await?;
        let result = link_to_content_identity(ctx, entry_id, content_hash).await?;
        link_results.push(result);
    }

    // Sync models
    sync_models_to_tm(ctx, &link_results).await?;

    // NEW: Emit File events for entries that just got content IDs + UUIDs
    let entry_ids: Vec<i32> = link_results.iter().map(|r| r.entry_id).collect();
    emit_file_events(ctx, &entry_ids).await?;

    Ok(())
}
```

#### Step 4: Implement Identifiable for File

```rust
// core/src/domain/file.rs

use crate::domain::resource::Identifiable;

impl Identifiable for File {
    fn id(&self) -> Uuid {
        self.id  // Uses Entry's UUID
    }

    fn resource_type() -> &'static str {
        "file"
    }

    // Optional: Declare dependencies for future virtual resource system
    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar"]
    }
}
```

#### Step 5: Frontend - Use Normalized Cache

```typescript
// packages/interface/src/Explorer.tsx

function FileList({ locationId, path }: Props) {
    const filesQuery = useNormalizedCache<DirectoryListingInput, DirectoryListingOutput>({
        wireMethod: "query:files.directory_listing",
        input: { location_id: locationId, path },
        resourceType: "file",  // ← Matches Rust!
    });

    const files = filesQuery.data?.entries || [];

    return (
        <div>
            {files.map(file => (
                <FileCard
                    key={file.id}
                    file={file}
                    // As indexing progresses:
                    // 1. File appears (no content_identity)
                    // 2. File updates (content_identity present)
                    // 3. Thumbnail appears (sidecar added)
                />
            ))}
        </div>
    );
}
```

#### Step 6: Update Event Type for Batch Support

```rust
// core/src/infra/event/mod.rs

pub enum Event {
    // ... existing variants ...

    // OPTION 1: Keep singular (emit multiple times per batch)
    ResourceChanged {
        resource_type: String,
        resource: serde_json::Value,
    },

    // OPTION 2: Add batch variant (better performance)
    ResourceChangedBatch {
        resource_type: String,
        resources: Vec<serde_json::Value>,  // Array of resources
    },
}
```

**Recommendation**: Use OPTION 2 (batch variant) to reduce event overhead.

#### Step 7: Frontend - Handle Batch Events

```typescript
// packages/ts-client/src/hooks/useNormalizedCache.ts

useEffect(() => {
    const handleEvent = (event: any) => {
        // Existing: Single resource
        if ("ResourceChanged" in event) {
            const { resource_type, resource } = event.ResourceChanged;
            if (resource_type === resourceType) {
                mergeResource(resource);
            }
        }

        // NEW: Batch resources
        if ("ResourceChangedBatch" in event) {
            const { resource_type, resources } = event.ResourceChangedBatch;
            if (resource_type === resourceType) {
                resources.forEach(resource => mergeResource(resource));
            }
        }
    };

    function mergeResource(resource: any) {
        queryClient.setQueryData<O>(queryKey, (oldData) => {
            if (!oldData) return oldData;

            if (Array.isArray(oldData)) {
                const existingIndex = oldData.findIndex((item: any) => item.id === resource.id);
                if (existingIndex >= 0) {
                    const newData = [...oldData];
                    newData[existingIndex] = resource;
                    return newData as O;
                } else {
                    return [...oldData, resource] as O;
                }
            }
            // ... wrapped object handling ...
        });
    }

    client.on("spacedrive-event", handleEvent);
    return () => client.off("spacedrive-event", handleEvent);
}, [resourceType, queryKey]);
```

---

## Performance Considerations

### Batch Size Tuning

**Current indexing batches**:
- Processing: 1000 entries/batch
- Content: 100 files/batch

**Event batch sizes** (recommended):
- Processing phase: 500 files/event (directories are lightweight)
- Content phase: 100 files/event (matches processing batch)

**Why smaller batches?**
- Frontend processes events on main thread
- 100 File objects = ~80 KB JSON = ~5ms to parse
- Keeps UI responsive

### Event Throttling

```rust
// core/src/ops/indexing/state.rs

struct IndexingState {
    // ... existing fields ...

    /// Buffer for pending file events (flush every 100 files or 500ms)
    pending_file_events: Vec<File>,
    last_event_flush: Instant,
}

impl IndexingState {
    fn buffer_file_event(&mut self, file: File) {
        self.pending_file_events.push(file);

        if self.pending_file_events.len() >= 100
            || self.last_event_flush.elapsed() > Duration::from_millis(500)
        {
            self.flush_file_events();
        }
    }

    fn flush_file_events(&mut self) {
        if self.pending_file_events.is_empty() {
            return;
        }

        self.events.emit(Event::ResourceChangedBatch {
            resource_type: "file".to_string(),
            resources: serde_json::to_value(&self.pending_file_events).unwrap(),
        });

        self.pending_file_events.clear();
        self.last_event_flush = Instant::now();
    }
}
```

### Memory Impact

**Scenario**: Indexing 10,000 files

**Without events**:
- Memory: ~5 MB (just Entry records)

**With events (Option A)**:
- Memory: ~5 MB (Entry records) + ~2 MB (event buffer) = **7 MB**
- Peak: ~8 MB when flushing batches

**With events (Option B - ID only)**:
- Memory: ~5 MB (Entry records) + ~100 KB (event buffer) = **5.1 MB**
- But: Frontend must refetch Files (database queries repeated)

**Trade-off**: +40% memory for 10,000 files is acceptable for instant UI updates.

---

## Edge Cases

### 1. Files Updated During Indexing

**Problem**: User modifies file while indexing is running

**Solution**: Last-write-wins (same as Location)
- Content phase detects hash mismatch
- Rescans affected files
- Emits updated File event
- Frontend sees update instantly

### 2. Large Batches (100,000+ files)

**Problem**: Indexing huge directories (e.g., node_modules)

**Solution**: Adaptive batching
```rust
const MAX_EVENT_SIZE: usize = 100; // Files per event

if batch.len() > MAX_EVENT_SIZE {
    for chunk in batch.chunks(MAX_EVENT_SIZE) {
        emit_file_events(ctx, chunk).await?;
    }
} else {
    emit_file_events(ctx, &batch).await?;
}
```

### 3. Thumbnail Generation (Separate Job)

**Problem**: Thumbnails generated after indexing completes

**Solution**: Emit File events when Sidecar is created
```rust
// core/src/ops/media/thumbnail/action.rs

async fn save_thumbnail(ctx: &Context, sidecar: Sidecar) -> Result<()> {
    // Insert sidecar
    entity::sidecar::ActiveModel::insert(sidecar, ctx.db()).await?;

    // Emit File event (triggers frontend update)
    let entry = find_entry_by_content_uuid(ctx.db(), sidecar.content_uuid).await?;
    let file = File::from_entry_ids(ctx.db(), &[entry.id]).await?;

    ctx.events.emit(Event::ResourceChanged {
        resource_type: "file".to_string(),
        resource: serde_json::to_value(&file[0]).unwrap(),
    });

    Ok(())
}
```

### 4. Files Without UUIDs (Pre-Content Phase)

**Problem**: Regular files have `uuid = None` until Content phase

**Solution**: Don't emit events until UUID is assigned
```rust
// Only emit if entry has UUID
if entry.uuid.is_some() {
    emit_file_events(ctx, &[entry.id]).await?;
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_file_from_entry_ids() {
    let db = setup_test_db().await;

    // Create test entries with content identities
    let entry_ids = create_test_entries(&db, 100).await;

    // Construct Files
    let files = File::from_entry_ids(&db, &entry_ids).await.unwrap();

    assert_eq!(files.len(), 100);
    assert!(files.iter().all(|f| f.id != Uuid::nil()));
}

#[tokio::test]
async fn test_batch_event_emission() {
    let ctx = setup_test_context().await;

    // Subscribe to events
    let mut events = ctx.events.subscribe();

    // Process batch
    process_content_batch(&ctx, test_files).await.unwrap();

    // Verify event emitted
    let event = events.recv().await.unwrap();
    assert!(matches!(event, Event::ResourceChangedBatch { .. }));
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

        // Wait for events
        await waitFor(() => {
            expect(result.current.data?.entries.length).toBeGreaterThan(0);
        });

        // Verify files have content identities
        const filesWithContent = result.current.data?.entries.filter(
            f => f.content_identity !== null
        );
        expect(filesWithContent.length).toBeGreaterThan(0);
    });
});
```

---

## Migration Path

### Phase 1: Foundation (Week 1)
- [ ] Implement `Identifiable` for `File`
- [ ] Extract `File::from_entry_ids()` helper
- [ ] Add `ResourceChangedBatch` event variant
- [ ] Update frontend to handle batch events

### Phase 2: Processing Phase Events (Week 2)
- [ ] Emit File events for directories in Processing phase
- [ ] Test with simple directory structures
- [ ] Verify normalized cache updates

### Phase 3: Content Phase Events (Week 3)
- [ ] Emit File events in Content phase (after UUID assignment)
- [ ] Test with large file sets (1000+ files)
- [ ] Tune batch sizes for performance

### Phase 4: Thumbnail Events (Week 4)
- [ ] Emit File events when Sidecar created
- [ ] Test thumbnail appearance in UI
- [ ] Verify no duplicate events

### Phase 5: Optimization (Week 5)
- [ ] Add event throttling (flush every 100 files or 500ms)
- [ ] Implement adaptive batching for huge directories
- [ ] Profile memory usage under load

---

## Future: Generic Virtual Resource System

Once File events work, we can generalize:

```rust
// core/src/domain/resource.rs

pub trait Identifiable {
    fn id(&self) -> Uuid;
    fn resource_type() -> &'static str;

    // NEW: Declare dependencies
    fn sync_dependencies() -> &'static [&'static str] {
        &[]  // Default: no dependencies
    }

    // NEW: Construct from dependent resource changes
    async fn rebuild_from_dependency(
        db: &DatabaseConnection,
        dependency_type: &str,
        dependency_id: Uuid,
    ) -> Result<Vec<Self>> where Self: Sized {
        unimplemented!("Override if virtual resource")
    }
}

// Example: File depends on Entry, ContentIdentity, Sidecar
impl Identifiable for File {
    fn sync_dependencies() -> &'static [&'static str] {
        &["entry", "content_identity", "sidecar"]
    }

    async fn rebuild_from_dependency(
        db: &DatabaseConnection,
        dependency_type: &str,
        dependency_id: Uuid,
    ) -> Result<Vec<Self>> {
        match dependency_type {
            "content_identity" => {
                // Find all Entries with this content_id
                let entries = find_entries_by_content_uuid(db, dependency_id).await?;
                Self::from_entry_ids(db, &entries).await
            }
            "sidecar" => {
                // Find Entry by content_uuid
                let entry = find_entry_by_content_uuid(db, dependency_id).await?;
                Self::from_entry_ids(db, &[entry.id]).await
            }
            _ => Ok(vec![]),
        }
    }
}
```

**Resource Manager** (future):
```rust
struct ResourceManager {
    // Maps "content_identity" → ["file"]
    dependency_graph: HashMap<&'static str, Vec<&'static str>>,
}

impl ResourceManager {
    async fn emit_dependent_events(
        &self,
        ctx: &Context,
        source_type: &str,
        source_id: Uuid,
    ) -> Result<()> {
        if let Some(dependents) = self.dependency_graph.get(source_type) {
            for dependent_type in dependents {
                // Call rebuild_from_dependency trait method
                let resources = match *dependent_type {
                    "file" => File::rebuild_from_dependency(ctx.db(), source_type, source_id).await?,
                    _ => continue,
                };

                // Emit batch event
                ctx.events.emit(Event::ResourceChangedBatch {
                    resource_type: dependent_type.to_string(),
                    resources: serde_json::to_value(&resources).unwrap(),
                });
            }
        }
        Ok(())
    }
}
```

But for now, **explicit File event emission in indexing phases is simpler and sufficient.**

---

## Decision: Go with Option A (Eager File Construction)

**Rationale**:
1. Simpler architecture (no ResourceMapper needed)
2. Normalized cache works out-of-the-box
3. Reuses existing `directory_listing` SQL logic
4. Matches Location event pattern (full struct)
5. Performance acceptable (less than 100 KB per batch)
6. No extra round-trips (frontend gets complete data)

**Next Steps**:
1. Review this design with team
2. Prototype `File::from_entry_ids()` extraction
3. Test batch event emission in Processing phase
4. Measure memory impact with 10,000 file test case
5. Iterate based on findings

---

## Open Questions

1. **Should we emit events for every file during indexing?**
   - Pro: Complete reactivity, users see everything
   - Con: 100,000 files = 1000 events (even with batching)
   - Compromise: Only emit for visible directories? (requires path filtering)

2. **How do we handle pagination?**
   - Directory listing uses LIMIT/OFFSET
   - Events don't know which page a file belongs to
   - Solution: Normalized cache updates all pages? Or invalidate query?

3. **Should thumbnail generation emit separate events?**
   - Pro: More granular updates (file → content ID → thumbnail)
   - Con: 3 events per file (Processing, Content, Thumbnail)
   - Current plan: Yes, emit all 3 (user sees progressive enhancement)

4. **What about file moves/renames?**
   - Currently: EntryMoved event exists but doesn't emit File struct
   - Should file moves also use ResourceChanged?
   - Answer: Yes, emit updated File with new path

---

## Summary

**Goal**: Files should update reactively in UI as indexing progresses

**Solution**: Emit `ResourceChangedBatch` events with full File structs

**Key Insight**: File is virtual (Entry + ContentIdentity + Sidecar), so we construct Files eagerly on backend and emit complete structs

**Performance**: Acceptable (less than 100 KB per batch, less than 10 MB total for 10,000 files)

**Timeline**: 5 weeks (foundation → processing → content → thumbnails → optimization)

**Risk**: Medium (proven pattern with Location, but Files are more complex)

**Reward**: High (instant UI updates, best UX for indexing)
