# Phase Processor Refactor Plan

## Executive Summary

Refactor the job system to use a universal **PhaseProcessor** pattern that enables:
- Reusable atomic units of work (content hashing, thumbnail generation, OCR, etc.)
- Per-location configuration of which phases to run
- UI-triggered single-file processing ("Regenerate Thumbnail" button)
- Batch processing in jobs with progress tracking
- Clean separation between business logic and orchestration

## Problem Statement

Currently, processing logic is duplicated across:
1. **Responder** - Inline content hashing + thumbnail generation for single files
2. **Indexer Job** - Same logic but with batching and progress tracking
3. **Thumbnail Job** - Standalone thumbnail generation
4. **Future jobs** - OCR, subtitle generation, metadata extraction

Each time we add a new processing capability, we need to:
- Implement it in the job
- Copy-paste it into the responder for real-time updates
- Create UI actions for manual triggering
- Handle configuration in multiple places

## Solution: Atomic Phase Processors

### Core Abstraction

```rust
/// Atomic processor for a single file/entry operation
#[async_trait::async_trait]
pub trait AtomicProcessor: Send + Sync {
    /// Process a single entry
    async fn process(
        &self,
        ctx: &ProcessorContext,
        entry: &ProcessorEntry,
    ) -> ProcessorResult;

    /// Check if this processor should run for the given entry
    fn should_process(&self, entry: &ProcessorEntry) -> bool;

    /// Get processor name for logging/metrics
    fn name(&self) -> &'static str;
}

pub struct ProcessorEntry {
    pub id: i32,
    pub uuid: Option<Uuid>,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    pub content_id: Option<i32>,
    pub mime_type: Option<String>,
}

pub struct ProcessorContext {
    pub library: Arc<Library>,
    pub db: DatabaseConnection,
    pub volume_backend: Option<Arc<dyn VolumeBackend>>,
}

pub struct ProcessorResult {
    pub success: bool,
    pub artifacts_created: usize,
    pub bytes_processed: u64,
    pub error: Option<String>,
}
```

### Concrete Processors

```rust
// core/src/ops/processors/content_hash.rs
pub struct ContentHashProcessor {
    library_id: Uuid,
}

impl AtomicProcessor for ContentHashProcessor {
    async fn process(&self, ctx: &ProcessorContext, entry: &ProcessorEntry) -> ProcessorResult {
        // 1. Generate content hash
        let hash = ContentHashGenerator::generate_content_hash(&entry.path).await?;

        // 2. Link to entry
        EntryProcessor::link_to_content_identity(
            ctx.db, entry.id, &entry.path, hash, self.library_id
        ).await?;

        Ok(ProcessorResult {
            success: true,
            artifacts_created: 1,
            bytes_processed: entry.size,
            error: None,
        })
    }

    fn should_process(&self, entry: &ProcessorEntry) -> bool {
        matches!(entry.kind, EntryKind::File { .. }) && entry.content_id.is_none()
    }

    fn name(&self) -> &'static str {
        "content_hash"
    }
}

// core/src/ops/processors/thumbnail.rs
pub struct ThumbnailProcessor {
    library: Arc<Library>,
    variants: Vec<ThumbnailVariantConfig>,
    regenerate: bool,
}

impl ThumbnailProcessor {
    pub fn new(library: Arc<Library>) -> Self {
        Self {
            library,
            variants: ThumbnailVariants::defaults(),
            regenerate: false,
        }
    }

    pub fn with_config(mut self, config: &ThumbnailConfig) -> Self {
        self.variants = config.variants.clone();
        self.regenerate = config.regenerate;
        self
    }
}

impl AtomicProcessor for ThumbnailProcessor {
    async fn process(&self, ctx: &ProcessorContext, entry: &ProcessorEntry) -> ProcessorResult {
        let content_uuid = /* get from entry */;
        let mime_type = entry.mime_type.as_ref().ok_or(...)?;

        let count = crate::ops::media::thumbnail::generate_thumbnails_for_file(
            &self.library,
            &content_uuid,
            &entry.path,
            mime_type,
        ).await?;

        Ok(ProcessorResult {
            success: true,
            artifacts_created: count,
            bytes_processed: 0,
            error: None,
        })
    }

    fn should_process(&self, entry: &ProcessorEntry) -> bool {
        entry.content_id.is_some() &&
        entry.mime_type.as_ref().map_or(false, |m|
            ThumbnailUtils::is_thumbnail_supported(m)
        )
    }

    fn name(&self) -> &'static str {
        "thumbnail"
    }
}

// Future processors
pub struct OcrProcessor { /* ... */ }
pub struct SubtitleProcessor { /* ... */ }
pub struct MetadataExtractorProcessor { /* ... */ }
```

### Location Configuration

```rust
// core/src/infra/db/entities/location.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationProcessorConfig {
    /// Processors to run during indexing
    pub indexing_processors: Vec<ProcessorConfig>,
    /// Processors to run on watcher events
    pub watcher_processors: Vec<ProcessorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    pub processor_type: String, // "content_hash", "thumbnail", "ocr", "subtitle"
    pub enabled: bool,
    pub settings: serde_json::Value, // Processor-specific settings
}

impl Default for LocationProcessorConfig {
    fn default() -> Self {
        Self {
            indexing_processors: vec![
                ProcessorConfig {
                    processor_type: "content_hash".to_string(),
                    enabled: true,
                    settings: json!({}),
                },
                ProcessorConfig {
                    processor_type: "thumbnail".to_string(),
                    enabled: true,
                    settings: json!({
                        "variants": ["grid@1x", "grid@2x", "detail@1x"],
                        "quality": 85
                    }),
                },
            ],
            watcher_processors: vec![
                ProcessorConfig {
                    processor_type: "content_hash".to_string(),
                    enabled: true,
                    settings: json!({}),
                },
                ProcessorConfig {
                    processor_type: "thumbnail".to_string(),
                    enabled: true,
                    settings: json!({
                        "variants": ["grid@1x", "grid@2x"],
                        "quality": 80
                    }),
                },
            ],
        }
    }
}
```

### Usage Patterns

**1. Responder (single file, watcher events)**

```rust
async fn handle_create(...) -> Result<()> {
    let entry_id = create_entry(...).await?;

    // Load location config
    let location_config = load_location_config(location_id).await?;

    // Build processor entry
    let proc_entry = ProcessorEntry {
        id: entry_id,
        path: path.to_path_buf(),
        /* ... */
    };

    let proc_ctx = ProcessorContext {
        library: library.clone(),
        db: ctx.library_db().clone(),
        volume_backend: None,
    };

    // Run configured watcher processors
    for proc_config in location_config.watcher_processors {
        if !proc_config.enabled {
            continue;
        }

        let processor = create_processor(&proc_config, &library)?;

        if processor.should_process(&proc_entry) {
            match processor.process(&proc_ctx, &proc_entry).await {
                Ok(result) => {
                    debug!("✓ {} completed: {} artifacts", processor.name(), result.artifacts_created);
                }
                Err(e) => {
                    warn!("{} failed for {}: {}", processor.name(), path.display(), e);
                }
            }
        }
    }

    emit_resource_event(...);
}
```

**2. Indexer Job (batch)**

```rust
async fn run_processing_phase(state: &mut IndexerState, ctx: &JobContext) -> Result<()> {
    // Load location config
    let location_config = load_location_config(state.location_id).await?;

    let proc_ctx = ProcessorContext {
        library: ctx.library().clone(),
        db: ctx.library_db().clone(),
        volume_backend: state.volume_backend.clone(),
    };

    // Create all enabled processors
    let processors: Vec<Box<dyn AtomicProcessor>> = location_config
        .indexing_processors
        .iter()
        .filter(|c| c.enabled)
        .map(|c| create_processor(c, ctx.library()))
        .collect();

    for batch in &state.batches {
        for entry in batch {
            ctx.check_interrupt().await?;

            let proc_entry = ProcessorEntry::from_dir_entry(entry);

            // Run all processors on this entry
            for processor in &processors {
                if processor.should_process(&proc_entry) {
                    match processor.process(&proc_ctx, &proc_entry).await {
                        Ok(result) => {
                            state.stats.artifacts_created += result.artifacts_created;
                        }
                        Err(e) => {
                            state.errors.push(IndexError::ProcessorFailed {
                                path: entry.path.clone(),
                                processor: processor.name().to_string(),
                                error: e.to_string(),
                            });
                        }
                    }
                }
            }

            ctx.progress(...);
        }

        ctx.checkpoint().await?;
    }
}
```

**3. UI Action (single file regeneration)**

```rust
// core/src/ops/media/thumbnail/action.rs

#[derive(Serialize, Deserialize, Type)]
pub struct RegenerateThumbnailInput {
    pub entry_uuid: Uuid,
    pub variants: Option<Vec<String>>, // None = defaults
    pub force: bool, // Regenerate even if exists
}

pub struct RegenerateThumbnailAction {
    input: RegenerateThumbnailInput,
}

impl LibraryAction for RegenerateThumbnailAction {
    type Output = RegenerateThumbnailOutput;

    async fn execute(&self, library: &Arc<Library>) -> Result<Self::Output> {
        // Load entry
        let entry = load_entry_by_uuid(&self.input.entry_uuid, library.db()).await?;
        let path = PathResolver::get_full_path(library.db(), entry.id).await?;

        // Create processor with custom config
        let variants = self.input.variants
            .as_ref()
            .map(|v| v.iter().map(|s| ThumbnailVariants::from_str(s)).collect())
            .unwrap_or_else(|| ThumbnailVariants::defaults());

        let processor = ThumbnailProcessor::new(library.clone())
            .with_variants(variants)
            .with_regenerate(self.input.force);

        let proc_entry = ProcessorEntry::from_db_entry(&entry, &path);
        let proc_ctx = ProcessorContext {
            library: library.clone(),
            db: library.db().clone(),
            volume_backend: None,
        };

        let result = processor.process(&proc_ctx, &proc_entry).await?;

        Ok(RegenerateThumbnailOutput {
            generated_count: result.artifacts_created,
            variants: variants.iter().map(|v| v.variant.to_string()).collect(),
        })
    }
}
```

## Implementation Phases

### Phase 1: Create Processor Infrastructure (Week 1)
- [ ] Create `core/src/ops/processors/mod.rs`
- [ ] Define `AtomicProcessor` trait
- [ ] Define `ProcessorContext`, `ProcessorEntry`, `ProcessorResult`
- [ ] Create processor factory function
- [ ] Add unit tests

### Phase 2: Extract Existing Processors (Week 1-2)
- [ ] Extract `ContentHashProcessor` from responder code
- [ ] Extract `ThumbnailProcessor` from thumbnail module
- [ ] Move `generate_thumbnails_for_file` to processor
- [ ] Update responder to use processors
- [ ] Test watcher events still work

### Phase 3: Add Location Configuration (Week 2)
- [ ] Add `processor_config` JSON field to locations table
- [ ] Create migration
- [ ] Add default configs
- [ ] Implement config loading in responder
- [ ] Add UI for editing location processor config

### Phase 4: Add UI Actions (Week 2-3)
- [ ] Create `RegenerateThumbnailAction`
- [ ] Add action registration
- [ ] Wire up to Inspector "Regenerate Thumbnail" button
- [ ] Test manual regeneration works

### Phase 5: Refactor Jobs to Use Processors (Week 3-4)
- [ ] Update IndexerJob processing phase to use processors
- [ ] Update ThumbnailJob to use processor
- [ ] Remove duplicate code
- [ ] Test batch processing still works
- [ ] Verify resumability preserved

### Phase 6: Add Future Processors (Week 4+)
- [ ] Create `OcrProcessor` with tesseract integration
- [ ] Create `SubtitleProcessor` for video subtitle extraction
- [ ] Create `MetadataExtractorProcessor` for EXIF/ID3/etc
- [ ] Add to location config options
- [ ] Add UI actions for manual triggering

## File Structure

```
core/src/ops/
├── processors/
│   ├── mod.rs              # AtomicProcessor trait + factory
│   ├── content_hash.rs     # ContentHashProcessor
│   ├── thumbnail.rs        # ThumbnailProcessor
│   ├── ocr.rs              # OcrProcessor (future)
│   ├── subtitle.rs         # SubtitleProcessor (future)
│   └── metadata.rs         # MetadataExtractorProcessor (future)
├── indexing/
│   ├── responder.rs        # Uses processors via factory
│   ├── job.rs              # Uses processors in batches
│   └── ...
└── media/
    ├── thumbnail/
    │   ├── mod.rs          # Keep generate_thumbnails_for_file helper
    │   ├── action.rs       # RegenerateThumbnailAction uses ThumbnailProcessor
    │   └── ...
    └── ...
```

## Migration Strategy

### Option A: Gradual Migration (Recommended)
1. Create processor infrastructure alongside existing code
2. Update responder to use processors (prove it works)
3. Add location config (opt-in, defaults to current behavior)
4. Migrate jobs one by one
5. Remove old code once all migrated

**Pros**: Low risk, can ship incrementally, easy rollback
**Cons**: Temporary duplication

### Option B: Big Bang Refactor
1. Design entire system upfront
2. Implement all processors
3. Refactor all jobs at once
4. Switch over in one PR

**Pros**: Clean, no duplication
**Cons**: High risk, long development time, hard to test

**Recommendation**: **Option A** - gradual migration with feature flags

## Location Processor Config Schema

```json
{
  "location_id": "uuid",
  "processor_config": {
    "indexing": {
      "processors": [
        {
          "type": "content_hash",
          "enabled": true,
          "settings": {}
        },
        {
          "type": "thumbnail",
          "enabled": true,
          "settings": {
            "variants": ["grid@1x", "grid@2x", "detail@1x"],
            "quality": 85,
            "skip_existing": true
          }
        },
        {
          "type": "ocr",
          "enabled": false,
          "settings": {
            "languages": ["eng"],
            "dpi": 300
          }
        }
      ]
    },
    "watcher": {
      "processors": [
        {
          "type": "content_hash",
          "enabled": true,
          "settings": {}
        },
        {
          "type": "thumbnail",
          "enabled": true,
          "settings": {
            "variants": ["grid@1x", "grid@2x"],
            "quality": 80
          }
        }
      ]
    }
  }
}
```

## Immediate Next Steps (This Week)

### Step 1: Create Processor Module (2 hours)
```bash
# Create files
touch core/src/ops/processors/mod.rs
touch core/src/ops/processors/content_hash.rs
touch core/src/ops/processors/thumbnail.rs
```

Define the `AtomicProcessor` trait and basic types.

### Step 2: Extract ContentHashProcessor (2 hours)
Move the content hashing logic from responder into `ContentHashProcessor::process()`.

### Step 3: Extract ThumbnailProcessor (2 hours)
Move `generate_thumbnails_for_file` logic into `ThumbnailProcessor::process()`.

### Step 4: Update Responder (2 hours)
Replace inline code with processor calls:
```rust
let processors = vec![
    Box::new(ContentHashProcessor::new(library_id)) as Box<dyn AtomicProcessor>,
    Box::new(ThumbnailProcessor::new(library.clone())),
];

for processor in processors {
    if processor.should_process(&entry) {
        processor.process(&ctx, &entry).await?;
    }
}
```

### Step 5: Test (1 hour)
Verify watcher events still work with the new architecture.

**Total**: ~9 hours to prove the concept works

## Benefits

### For Your Current Problem
Responder uses processors (DRY)
Location config controls which processors run
Easy to add OCR/subtitle processors later
UI actions just call processors directly

### For Future Features
"Regenerate Thumbnail" button → calls ThumbnailProcessor
"Extract Text (OCR)" button → calls OcrProcessor
"Generate Subtitles" button → calls SubtitleProcessor
Batch operations → job wraps processor with progress tracking

### For Code Quality
Single source of truth for each operation
Easy to test (mock ProcessorContext)
Clear separation of concerns
Composable and configurable

## Risk Mitigation

- Keep `generate_thumbnails_for_file()` as a simple wrapper around `ThumbnailProcessor`
- Don't break existing jobs during migration
- Add feature flag for processor-based responder
- Extensive testing before removing old code

## Questions to Answer

1. **Should processors mutate state or return results?** → Return results (functional, testable)
2. **Should processors emit events?** → No, caller emits (separation of concerns)
3. **How to handle processor dependencies?** → Chain them in order (content_hash before thumbnail)
4. **Should processors be stateless?** → Yes, receive all context via parameters
5. **How to handle processor failures?** → Return Result, caller decides (continue vs abort)

## Recommendation

**Start with the gradual migration (Option A)**:

1. This week: Create processor infrastructure and extract content_hash + thumbnail
2. Next week: Add location config and update responder
3. Week 3: Add RegenerateThumbnailAction for UI
4. Week 4+: Add OCR and other processors as needed

This gives you immediate value (DRY code, configurable processors) without the risk of a big refactor.

---

**Want me to start implementing Phase 1 (Create Processor Module)?**
