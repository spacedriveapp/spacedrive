---
id: VSS-006
title: Ephemeral Sidecar System
status: To Do
assignee: jamiepine
parent: CORE-008
priority: High
tags: [core, vdfs, sidecars, ephemeral, thumbnails]
last_updated: 2025-12-24
related_tasks: [CORE-008, VSS-001, VSS-002, INDEX-000]
---

## Overview

Implement an ephemeral sidecar system for generating and managing derivative files (thumbnails, previews) for ephemeral locations. Unlike managed locations that use content-addressed sidecars stored in the library folder, ephemeral sidecars are stored in the system temp directory and use entry-based identifiers (UUIDs) since ephemeral entries lack content IDs.

## Problem Statement

The current sidecar system (VSS) works exclusively with managed locations:
- **Content-addressed:** Sidecars are stored by content hash (`content_uuid`)
- **Library-scoped:** Stored in `~/.sdlibrary/sidecars/`
- **Database-tracked:** All sidecars have database records
- **Batch generation:** Thumbnails generated for entire folders during indexing

This doesn't work for ephemeral locations because:
1. No content IDs (ephemeral entries aren't hashed)
2. Ephemeral data shouldn't persist in the library folder
3. Database tracking would create excessive overhead for temporary data
4. Full-folder generation wastes resources (user might only view a few files)

## Requirements

### Functional

1. **On-Demand Generation:** Generate thumbnails only for items visible in the explorer viewport
2. **Temp Storage:** Store ephemeral sidecars in system temp directory, not library folder
3. **Entry-Based IDs:** Use entry UUIDs (already in `EphemeralIndex.entry_uuids`) as identifiers
4. **No Database:** Query sidecar existence directly from filesystem, no database records
5. **In-Memory Cache:** Track which ephemeral sidecars exist to avoid filesystem queries
6. **Resource Events:** Emit events when sidecars are generated so frontend can display them
7. **Reuse Serving API:** Use existing `/sidecar/` HTTP endpoint with minimal changes
8. **File Inspector Support:** Show ephemeral sidecars in the file inspector for transparency
9. **Extensible:** Support multiple sidecar types (thumbnails, previews) not just thumbnails

### Non-Functional

1. **Fast Viewport Loading:** Thumbnails appear as user scrolls without blocking
2. **Memory Efficiency:** Leverage existing `EphemeralIndexCache` structure
3. **Cleanup:** Remove temp sidecars when session ends or cache is cleared
4. **Concurrent Safe:** Handle multiple viewport requests without race conditions

## Architecture

### Storage Structure

```
/tmp/spacedrive-ephemeral-{library_id}/
├── sidecars/
│   └── entry/
│       ├── {entry_uuid}/
│       │   ├── thumbs/
│       │   │   ├── grid@1x.webp
│       │   │   └── detail@2x.webp
│       │   ├── previews/
│       │   │   └── video.mp4
│       │   └── transcript/
│       │       └── audio.txt
│       └── {another_entry_uuid}/
│           └── thumbs/...
```

**Key differences from managed sidecars:**
- Lives in **temp directory**, not library folder
- Organized by **entry UUID**, not content hash
- **No sharding** (fewer entries, simpler structure)
- **Auto-cleanup** on session end

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Frontend Explorer                                                │
│ ┌─────────────────────┐                                         │
│ │ Viewport Calculation│ (calculates visible entry IDs)           │
│ └──────────┬──────────┘                                         │
│            │                                                      │
│            v                                                      │
│ ┌─────────────────────┐                                         │
│ │ Request Thumbnails  │ (POST /ephemeral/thumbnails)            │
│ └──────────┬──────────┘                                         │
└────────────┼──────────────────────────────────────────────────┘
             │
             v
┌─────────────────────────────────────────────────────────────────┐
│ Core: Ephemeral Sidecar Handler                                 │
│ ┌──────────────────────────────────────────────────────────┐   │
│ │ 1. Check EphemeralSidecarCache for existing thumbnails   │   │
│ │ 2. For missing, dispatch EphemeralThumbnailJob           │   │
│ │ 3. Return immediate response (existing + pending)        │   │
│ └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
             │
             v
┌─────────────────────────────────────────────────────────────────┐
│ EphemeralThumbnailJob                                            │
│ ┌──────────────────────────────────────────────────────────┐   │
│ │ 1. Resolve entry UUIDs to paths via EphemeralIndex       │   │
│ │ 2. Generate thumbnails to temp directory                 │   │
│ │ 3. Update EphemeralSidecarCache                          │   │
│ │ 4. Emit ResourceEvent::SidecarGenerated                  │   │
│ └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
             │
             v
┌─────────────────────────────────────────────────────────────────┐
│ Frontend: Resource Event Listener                               │
│ ┌──────────────────────────────────────────────────────────┐   │
│ │ 1. Receive SidecarGenerated events                        │   │
│ │ 2. Update UI to show thumbnails as they complete         │   │
│ │ 3. Load via /sidecar/{library_id}/{entry_uuid}/thumb/... │   │
│ └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Core Data Structures

#### 1.1 EphemeralSidecarCache

Location: `core/src/ops/indexing/ephemeral/sidecar_cache.rs`

```rust
/// In-memory cache of ephemeral sidecar existence
pub struct EphemeralSidecarCache {
    /// entry_uuid -> kind -> variant -> exists
    entries: RwLock<HashMap<Uuid, HashMap<String, HashSet<String>>>>,
    /// Temp directory root for this library
    temp_root: PathBuf,
    /// Library ID
    library_id: Uuid,
}

impl EphemeralSidecarCache {
    pub fn new(library_id: Uuid) -> std::io::Result<Self>;

    /// Check if a sidecar exists (in-memory, no I/O)
    pub fn has(&self, entry_uuid: &Uuid, kind: &str, variant: &str) -> bool;

    /// Record that a sidecar was generated
    pub fn insert(&self, entry_uuid: Uuid, kind: String, variant: String);

    /// Get the filesystem path for a sidecar
    pub fn compute_path(
        &self,
        entry_uuid: &Uuid,
        kind: &str,
        variant: &str,
        format: &str,
    ) -> PathBuf;

    /// Bootstrap: scan temp directory and populate cache
    pub async fn scan_existing(&self) -> std::io::Result<usize>;

    /// Cleanup: remove all ephemeral sidecars for this library
    pub async fn clear_all(&self) -> std::io::Result<usize>;
}
```

**Path structure:**
```
/tmp/spacedrive-ephemeral-{library_id}/sidecars/entry/{entry_uuid}/{kind}s/{variant}.{format}
```

**Bootstrap on startup:**
- Scan temp directory for existing sidecars (from previous session)
- Populate cache with found entries
- Remove orphaned sidecars (entries no longer in `EphemeralIndex`)

#### 1.2 Integration with EphemeralIndexCache

Location: `core/src/ops/indexing/ephemeral/cache.rs`

Add ephemeral sidecar cache to the existing cache:

```rust
pub struct EphemeralIndexCache {
    // ... existing fields ...

    /// Ephemeral sidecar cache (lazy-initialized per library)
    sidecar_cache: RwLock<Option<Arc<EphemeralSidecarCache>>>,
}

impl EphemeralIndexCache {
    /// Get or create the ephemeral sidecar cache
    pub fn get_sidecar_cache(&self, library_id: Uuid) -> Arc<EphemeralSidecarCache>;
}
```

### Phase 2: Thumbnail Generation Job

#### 2.1 EphemeralThumbnailJob

Location: `core/src/ops/media/thumbnail/ephemeral_job.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct EphemeralThumbnailJob {
    /// Entry UUIDs to generate thumbnails for (from viewport)
    pub entry_uuids: Vec<Uuid>,

    /// Target variant (typically "grid@1x" for viewport)
    pub variant: String,

    /// Library ID
    pub library_id: Uuid,

    /// Maximum concurrent generations
    pub max_concurrent: usize,
}

impl JobHandler for EphemeralThumbnailJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // 1. Get ephemeral index and sidecar cache
        let index = ctx.core.ephemeral_cache.get_global_index();
        let sidecar_cache = ctx.core.ephemeral_cache
            .get_sidecar_cache(self.library_id);

        // 2. Resolve entry UUIDs to filesystem paths
        let paths = self.resolve_entry_paths(&index).await?;

        // 3. Filter out entries that already have thumbnails
        let missing = self.filter_missing(&sidecar_cache, &paths);

        // 4. Generate thumbnails in parallel (max_concurrent)
        for batch in missing.chunks(self.max_concurrent) {
            self.generate_batch(batch, &sidecar_cache, &ctx).await?;
        }

        Ok(ThumbnailOutput { ... })
    }
}
```

**Key behaviors:**
- Only generates thumbnails that don't exist
- Emits `ResourceEvent::SidecarGenerated(entry_uuid, kind, variant)` per thumbnail
- Updates `EphemeralSidecarCache` immediately after generation
- Non-blocking: Frontend can request more while job runs

#### 2.2 Generation Strategy

**Small variant first:**
- Always generate `grid@1x` (smallest) for viewport
- On-demand generate `detail@2x` when user clicks/inspects
- Avoids wasting I/O on high-res thumbnails user might not need

**Deduplication:**
- If user scrolls fast, multiple viewport requests might overlap
- Check cache before generating to avoid duplicate work
- Use `active_tasks` map (like `SidecarManager`) to prevent concurrent generation of same thumbnail

### Phase 3: HTTP Server Integration

#### 3.1 Extend Sidecar Endpoint

Location: `apps/tauri/src-tauri/src/server.rs`

Modify `serve_sidecar` to support ephemeral sidecars:

```rust
async fn serve_sidecar(
    State(state): State<ServerState>,
    Path((library_id, uuid, kind, variant_and_ext)): Path<(String, String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
    // Try managed sidecar first (content_uuid)
    if let Ok(uuid) = Uuid::parse_str(&uuid) {
        if let Ok(response) = serve_managed_sidecar(&state, &library_id, uuid, &kind, &variant_and_ext).await {
            return Ok(response);
        }
    }

    // Fallback to ephemeral sidecar (entry_uuid)
    if let Ok(uuid) = Uuid::parse_str(&uuid) {
        if let Ok(response) = serve_ephemeral_sidecar(&state, &library_id, uuid, &kind, &variant_and_ext).await {
            return Ok(response);
        }
    }

    Err(StatusCode::NOT_FOUND)
}

async fn serve_ephemeral_sidecar(
    state: &ServerState,
    library_id: &str,
    entry_uuid: Uuid,
    kind: &str,
    variant_and_ext: &str,
) -> Result<Response<Body>, StatusCode> {
    // Construct path: /tmp/spacedrive-ephemeral-{library_id}/sidecars/entry/{entry_uuid}/{kind}s/{variant}.{ext}
    let temp_root = std::env::temp_dir()
        .join(format!("spacedrive-ephemeral-{}", library_id));

    let kind_dir = if kind == "transcript" {
        kind.to_string()
    } else {
        format!("{}s", kind)
    };

    let sidecar_path = temp_root
        .join("sidecars")
        .join("entry")
        .join(entry_uuid.to_string())
        .join(&kind_dir)
        .join(variant_and_ext);

    // Security: ensure path is under temp_root
    if !sidecar_path.starts_with(&temp_root) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Serve file (same logic as managed sidecars)
    serve_file(&sidecar_path).await
}
```

**URL format:**
```
http://localhost:{port}/sidecar/{library_id}/{entry_uuid}/thumb/grid@1x.webp
```

**Ambiguity resolution:**
- Entry UUIDs and content UUIDs are both UUIDs
- Try managed first (most common)
- Fallback to ephemeral if not found
- This allows same frontend code for both

### Phase 4: Frontend Integration

#### 4.1 Viewport Thumbnail Request

Location: `packages/interface/src/components/Explorer/VirtualGrid.tsx`

```typescript
// Calculate visible entries
const visibleEntries = virtualizer.getVirtualItems().map(item => items[item.index]);
const visibleEntryIds = visibleEntries.map(e => e.id);

// Request ephemeral thumbnails for visible items
useEffect(() => {
  if (isEphemeral && visibleEntryIds.length > 0) {
    requestEphemeralThumbnails.mutate({
      libraryId: currentLibrary.id,
      entryUuids: visibleEntryIds,
      variant: "grid@1x",
    });
  }
}, [visibleEntryIds, isEphemeral]);

// Listen for sidecar generation events
useEvent('resource', (event) => {
  if (event.type === 'SidecarGenerated') {
    // Invalidate query or update state to show thumbnail
    queryClient.invalidateQueries(['ephemeral', event.entryUuid]);
  }
});
```

**Batching:**
- Debounce viewport changes (e.g., 150ms) to avoid request spam
- Send batch of visible entry IDs in single request
- Backend filters out entries that already have thumbnails

#### 4.2 Thumbnail URL Building

Location: `packages/interface/src/ServerContext.tsx`

```typescript
buildSidecarUrl = (identifier: string, kind: string, variant: string, format: string) => {
  // identifier is either content_uuid (managed) or entry_uuid (ephemeral)
  return `${serverUrl}/sidecar/${libraryId}/${identifier}/${kind}/${variant}.${format}`;
};
```

**Frontend doesn't need to know** if it's managed vs ephemeral:
- Same URL structure
- Server resolves the ambiguity

#### 4.3 File Inspector

Location: `packages/interface/src/components/Inspector/FileInspector.tsx`

Show ephemeral sidecars for transparency:

```typescript
const { data: sidecars } = useQuery({
  queryKey: ['ephemeral-sidecars', file.id],
  queryFn: async () => {
    if (isEphemeral) {
      // Query filesystem for ephemeral sidecars
      return client.query({
        type: 'ephemeral.list_sidecars',
        input: { entryUuid: file.uuid },
      });
    } else {
      // Existing managed sidecar logic
      return file.sidecars;
    }
  },
});

// Display
{sidecars?.map(sidecar => (
  <div key={sidecar.variant}>
    {sidecar.kind}: {sidecar.variant} ({sidecar.size} bytes)
    {isEphemeral && <Badge>Temporary</Badge>}
  </div>
))}
```

### Phase 5: Query & Action Layer

#### 5.1 List Ephemeral Sidecars Query

Location: `core/src/ops/queries/ephemeral.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ListEphemeralSidecarsInput {
    pub entry_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EphemeralSidecarInfo {
    pub kind: String,
    pub variant: String,
    pub format: String,
    pub size: u64,
    pub path: PathBuf,
}

pub async fn list_ephemeral_sidecars(
    ctx: &CoreContext,
    input: ListEphemeralSidecarsInput,
) -> Result<Vec<EphemeralSidecarInfo>> {
    let cache = ctx.ephemeral_cache.get_sidecar_cache(ctx.current_library_id()?);

    // Scan filesystem for this entry's sidecars
    let entry_dir = cache.compute_entry_dir(&input.entry_uuid);

    if !entry_dir.exists() {
        return Ok(vec![]);
    }

    let mut sidecars = vec![];

    // Iterate over kind directories (thumbs/, previews/, etc.)
    for kind_dir in std::fs::read_dir(&entry_dir)? {
        let kind_dir = kind_dir?;
        let kind = kind_dir.file_name().to_string_lossy().trim_end_matches('s').to_string();

        // Iterate over sidecar files
        for file in std::fs::read_dir(kind_dir.path())? {
            let file = file?;
            let filename = file.file_name().to_string_lossy().to_string();
            let (variant, format) = filename.rsplit_once('.').unwrap_or((&filename, ""));

            sidecars.push(EphemeralSidecarInfo {
                kind: kind.clone(),
                variant: variant.to_string(),
                format: format.to_string(),
                size: file.metadata()?.len(),
                path: file.path(),
            });
        }
    }

    Ok(sidecars)
}
```

#### 5.2 Request Ephemeral Thumbnails Action

Location: `core/src/ops/actions/ephemeral_thumbnails.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestEphemeralThumbnailsInput {
    pub entry_uuids: Vec<Uuid>,
    pub variant: String,
    pub library_id: Uuid,
}

pub async fn request_ephemeral_thumbnails(
    ctx: &CoreContext,
    input: RequestEphemeralThumbnailsInput,
) -> Result<RequestEphemeralThumbnailsOutput> {
    let sidecar_cache = ctx.ephemeral_cache.get_sidecar_cache(input.library_id);

    // Filter out entries that already have thumbnails
    let missing: Vec<Uuid> = input.entry_uuids
        .into_iter()
        .filter(|uuid| !sidecar_cache.has(uuid, "thumb", &input.variant))
        .collect();

    if missing.is_empty() {
        return Ok(RequestEphemeralThumbnailsOutput {
            requested: 0,
            already_exist: input.entry_uuids.len(),
        });
    }

    // Dispatch job
    let job = EphemeralThumbnailJob {
        entry_uuids: missing.clone(),
        variant: input.variant,
        library_id: input.library_id,
        max_concurrent: 4,
    };

    ctx.job_manager.enqueue(job).await?;

    Ok(RequestEphemeralThumbnailsOutput {
        requested: missing.len(),
        already_exist: input.entry_uuids.len() - missing.len(),
    })
}
```

### Phase 6: Lifecycle & Cleanup

#### 6.1 Session Cleanup

Location: `core/src/ops/indexing/ephemeral/cache.rs`

```rust
impl EphemeralIndexCache {
    /// Clear all ephemeral data (index + sidecars)
    pub async fn clear_all(&self) -> usize {
        // Clear index entries
        let cleared_paths = { /* existing logic */ };

        // Clear ephemeral sidecars
        if let Some(sidecar_cache) = self.sidecar_cache.write().take() {
            let _ = sidecar_cache.clear_all().await;
        }

        cleared_paths
    }
}
```

**Trigger cleanup on:**
- User navigates away from ephemeral location
- App shutdown
- Manual "Clear Cache" action in settings

#### 6.2 Orphan Cleanup

On bootstrap, remove ephemeral sidecars for entries that no longer exist:

```rust
impl EphemeralSidecarCache {
    pub async fn cleanup_orphans(&self, index: &EphemeralIndex) -> std::io::Result<usize> {
        let entry_uuids = index.all_uuids();
        let mut removed = 0;

        for entry_dir in std::fs::read_dir(&self.temp_root.join("sidecars/entry"))? {
            let entry_dir = entry_dir?;
            let entry_uuid = Uuid::parse_str(&entry_dir.file_name().to_string_lossy())
                .ok();

            if let Some(uuid) = entry_uuid {
                if !entry_uuids.contains(&uuid) {
                    // Entry no longer in index, remove sidecars
                    std::fs::remove_dir_all(entry_dir.path())?;
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }
}
```

### Phase 7: Resource Events

#### 7.1 Event Types

Location: `core/src/infra/event/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceEvent {
    // ... existing events ...

    /// Ephemeral sidecar was generated
    EphemeralSidecarGenerated {
        entry_uuid: Uuid,
        kind: String,
        variant: String,
        format: String,
        size: u64,
    },

    /// Ephemeral sidecars cleared
    EphemeralSidecarsCleared {
        count: usize,
    },
}
```

#### 7.2 Emission

Location: `core/src/ops/media/thumbnail/ephemeral_job.rs`

```rust
impl EphemeralThumbnailJob {
    async fn generate_batch(&mut self, batch: &[...], ctx: &JobContext<'_>) -> Result<()> {
        for entry in batch {
            let thumbnail_path = self.generate_thumbnail(entry)?;
            let size = thumbnail_path.metadata()?.len();

            // Update cache
            sidecar_cache.insert(entry.uuid, "thumb".to_string(), self.variant.clone());

            // Emit event
            ctx.emit_event(ResourceEvent::EphemeralSidecarGenerated {
                entry_uuid: entry.uuid,
                kind: "thumb".to_string(),
                variant: self.variant.clone(),
                format: "webp".to_string(),
                size,
            }).await?;
        }

        Ok(())
    }
}
```

## Extensibility

### Supporting Additional Sidecar Types

The system is designed to support more than just thumbnails:

**1. Video Previews:**
```rust
pub struct EphemeralPreviewJob {
    pub entry_uuids: Vec<Uuid>,
    pub quality: String, // "low", "medium", "high"
}
```

**2. Audio Transcripts:**
```rust
pub struct EphemeralTranscriptJob {
    pub entry_uuids: Vec<Uuid>,
    pub language: Option<String>,
}
```

**3. OCR Text:**
```rust
pub struct EphemeralOcrJob {
    pub entry_uuids: Vec<Uuid>,
}
```

All follow the same pattern:
1. Check `EphemeralSidecarCache`
2. Generate to temp directory
3. Update cache
4. Emit event

## Migration Path

### From Ephemeral to Managed

When user promotes an ephemeral location to a managed location:

```rust
pub async fn promote_ephemeral_to_managed(
    ctx: &CoreContext,
    path: PathBuf,
) -> Result<()> {
    let ephemeral_cache = &ctx.ephemeral_cache;
    let sidecar_cache = ephemeral_cache.get_sidecar_cache(ctx.current_library_id()?);

    // 1. Index entries normally (generates content IDs)
    let indexed_entries = index_location(&path).await?;

    // 2. For each entry with ephemeral sidecars:
    for (entry_uuid, content_uuid) in indexed_entries {
        if sidecar_cache.has(&entry_uuid, "thumb", "grid@1x") {
            // Copy ephemeral sidecar to managed location
            let ephemeral_path = sidecar_cache.compute_path(&entry_uuid, "thumb", "grid@1x", "webp");
            let managed_path = ctx.sidecar_manager
                .compute_path(&ctx.current_library_id()?, &content_uuid, &SidecarKind::Thumb, &"grid@1x".into(), &SidecarFormat::Webp)
                .await?;

            tokio::fs::copy(ephemeral_path, &managed_path.absolute_path).await?;

            // Record in database
            ctx.sidecar_manager.record_sidecar(...).await?;
        }
    }

    // 3. Clear ephemeral sidecars
    sidecar_cache.clear_all().await?;

    Ok(())
}
```

**Benefits:**
- Reuse already-generated thumbnails
- Faster promotion (no regeneration needed)
- Seamless UX (thumbnails don't disappear)

## Performance Characteristics

### Memory

**Per ephemeral sidecar in cache:**
```
UUID (16 bytes) + kind (24 bytes) + variant (24 bytes) = ~64 bytes
```

**For 10,000 ephemeral entries with 2 thumbnails each:**
```
10,000 entries × 2 variants × 64 bytes = 1.28 MB
```

Negligible compared to the `EphemeralIndex` itself (~50 bytes per entry = 500 KB).

### Disk

**Thumbnails:**
- `grid@1x.webp`: ~5-15 KB
- `detail@2x.webp`: ~50-100 KB

**For 1,000 visible files:**
```
1,000 × 10 KB (grid@1x) = 10 MB
```

**Cleanup:**
- Temp directory is OS-managed (auto-cleanup on reboot)
- Explicit cleanup on session end
- Orphan cleanup on bootstrap

### Network

**No network overhead:**
- Ephemeral sidecars are local-only
- No sync protocol needed
- No cross-device transfer

## Testing Plan

### Unit Tests

1. **EphemeralSidecarCache:**
   - `test_insert_and_has()`
   - `test_compute_path()`
   - `test_scan_existing()`
   - `test_clear_all()`

2. **EphemeralThumbnailJob:**
   - `test_filter_missing()`
   - `test_resolve_entry_paths()`
   - `test_generate_batch()`

### Integration Tests

1. **End-to-End Workflow:**
   ```rust
   #[tokio::test]
   async fn test_ephemeral_thumbnail_generation() {
       // 1. Create ephemeral index with test files
       // 2. Request thumbnails for viewport
       // 3. Verify job generates thumbnails
       // 4. Verify cache is updated
       // 5. Verify events are emitted
       // 6. Verify HTTP endpoint serves thumbnails
   }
   ```

2. **Cleanup:**
   ```rust
   #[tokio::test]
   async fn test_ephemeral_sidecar_cleanup() {
       // 1. Generate ephemeral sidecars
       // 2. Clear ephemeral cache
       // 3. Verify temp directory is empty
   }
   ```

3. **Orphan Removal:**
   ```rust
   #[tokio::test]
   async fn test_orphan_cleanup() {
       // 1. Generate sidecars for entries
       // 2. Remove entries from ephemeral index
       // 3. Run orphan cleanup
       // 4. Verify sidecars are removed
   }
   ```

### Manual Testing

1. **Viewport Scrolling:**
   - Navigate to ephemeral location with 1000+ images
   - Scroll through grid view
   - Verify thumbnails appear progressively
   - Verify no duplicate generation (check logs)

2. **File Inspector:**
   - Select ephemeral file
   - Verify file inspector shows "Temporary" badge on sidecars
   - Verify sidecar list is accurate

3. **Promotion:**
   - Browse ephemeral location, generate thumbnails
   - Promote to managed location
   - Verify thumbnails are preserved
   - Verify ephemeral temp directory is cleaned up

4. **Session Persistence:**
   - Browse ephemeral location, generate thumbnails
   - Restart app
   - Re-browse same location
   - Verify thumbnails are reused (no regeneration)

## Acceptance Criteria

- [ ] Ephemeral sidecars are stored in system temp directory
- [ ] Entry UUIDs are used as identifiers (not content UUIDs)
- [ ] No database records for ephemeral sidecars
- [ ] `EphemeralSidecarCache` tracks existence in-memory
- [ ] Thumbnails generate on-demand for visible viewport items
- [ ] Resource events emitted as thumbnails are generated
- [ ] HTTP endpoint serves ephemeral sidecars via same URL structure
- [ ] File inspector shows ephemeral sidecars with "Temporary" badge
- [ ] Session cleanup removes temp sidecars on cache clear
- [ ] Bootstrap orphan cleanup removes sidecars for deleted entries
- [ ] Viewport scrolling shows thumbnails progressively without blocking
- [ ] Promoting ephemeral location preserves generated thumbnails
- [ ] System works with multiple sidecar types (extensible design)

## Implementation Files

**Core:**
- `core/src/ops/indexing/ephemeral/sidecar_cache.rs` (new)
- `core/src/ops/indexing/ephemeral/cache.rs` (modified)
- `core/src/ops/media/thumbnail/ephemeral_job.rs` (new)
- `core/src/ops/actions/ephemeral_thumbnails.rs` (new)
- `core/src/ops/queries/ephemeral.rs` (modified)
- `core/src/infra/event/types.rs` (modified)

**HTTP Server:**
- `apps/tauri/src-tauri/src/server.rs` (modified)

**Frontend:**
- `packages/interface/src/components/Explorer/VirtualGrid.tsx` (modified)
- `packages/interface/src/components/Inspector/FileInspector.tsx` (modified)
- `packages/interface/src/ServerContext.tsx` (minimal changes)

**Tests:**
- `core/tests/ephemeral_sidecars.rs` (new)

## Future Enhancements

### 1. Smart Prefetching

Prefetch thumbnails for items just outside viewport (next page):

```rust
pub struct PrefetchConfig {
    pub ahead_count: usize, // e.g., 50 items ahead
    pub behind_count: usize, // e.g., 20 items behind
}
```

### 2. Sidecar Limits

Prevent temp directory from growing unbounded:

```rust
pub struct EphemeralSidecarLimits {
    pub max_total_size: u64, // e.g., 500 MB
    pub max_age: Duration, // e.g., 7 days
}
```

LRU eviction when limits are reached.

### 3. Cross-Session Sharing

If multiple users browse the same network share, share temp sidecars:

```
/tmp/spacedrive-ephemeral-shared/{path_hash}/sidecars/...
```

Requires coordination mechanism (lock file, shared cache).

### 4. Progressive Loading

Generate low-quality thumbnail first, then enhance:

```rust
pub enum ThumbnailQuality {
    Fast,    // 1x scale, lower quality (10 KB)
    Normal,  // 1x scale, normal quality (15 KB)
    High,    // 2x scale, high quality (100 KB)
}
```

### 5. Format Selection

Let user choose thumbnail format (webp, avif, jpeg):

```rust
pub struct EphemeralThumbnailConfig {
    pub format: SidecarFormat, // Webp, Avif, Jpeg
    pub quality: u8, // 0-100
}
```

## Related Documentation

- [CORE-008: Virtual Sidecar System](/Users/jamespine/Projects/spacedrive/.tasks/core/CORE-008-virtual-sidecar-system.md)
- [INDEX-000: Indexing & File Management](/Users/jamespine/Projects/spacedrive/.tasks/core/INDEX-000-indexing-file-management.md)
- Whitepaper: Section 4.1.5 (Virtual Sidecars)
- Workbench: `workbench/core/storage/VIRTUAL_SIDECAR_SYSTEM_V2.md`

## Notes

- This design reuses the existing `EphemeralIndex.entry_uuids` mapping, avoiding the need to track inodes
- Entry UUIDs are stable across sessions (as long as the file exists at the same path)
- The HTTP endpoint automatically distinguishes managed vs ephemeral by trying both paths
- This system is completely orthogonal to the managed sidecar system (VSS) - they can coexist without conflicts
- Ephemeral sidecars are a **caching layer**, not persistent storage - they can be deleted without data loss
