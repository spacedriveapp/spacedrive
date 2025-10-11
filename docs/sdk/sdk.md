**Date:** October 10, 2025
**Status:** Finalized for Implementation (Based on Core v2.0 Roadmap)
**Overview:** This is the master specification for the Virtual Distributed File System (VDFS) Extensions SDK. The SDK enables developers to build domain-specific extensions that leverage Spacedrive's local-first, AI-native architecture without bloating the core.

## Guiding Principles

The SDK follows these core tenets, grounded in Spacedrive's whitepaper and 87% complete implementation:

- **Declarative and Type-Safe:** Use Rust attributes to describe intent; the compiler ensures safety.
- **Core Primitives, Extension Experiences:** Core handles generic operations (e.g., indexing, sync, basic extraction like EXIF/OCR). Extensions add specialized behavior (e.g., face detection in Photos).
- **Asynchronous and Progressive:** Operations are job-based and on-demand; results improve as data is analyzed, without UI blocks.
- **User-Controlled Scoping:** Extensions request permissions; users grant and scope them to specific locations/paths for privacy.
- **Durability and Safety:** All mutations use transactional actions (preview-commit-verify, 100% complete). Jobs are resumable (100% complete).
- **Model-Agnostic AI:** Core provides loaders; extensions register/use models (local, API, custom).
- **WASM Sandboxing:** Extensions run in WASM for isolation, with host functions for core access (60% complete API).

Extensions are plugins installable via the Extension Store. They adapt Spacedrive (e.g., Photos adds media features; CRM adds contact views) without core changes.

## Core vs. Extension Responsibilities

### Core Provides:

**Data Infrastructure:**

- `entries` table - Files and directories (device-owned sync)
- `content_identities` table - Unique content (shared via deterministic UUID)
- `user_metadata` table - Tags, notes, custom_data (dual-scoped: entry OR content)
- `models` table - Extension models (NEW - required for extensions)
- `tags`, `collections` - Universal organization primitives

**Processing:**

- Generic extraction: EXIF from images, OCR from docs, thumbnails (70% complete)
- Indexing pipeline: 5 phases, resumable (90% complete)
- Sync: HLC timestamps, CRDTs (95% complete)
- Jobs/Actions: Durable, previewable (100% complete)

**AI Infrastructure:**

- Model loaders: Local (Ollama), API (OpenAI), Custom (ONNX)
- Model registry: Categories (ocr, llm, face_detection, etc.)
- Jinja template rendering

### Extensions Provide:

**Domain Models:**

- Content-scoped: PhotoAnalysis (attached to photos), VideoAnalysis
- Standalone: Person, Album, Place, Moment, Contact, Email, Note
- Stored in: `models` table with extension_id + model_type

**Specialized Analysis:**

- On-demand jobs (user-initiated, scoped to locations)
- Custom AI models (face detection, receipt parsing, citation extraction)
- Model → Tag generation (detailed models.data → searchable tags)

**User Experience:**

- UI via `ui_manifest.json` (sidebar, menus, views)
- Custom queries and actions
- Agent memories and reasoning

**Key Principle:**

- Core does generic, always-useful work (EXIF, thumbnails, basic OCR)
- Extensions do specialized work on user-scoped locations
- Both use same primitives (tags, collections, sync)

## 1. Extension Definition (`#[extension]`)

The entry point for your extension. Defines metadata, dependencies, and permissions.

**Syntax:**

```rust
#[extension(
    id = "com.spacedrive.photos",  // Unique reverse-DNS ID
    name = "Photos Manager",
    version = "1.0.0",
    description = "Advanced photo organization with faces and places.",
    min_core_version = "2.0.0",  // Minimum Spacedrive core version
    required_features = ["ai_models", "exif_extraction"],  // Core features needed
    permissions = [  // Requested permissions (user grants/scopes)
        Permission::ReadEntries(glob = "**/*.{jpg,png,heic}"),
        Permission::ReadSidecars(kinds = ["exif"]),
        Permission::WriteSidecars(kinds = ["faces", "places"]),
        Permission::WriteTags,
        Permission::UseModel(category = "face_detection", preference = "local"),
    ]
)]
struct Photos {
    config: PhotosConfig,  // User-configurable settings
}
```

**Behavior:**

- On install: Core validates `min_core_version` and `required_features` via `PluginManager`.
- Permissions: Requested here; user scopes during setup (e.g., limit to "/My Photos"). Core enforces on every call (e.g., `ReadEntries` fails outside scope).
- Config: Generates UI settings pane. Example:

```rust
#[derive(Serialize, Deserialize)]
struct PhotosConfig {
    #[setting(label = "Enable Face Recognition", default = true)]
    face_recognition: bool,
    #[setting(label = "Model Preference", default = "local")]
    model_selector: String,
}
```

- Installation Flow: User installs from Store; core loads WASM, registers models/jobs, prompts for scopes.

## 2. Extension Models (`#[model]`)

Extensions create models stored in the `models` table. Three scoping strategies:

### Content-Scoped Models (Attach to Photos/Videos)

```rust
/// Attached to content_identity (device-independent)
/// Same photo at different paths → one PhotoAnalysis
#[model(version = "1.0.0")]
#[scope = "content"]  // Scoped to content_identity
#[sync_strategy = "shared"]
struct PhotoAnalysis {
    id: Uuid,
    detected_faces: Vec<FaceDetection>,
    scene_tags: Vec<SceneTag>,
    identified_people: Vec<Uuid>,  // References Person models
}
```

**Storage:**

```sql
-- models table
{
  uuid: <model_uuid>,
  extension_id: "photos",
  model_type: "PhotoAnalysis",
  content_identity_uuid: <content_uuid>,  -- Scoped to content!
  data: JSON,
  metadata_id: → user_metadata  -- Can be tagged
}
```

**Key Benefit:** Survives device changes. If you remove a device, entries disappear but ContentIdentity (and PhotoAnalysis) remain.

### Standalone Models (Independent Entities)

```rust
/// Not tied to any file/content
#[model(version = "1.0.0")]
#[scope = "standalone"]
#[sync_strategy = "shared"]
struct Person {
    id: Uuid,
    name: Option<String>,
    embeddings: Vec<Vec<f32>>,
    photo_count: usize,
}

struct Album {
    name: String,
    content_ids: Vec<Uuid>,  // References content_identity UUIDs!
}
```

**Storage:**

```sql
-- models table
{
  uuid: <person_uuid>,
  extension_id: "photos",
  model_type: "Person",
  standalone: 1,  -- Not scoped to entry/content
  data: JSON,
  metadata_id: → user_metadata  -- Can be tagged with #family
}
```

### Entry-Scoped Models (Rare, Device-Specific)

```rust
#[model]
#[scope = "entry"]  // Tied to specific path
#[sync_strategy = "device_owned"]
struct LocalEditState {
    processing_state: String,
}
```

### Large Data: Blob Storage

For large data (embeddings, cached results), use `#[blob_data]` to avoid bloating queries:

```rust
#[model]
struct Person {
    id: Uuid,
    name: Option<String>,        // Inline in models.data (fast queries)
    photo_count: usize,          // Inline

    #[blob_data(compression = "zstd", lazy = true)]
    embeddings: Vec<Vec<f32>>,   // Stored in metadata_blobs table
}
```

**Storage:**

```sql
-- Lightweight data (fast queries)
models.data = '{"name":"Alice","photo_count":42}'

-- Heavy data (separate, content-addressed)
metadata_blobs { blob_hash: "abc123", blob_data: <compressed>, size_bytes: 51200 }
model_blobs { model_uuid: person_uuid, blob_key: "embeddings", blob_id: 1 }
```

**Benefits:**

- ✅ Fast queries (heavy data not loaded)
- ✅ Lazy loading (load blobs only when accessed)
- ✅ Deduplication (content-addressed by hash)
- ✅ Compression (zstd reduces embeddings 4x)

**API Methods:**

```rust
// Create content-scoped model
ctx.vdfs().create_model_for_content(content_uuid, photo_analysis).await?;

// Get content-scoped model (lightweight fields only)
let analysis = ctx.vdfs().get_model_by_content::<PhotoAnalysis>(content_uuid).await?;

// Access blob field (triggers lazy load)
let faces = analysis.detected_faces().await?;  // ← Loads from metadata_blobs

// Create standalone model
ctx.vdfs().create_model(person).await?;

// Query without loading blobs (fast!)
let people = ctx.vdfs()
    .query_models::<Person>()
    .select_inline_only()  // Don't load embeddings
    .collect()
    .await?;

// Tag content (all entries pointing to this content get the tag)
ctx.vdfs().add_tag_to_content(content_uuid, "#vacation").await?;

// Tag model
ctx.vdfs().add_tag_to_model(person_uuid, "#family").await?;
```

**Behavior:**

- **Tags:** All models have `metadata_id` → participate in tag system
- **Collections:** All models can be in collections (polymorphic reference)
- **Sync:** Content-scoped and standalone use shared sync (HLC). Entry-scoped uses device-owned.
- **Device Independence:** PhotoAnalysis attached to content survives device removal
- **Performance:** Blob separation keeps queries fast even with large data

## 3. Jobs and Tasks (`#[job]`, `#[task]`)

Durable units of work. Extensions define for on-demand analysis.

**Syntax:**

```rust
#[task(retries = 3, timeout_ms = 30000)]
async fn detect_faces(ctx: &TaskContext, entry: &Entry) -> TaskResult<Vec<FaceDetection>> {
    let image_bytes = entry.read().await?;
    let faces = ctx.ai()
        .from_registered("face_detection:photos_v1")
        .detect_faces(&image_bytes)
        .await?;
    Ok(faces)
}

#[job(parallelism = 4, trigger = "user_initiated")]
async fn analyze_photos(ctx: &JobContext, location: SdPath) -> JobResult<()> {
    // Get content UUIDs (not entry UUIDs!)
    let content_uuids = ctx.vdfs()
        .query_entries()
        .in_location(location)
        .of_type::<Image>()
        .map(|e| e.content_uuid())
        .collect()
        .await?;

    for content_uuid in content_uuids {
        // Check if already analyzed
        if ctx.vdfs().get_model_by_content::<PhotoAnalysis>(content_uuid).await.is_ok() {
            continue;
        }

        // Get an entry to read image data
        let entry = ctx.vdfs()
            .query_entries()
            .where_content_id(content_uuid)
            .on_this_device()
            .first()
            .await?;

        // Analyze
        let faces = ctx.run(detect_faces, (&entry,)).await?;
        let scenes = ctx.run(classify_scene, (&entry,)).await?;

        // Create content-scoped model
        let analysis = PhotoAnalysis { faces, scenes, ... };
        ctx.vdfs().create_model_for_content(content_uuid, analysis).await?;

        // Tag the content
        ctx.vdfs().add_tag_to_content(content_uuid, "#analyzed").await?;
    }

    Ok(())
}
```

**Behavior:**

- **Triggers:** "user_initiated", "on_event", etc. Scoped to user-granted locations
- **Persistence:** Shared `jobs.db` with extension_id (unified monitoring)
- **Checkpoints:** Auto-saved; resumable (100% core)
- **Content-Awareness:** Works with content_identity (device-independent)
- **Model → Tags:** Create PhotoAnalysis model, then generate tags for search

## 4. AI Agents and Memory (`#[agent]`, `#[agent_memory]`)

Autonomous logic for extensions.

**Syntax:**

```rust
#[agent_memory]  // Defines the "mind"
struct PhotosMind {
    #[sync(shared)]  // Syncs across devices
    knowledge: AssociativeMemory<FaceGraph>,  // e.g., person relationships

    #[sync(device_owned)]  // Local only
    plan: WorkingMemory<AnalysisPlan>,
}

#[agent]  // The agent itself
#[agent_trail(level = "debug", format = "jsonl")]  // Debug logs only
impl Photos {
    async fn on_new_photo(&self, ctx: &AgentContext, photo: Photo) -> AgentResult<()> {
        ctx.trace("New photo added - checking for faces");  // To trail (debug)

        // Use memory for reasoning
        let mind = ctx.memory();
        let similar_faces = mind.knowledge.query_similar_faces(photo.exif).await;

        // Dispatch job if needed
        if similar_faces.is_empty() {
            ctx.dispatch_job(analyze_photos, photo.location()).await?;
        }

        // Mutation to VDFS (audit log)
        ctx.vdfs().add_tag(photo.id, "#new-photo").await?;

        Ok(())
    }
}
```

**Behavior:**

- **Agent Loop:** Observe (via event hooks, e.g., "on_new_photo"), Orient (query memory), Act (dispatch jobs/actions).
- **Memory:** Extension-defined. Temporal: Time-based events; Associative: Graphs/vectors; Working: Short-term state. Backends: SQLite for temporal, VSS for associative.
- **Trail:** Debug only (e.g., "Decision: Skipping analysis"—stored in logs/extension/). Not for cognition.
- **Context Building:** Extensions define prompts/templates (Jinja) for model inputs.
- **Sync:** Selective per-field; heavy data (e.g., vectors) optional.

## 5. Model Registration and AI Integration

**Syntax:**

```rust
// On extension install/init
fn init(ctx: &ExtensionContext) {
    ctx.models().register(
        name = "face_detection",
        category = "vision",
        source = ModelSource::Download { url: "https://example.com/model.onnx", sha256: "abc123" },
    ).await?;
}

// In jobs/tasks
ctx.ai().from_registered("face_detection").generate(...).await?;
```

**Behavior:**

- **Registration:** On install; core downloads/stores in `~/.spacedrive/models/` (root, no sync).
- **Sources:** Bundled bytes, download URL, or local path.
- **Loaders:** Core handles (Ollama local, API with consent UI).
- **Preferences:** User sets (local/cloud); extensions respect.
- **Prompts:** Jinja templates in `prompts/` for separation.

## 6. Actions (`#[action]`)

User-invokable operations with preview.

**Syntax:**

```rust
#[action]
async fn organize_photos(ctx: &ActionContext, location: SdPath) -> ActionResult<ActionPreview> {
    // Simulate: Query tags/faces
    let changes = ...;  // Preview moves/tags
    Ok(ActionPreview { title: "Organize Photos", changes, reversible: true })
}

#[action_execute]
async fn organize_photos_execute(ctx: &ActionContext, preview: ActionPreview) -> ActionResult<()> {
    // Apply: Use vdfs.add_tag(), etc. (audited)
    Ok(())
}
```

**Behavior:** Preview-commit-verify (100% core). Scoped to user permissions.

## 7. UI Integration (`ui_manifest.json`)

**Syntax (JSON in extension package):**

```json
{
	"sidebar_sections": [
		{
			"id": "people",
			"label": "People",
			"icon": "assets/people_icon.png",
			"query": "tags LIKE '#person:%'", // VDFS query for data
			"render_type": "list" // Generic: list, grid, etc.
		}
	],
	"views": [
		{
			"id": "places_map",
			"label": "Places",
			"component": "map_view", // Core-provided components
			"data_source": "query:exif_gps" // Fetch via VDFS
		}
	]
}
```

**Behavior:** Frontend parses; renders generically. Extensions bundle assets (icons/CSS). No Rust UI code.

## 8. Fluent Builders (Device/AI Orchestration)

**Syntax:**

```rust
let device = ctx.select_device()
    .with_capability("gpu")
    .prefer_local()
    .select()
    .await?;
ctx.execute_on(device, || async { /* heavy compute */ }).await?;
```

**Behavior:** Leverages core networking (85% complete). Scoped to permissions.

## Required Core Schema Changes

Extensions require these new tables in Core:

```sql
-- 1. Extension models (lightweight data)
CREATE TABLE models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    extension_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    data TEXT NOT NULL,  -- Lightweight JSON only

    -- Scoping (exactly one set)
    entry_uuid BLOB REFERENCES entries(uuid),
    content_identity_uuid BLOB REFERENCES content_identities(uuid),
    standalone BOOLEAN DEFAULT 0,

    -- Metadata (for tags/collections)
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),

    -- Sync
    sync_strategy INTEGER NOT NULL,  -- 0=DeviceOwned, 1=Shared
    hlc_timestamp TEXT,
    device_uuid BLOB,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    CHECK (
        (entry_uuid IS NOT NULL AND content_identity_uuid IS NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NOT NULL AND standalone = 0) OR
        (entry_uuid IS NULL AND content_identity_uuid IS NULL AND standalone = 1)
    )
);

-- 2. Large data storage (content-addressed, deduplicated)
CREATE TABLE metadata_blobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    blob_hash TEXT UNIQUE NOT NULL,      -- BLAKE3 hash for deduplication
    blob_data BLOB NOT NULL,             -- Compressed binary data
    blob_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    compression TEXT,                    -- "zstd", "gzip", NULL
    reference_count INTEGER DEFAULT 1,
    created_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL
);

-- 3. Links models to large blobs
CREATE TABLE model_blobs (
    model_uuid BLOB NOT NULL REFERENCES models(uuid),
    blob_key TEXT NOT NULL,              -- Field name ("embeddings", "cache")
    blob_id INTEGER NOT NULL REFERENCES metadata_blobs(id),
    PRIMARY KEY (model_uuid, blob_key)
);

-- 4. Links user_metadata to large blobs (for entries/content)
CREATE TABLE user_metadata_blobs (
    metadata_id INTEGER NOT NULL REFERENCES user_metadata(id),
    blob_key TEXT NOT NULL,
    blob_id INTEGER NOT NULL REFERENCES metadata_blobs(id),
    PRIMARY KEY (metadata_id, blob_key)
);

-- 5. Extend collections to support models
ALTER TABLE collection_items ADD COLUMN model_uuid BLOB REFERENCES models(uuid);
ALTER TABLE collection_items ADD CONSTRAINT check_item_type
    CHECK (entry_uuid IS NOT NULL OR content_uuid IS NOT NULL OR model_uuid IS NOT NULL);

-- Indexes
CREATE INDEX idx_models_uuid ON models(uuid);
CREATE INDEX idx_models_extension ON models(extension_id, model_type);
CREATE INDEX idx_models_content ON models(content_identity_uuid);
CREATE INDEX idx_models_metadata ON models(metadata_id);
CREATE UNIQUE INDEX idx_metadata_blobs_hash ON metadata_blobs(blob_hash);
CREATE INDEX idx_model_blobs_model ON model_blobs(model_uuid);
```

**UserMetadata already supports content-scoping** (existing):

```sql
user_metadata {
    entry_uuid: Option<Uuid>,            -- Entry-specific tag
    content_identity_uuid: Option<Uuid>, -- Content-universal tag (all copies)
}
```

## Implementation Notes

- **WASM Hosts:** Core provides functions: `vdfs_query_entries()`, `model_create()`, `model_query()`, `add_tag_to_content()`
- **Validation:** Use existing tests (1,554 LOC sync tests) + extension scenarios
- **Migration:** Add `models` table in next schema migration (required for extensions)
- **Roadmap:** Builds to November 2025 alpha (core completion in 3-4 months)

This spec realizes Spacedrive's vision: A lean core with infinite extensibility. The Photos extension demonstrates the complete architecture.
