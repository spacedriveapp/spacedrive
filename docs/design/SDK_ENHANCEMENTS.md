# SDK Enhancements & Core Development Alignment

**Date:** October 11, 2025
**Status:** Action Items for SDK v3.0
**Source:** Grok Analysis + Codebase Research

This document outlines **concrete enhancements** to the VDFS SDK specification based on:
- Deep codebase analysis of Spacedrive Core
- Grok's critique against whitepaper and project status
- Grounding in actual implementation (87% core complete, extensions 60%)

---

## Executive Summary

**The Core Insight:**
> **Core provides primitives. Extensions provide experiences.**

Spacedrive Core handles generic data extraction (OCR, embeddings, metadata) and stores results in the Virtual Sidecar System (VSS). Extensions **consume** this pre-computed intelligence and add domain-specific behavior (Photos face detection, Chronicle research analysis, Ledger receipt parsing).

**The Goal:**
Refine the SDK spec to create optimal separation between Core responsibilities and Extension capabilities, enabling:
- Photos extension that adds faces/places without bloating core
- CRM extension that adapts the UI without photos features
- Chronicle extension that leverages core OCR without re-processing

---

## Part 1: Critical Spec Updates

### 1.1. Rename `#[app]` → `#[extension]`

**Rationale:** Avoid confusion with Spacedrive client apps (iOS/macOS at 65%). Extensions are plugins, not standalone applications.

```rust
// BEFORE
#[app(id = "com.spacedrive.chronicle")]
struct Chronicle;

// AFTER
#[extension(id = "com.spacedrive.chronicle")]
struct Chronicle;
```

**Files to update:**
- `docs/design/SDK_SPEC.md` - All occurrences
- `docs/design/SDK_SPEC_GROUNDED.md` - All occurrences

---

### 1.2. Add Core Dependency Declaration

**Rationale:** Extensions need to declare minimum core version and required features to prevent runtime failures.

```rust
#[extension(
    id = "com.spacedrive.chronicle",
    name = "Chronicle Research Assistant",
    version = "1.0.0",

    // NEW: Declare dependencies on core features
    min_core_version = "2.0.0",
    required_features = [
        "ai_models",           // Needs AI model loader
        "semantic_search",     // Needs VSS embeddings
        "ocr_sidecars",        // Needs OCR from analysis pipeline
    ],

    permissions = [
        Permission::ReadEntries(glob = "**/*.pdf"),
        Permission::ReadSidecars(kinds = ["ocr", "embeddings"]),
        Permission::WriteTags,
        Permission::UseModel(category = "llm", preference = "local"),
    ]
)]
struct Chronicle;
```

**Implementation Note:**
Core's `PluginManager` validates `min_core_version` and `required_features` before loading extension.

---

### 1.3. Extension-Triggered Analysis (On-Demand, User-Scoped)

**DECISION:** No automatic extraction hooks. Extensions define jobs that run **on-demand** on **user-scoped locations**.

**Rationale:**
- Core should NOT extract faces from every screenshot (wasteful)
- Extensions control when/where specialized analysis happens
- User decides which locations get analyzed (privacy + performance)

**The Photos Extension Pattern:**

```rust
#[model]
struct Photo {
    #[entry(filter = "*.{jpg,png,heic}")]
    file: Entry,

    // Core provides EXIF automatically (part of indexing)
    #[metadata] exif: Option<ExifData>,

    // Extension-generated sidecars (stored in VSS extensions/ folder)
    #[sidecar(kind = "faces", extension = "photos")]
    faces: Option<Vec<FaceDetection>>,

    #[sidecar(kind = "scene", extension = "photos")]
    scene_tags: Option<Vec<String>>,
}

// User-initiated job when they enable Photos on a location
#[job(trigger = "user_initiated")]
async fn analyze_location(
    ctx: &JobContext,
    location: SdPath,
) -> JobResult<()> {
    ctx.progress(Progress::indeterminate("Finding photos..."));

    // Get all images in user-selected location
    let photos = ctx.vdfs()
        .query_entries()
        .in_location(location)
        .of_type::<Image>()
        .collect()
        .await?;

    for photo in photos.progress(ctx) {
        // Skip if already analyzed
        if ctx.sidecar_exists(photo.content_uuid(), "faces")? {
            continue;
        }

        // Face detection
        let faces = ctx.ai()
            .from_registered("face_detection")  // Model registered on install
            .detect_faces(&photo)
            .await?;

        // Save detailed data to sidecar
        ctx.save_sidecar(
            photo.content_uuid(),
            "faces",
            extension_id = "photos",
            &faces
        ).await?;

        ctx.check_interrupt().await?; // Checkpoint
    }

    // Bulk generate tags from face sidecars
    ctx.run(generate_face_tags, (location,)).await?;

    Ok(())
}

// Follow-up job: Sidecar → Tags for indexing/search
#[job]
async fn generate_face_tags(
    ctx: &JobContext,
    location: SdPath,
) -> JobResult<()> {
    let photos = ctx.vdfs()
        .query_entries()
        .in_location(location)
        .with_sidecar("faces")  // Only photos with face data
        .collect()
        .await?;

    for photo in photos {
        let faces: Vec<FaceDetection> = ctx.read_sidecar(
            photo.content_uuid(),
            "faces"
        ).await?;

        // Generate tags from sidecar data
        for face in faces {
            if let Some(person_id) = face.identified_as {
                ctx.vdfs()
                    .add_tag(photo.metadata_id(), &format!("#person:{}", person_id))
                    .await?;
            }
        }
    }

    Ok(())
}
```

**The Flow:**
1. User installs Photos extension
2. User enables Photos on `/My Photos` location (scoping)
3. Photos extension dispatches `analyze_location` job
4. Job processes photos, saves detailed face data to sidecars
5. Job generates searchable tags from sidecar data
6. User can now search "#person:alice" using core tag system

**Why This Works:**
- On-demand - no wasted computation
- User-scoped - only analyzes chosen locations
- Sidecars for details - face coords, confidence scores
- Tags for indexing - searchable via core
- Versioned - re-run job on model upgrade

---

###1.4. Agent Trail for Debugging (Not Memory)

**CLARIFICATION:** Agent trail is for **tracing/debugging**, not cognitive memory.

**Rationale:**
- **Memory** = Extension-defined (Temporal/Associative/Working) for reasoning
- **Trail** = Debug logs showing agent's decision flow
- **Audit Log** = VDFS mutations only (tag writes, file moves, etc.)

```rust
#[agent]
#[agent_trail(
    level = "debug",     // Standard log level
    format = "jsonl",
    rotation = "daily",
    // Stored in: .sdlibrary/logs/extension/{id}/
)]
impl Chronicle {
    async fn on_paper_analyzed(&self, ctx: &AgentContext) -> AgentResult<()> {
        // Debug trail (for developers/troubleshooting)
        ctx.trace("Received paper analysis event");
        ctx.trace("Checking memory for similar papers");

        // Agent memory (cognitive system - extension-defined)
        let memory = ctx.memory().read().await;
        let similar = memory.papers_related_to("neural networks").await;

        ctx.trace(format!("Found {} similar papers", similar.len()));

        // VDFS mutation (goes to audit log, not trail)
        ctx.vdfs()
            .add_tag(paper_id, "#machine-learning")
            .await?;

        Ok(())
    }
}
```

**Three Separate Systems:**
- **Agent Trail:** `.sdlibrary/logs/extension/{id}/trace.jsonl` (debug only)
- **Agent Memory:** `.sdlibrary/sidecars/extension/{id}/memory/` (cognitive state)
- **VDFS Audit Log:** `.sdlibrary/audit.db` (mutations only)

---

### 1.5. Simple "Work With What's Available" Philosophy

**DECISION:** No explicit progressive query modes. Extensions just work with available data and trigger jobs for gaps.

**Rationale:**
- Keeps query API simple
- Extensions naturally get better as more data is analyzed
- If data missing, extension triggers its own analysis job

```rust
impl ChronicleMind {
    async fn papers_about(&self, topic: &str) -> Vec<PaperAnalysisEvent> {
        // Just query what exists
        self.history
            .query()
            .where_semantic("summary", similar_to(topic))
            .limit(20)
            .collect()
            .await
            .unwrap_or_default()
    }
}

// If extension needs data that doesn't exist yet
#[agent]
impl Chronicle {
    async fn ensure_paper_analyzed(&self, paper: Paper, ctx: &AgentContext) -> AgentResult<()> {
        // Check if OCR sidecar exists
        if paper.full_text.is_none() {
            // Trigger analysis job to generate it
            ctx.jobs()
                .dispatch(analyze_paper, paper)
                .await?;

            return Ok(AgentResult::Pending); // Will retry when sidecar ready
        }

        // Data available - proceed
        Ok(AgentResult::Success)
    }
}
```

**Why This Works:**
- Extensions query what's available
- Missing data → trigger job → retry later
- No complex progressive modes
- Natural async improvement over time

---

### 1.6. User-Scoped Permission Model

**KEY INSIGHT:** Extensions **request** permissions. Users **scope** them to specific locations/paths.

**The Model:**

```rust
// Extension declares broad permissions (in manifest)
#[extension(
    id = "com.spacedrive.photos",
    permissions = [
        // Extension REQUESTS these capabilities
        Permission::ReadEntries,
        Permission::ReadSidecars(kinds = ["exif"]),
        Permission::WriteSidecars(kinds = ["faces", "places"]),
        Permission::WriteTags,
        Permission::UseModel(category = "face_detection"),
    ]
)]
struct Photos;
```

**User scopes during setup:**

```
[User installs Photos extension]

Spacedrive UI:
┌─────────────────────────────────────────┐
│ Photos Extension Setup                  │
├─────────────────────────────────────────┤
│                                         │
│ This extension requests:                │
│  ✓ Read image files                    │
│  ✓ Write face detection sidecars       │
│  ✓ Add tags                             │
│  ✓ Use face detection AI model         │
│                                         │
│ Grant access to:                        │
│  [x] /My Photos                         │
│  [x] /Family Photos                     │
│  [ ] /Documents (not relevant)          │
│                                         │
│ [ Advanced: Restrict by file type ]    │
│                                         │
│  [Cancel]  [Grant Access]              │
└─────────────────────────────────────────┘
```

**Runtime Enforcement:**

```rust
// Core enforces scope on every operation
impl WasmHost {
    fn vdfs_add_tag(&self, metadata_id: Uuid, tag: &str) -> Result<()> {
        // 1. Check permission granted
        if !self.has_permission(Permission::WriteTags) {
            return Err(PermissionDenied);
        }

        // 2. Check entry is in user-scoped locations
        let entry = self.db.get_entry(metadata_id).await?;
        if !self.in_granted_scope(&entry.path()) {
            return Err(OutOfScope {
                path: entry.path(),
                granted: self.granted_scopes.clone(),
            });
        }

        // 3. Execute
        self.db.add_tag(metadata_id, tag).await
    }
}
```

**Permission Types:**

```rust
pub enum Permission {
    // Entry access (scoped by user to locations)
    ReadEntries,
    WriteEntries,
    DeleteEntries,

    // Sidecars (scoped by user to locations)
    ReadSidecars { kinds: Vec<String> },
    WriteSidecars { kinds: Vec<String> },

    // Metadata (scoped by user to locations)
    ReadTags,
    WriteTags,
    WriteCustomFields { namespace: String },

    // Jobs
    DispatchJobs,

    // AI & Models
    UseModel {
        category: String,      // "face_detection", "ocr", "llm"
        preference: ModelPreference, // Local, API, or Bundled
    },
    RegisterModel {
        category: String,
        max_memory_mb: u64,
    },

    // Network (requires explicit user consent per-call)
    AccessNetwork {
        domains: Vec<String>,
        purpose: String,       // "Download model weights"
    },
}

pub enum ModelPreference {
    LocalOnly,           // Only local models (Ollama)
    ApiAllowed,          // Can use APIs (user provides keys + grants consent)
    BundledWithExtension, // Extension ships model weights
}
```

**Key Principle:**
> Extension requests capabilities.
> User grants and scopes to specific data.
> Core enforces at runtime.

---

## Part 2: Core Development Priorities

### 2.1. AI Model Registration & Loader System (P0 - Blocks AI Extensions)

**DECISION:** Models are **registered** with Core on extension install, then accessed by name.

**Storage:** Models live in **root data dir** (not library - no sync needed)
```
~/.spacedrive/
  └── models/
      ├── face_detection/
      │   ├── photos_v1.onnx      # Registered by Photos extension
      │   └── premium_v2.onnx      # Registered by Photos (premium)
      ├── ocr/
      │   └── tesseract.onnx       # Could be registered by Core or extension
      └── llm/
          └── llama3.gguf          # Ollama-managed
```

**The Model Manager:**

```rust
// In core/src/ai/manager.rs (NEW MODULE)

pub struct ModelManager {
    registry: HashMap<String, RegisteredModel>,
    root_dir: PathBuf, // ~/.spacedrive/models/
}

impl ModelManager {
    /// Extensions register models on install
    pub async fn register_model(
        &self,
        category: &str,
        name: &str,
        source: ModelSource,
    ) -> Result<ModelId> {
        match source {
            ModelSource::Bundled(bytes) => {
                // Extension includes model in WASM
                self.save_to_disk(category, name, bytes).await?;
            }
            ModelSource::Download { url, sha256 } => {
                // Extension provides download URL
                self.download_and_verify(category, name, url, sha256).await?;
            }
            ModelSource::Ollama(model_name) => {
                // Defer to Ollama
                self.register_ollama(category, name, model_name).await?;
            }
        }

        Ok(ModelId::new(category, name))
    }

    /// Load registered model for inference
    pub async fn load(&self, model_id: &ModelId) -> Result<LoadedModel> {
        let path = self.root_dir.join(&model_id.category).join(&model_id.name);

        match model_id.category.as_str() {
            "llm" => self.load_ollama(model_id).await,
            _ => self.load_onnx(&path).await,
        }
    }
}

pub enum ModelSource {
    Bundled(Vec<u8>),                    // Included in extension
    Download { url: String, sha256: String }, // Downloaded on install
    Ollama(String),                      // Managed by Ollama
}
```

**Extension Usage:**

```rust
// On extension install (via manifest or #[on_install] hook)
#[on_install]
async fn install(ctx: &InstallContext) -> InstallResult<()> {
    // Register face detection model
    ctx.models()
        .register(
            "face_detection",
            "photos_basic",
            ModelSource::Download {
                url: "https://models.spacedrive.com/photos/faces-v1.onnx",
                sha256: "abc123...",
            }
        )
        .await?;

    Ok(())
}

// Later, in jobs
#[job]
async fn detect_faces(ctx: &JobContext, photo: Photo) -> JobResult<Vec<Face>> {
    let faces = ctx.ai()
        .from_registered("face_detection:photos_basic")  // Category:name
        .detect_faces(&photo.file)
        .await?;

    Ok(faces)
}
```

**Host Functions Needed:**

```rust
// NEW host functions
fn model_register(category_ptr, name_ptr, source_ptr) -> u32;
fn model_load(model_id_ptr) -> u32;
fn model_infer(model_id, input_ptr) -> u32;
```

**Timeline:** 3-4 weeks

---

### 2.2. Complete Extension API Surface (P0)

**Current Status:** 30% VDFS API complete

**Missing Host Functions:**

```rust
// In core/src/infra/extension/host_functions.rs (EXPAND)

#[link(wasm_import_module = "spacedrive")]
extern "C" {
    // Implemented
    fn vdfs_query_entries(filter_ptr: u32, filter_len: u32) -> u32;
    fn vdfs_read_sidecar(uuid_ptr: u32, kind_ptr: u32) -> u32;

    // ️ Partially implemented
    fn vdfs_dispatch_job(job_ptr: u32, job_len: u32) -> u32;

    // Missing - MUST IMPLEMENT
    fn vdfs_write_tag(metadata_id: u32, tag_ptr: u32, tag_len: u32) -> u32;
    fn vdfs_write_custom_field(metadata_id: u32, key_ptr: u32, value_ptr: u32) -> u32;
    fn event_subscribe(event_type_ptr: u32) -> u32;
    fn event_next(subscription_id: u32) -> u32;
    fn model_load(category_ptr: u32, name_ptr: u32) -> u32;
    fn model_infer(model_id: u32, input_ptr: u32) -> u32;
    fn job_checkpoint(job_id: u32, state_ptr: u32, state_len: u32) -> u32;
}
```

**Priority Order:**
1. `vdfs_write_tag` - Unblocks basic extension functionality
2. `event_subscribe` / `event_next` - Enables agent event handlers
3. `job_checkpoint` - Enables resumable extension jobs
4. `model_load` / `model_infer` - Enables AI extensions
5. `vdfs_write_custom_field` - Enables extension-specific metadata

**Timeline:** 3-4 weeks

---

### 2.3. VSS Extension Storage Layout (P1)

**DECISION:** Extension data in library VSS. Models in root data dir (no sync).

**Storage Layout:**

```
~/.spacedrive/                          # Root data dir
  ├── models/                           # Models (NOT in library)
  │   ├── face_detection/
  │   │   └── photos_v1.onnx
  │   ├── ocr/
  │   │   └── tesseract.onnx
  │   └── llm/
  │       └── llama3.gguf                # Ollama-managed
  │
  └── libraries/
      └── my-library.sdlibrary/
          ├── database.db                # Core database
          ├── sidecars/
          │   ├── content/{h0}/{h1}/{content_uuid}/
          │   │   ├── ocr/ocr.json       # Core-generated
          │   │   ├── thumbs/grid@2x.webp # Core-generated
          │   │   └── extensions/
          │   │       └── {extension_id}/
          │   │           ├── faces.json  # Extension sidecar
          │   │           └── receipt.json
          │   │
          │   └── extension/{extension_id}/
          │       ├── memory/
          │       │   ├── history.db      # TemporalMemory
          │       │   └── knowledge.vss   # AssociativeMemory
          │       └── state.json          # Extension state
          │
          ├── logs/
          │   └── extension/{extension_id}/
          │       └── trace.jsonl         # Agent trail (debug)
          │
          └── virtual/                    # Optional: Persisted virtual entries
              └── {extension_id}/
                  └── {uuid}.json         # Email, Note, etc.
```

**Database Schema:**

```sql
-- Extend sidecars table
ALTER TABLE sidecars ADD COLUMN extension_id TEXT;
CREATE INDEX idx_sidecars_extension ON sidecars(extension_id, content_uuid, kind);

-- Extension scope grants (user-defined)
CREATE TABLE extension_scopes (
    id INTEGER PRIMARY KEY,
    extension_id TEXT NOT NULL,
    location_id INTEGER REFERENCES locations(id),
    path_pattern TEXT,  -- For sub-path scoping
    granted_at TIMESTAMP
);
```

**Timeline:** 1-2 weeks

---

### 2.4. Memory System Foundation (P1)

**Base Traits for Memory Types:**

```rust
// In core/src/infra/memory/mod.rs (NEW MODULE)

#[async_trait]
pub trait TemporalMemory<T>: Send + Sync
where
    T: Serialize + DeserializeOwned + Clone,
{
    /// Append event to temporal log
    async fn append(&mut self, event: T) -> Result<()>;

    /// Query builder for temporal queries
    fn query(&self) -> TemporalQuery<T>;
}

#[async_trait]
pub trait AssociativeMemory<T>: Send + Sync
where
    T: Serialize + DeserializeOwned + Clone,
{
    /// Add knowledge to associative memory
    async fn add(&mut self, knowledge: T) -> Result<()>;

    /// Semantic query builder
    fn query_similar(&self, query: &str) -> AssociativeQuery<T>;
}

#[async_trait]
pub trait WorkingMemory<T>: Send + Sync
where
    T: Serialize + DeserializeOwned + Clone + Default,
{
    /// Read current state
    async fn read(&self) -> T;

    /// Transactional update
    async fn update<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(T) -> Result<T> + Send;
}
```

**Concrete Implementations:**

```rust
// TemporalMemory backed by SQLite FTS5
pub struct SqliteTemporalMemory<T> {
    db_path: PathBuf,
    _phantom: PhantomData<T>,
}

// AssociativeMemory backed by VSS Vector Repository
pub struct VssAssociativeMemory<T> {
    vss_path: PathBuf,
    embedding_model: String,
    _phantom: PhantomData<T>,
}

// WorkingMemory backed by JSON file
pub struct JsonWorkingMemory<T> {
    state_path: PathBuf,
    _phantom: PhantomData<T>,
}
```

**Timeline:** 2-3 weeks

---

### 1.5. UI Integration via Manifest (Not Rust)

**DECISION:** Use `ui_manifest.json` for UI integration, not Rust attributes.

**Rationale:**
- Keep UI separate from business logic
- Manifest can be passed directly to frontend
- No bundling React/UI code in Rust WASM
- Cleaner separation of concerns

**Extension Package Structure:**

```
photos.wasm
manifest.json           # Extension metadata
ui_manifest.json        # UI integration points
prompts/
  └── describe_photo.jinja
assets/
  └── icon.svg
```

**ui_manifest.json Example:**

```json
{
  "sidebar": {
    "section": "Photos",
    "icon": "assets/icon.svg",
    "views": [
      {
        "id": "albums",
        "title": "Albums",
        "component": "grid",
        "query": "list_albums"
      },
      {
        "id": "people",
        "title": "People",
        "component": "cluster_grid",
        "query": "list_people"
      }
    ]
  },
  "context_menu": [
    {
      "action": "create_album",
      "label": "Add to Album...",
      "icon": "plus",
      "applies_to": ["image/*"],
      "keyboard_shortcut": "cmd+shift+a"
    }
  ],
  "file_viewers": [
    {
      "mime_types": ["image/*"],
      "component": "photo_viewer",
      "supports_slideshow": true
    }
  ]
}
```

**Frontend Rendering:**

```typescript
// Frontend (React/React Native) parses ui_manifest.json
function renderExtensionSidebar(extension: Extension) {
  const uiManifest = extension.uiManifest;

  return (
    <SidebarSection title={uiManifest.sidebar.section}>
      {uiManifest.sidebar.views.map(view => (
        <ExtensionView
          key={view.id}
          component={view.component}
          data={useExtensionQuery(extension.id, view.query)}
        />
      ))}
    </SidebarSection>
  );
}
```

**Why This Works:**
- No Rust UI code
- Frontend handles rendering generically
- Extensions provide data via queries
- Can bundle custom assets
- Manifest updates don't require recompilation

---

### 1.6. Virtual Entries with Optional Persistence

**NEW CAPABILITY:** Extensions can create virtual entries (emails, notes, tasks) with optional disk persistence.

```rust
#[model]
#[persist_strategy = "user_preference"] // User decides if persisted
struct Email {
    // No #[entry] - this is a virtual entry
    // No physical file on disk (unless user enables persistence)

    #[sync(shared)] from: String,
    #[sync(shared)] to: Vec<String>,
    #[sync(shared)] subject: String,
    #[sync(shared)] body: String,
    #[sync(shared)] received_at: DateTime<Utc>,

    // Extension can optionally persist to disk
    #[persist_to = "virtual/{extension_id}/{uuid}.json"]
    persisted: bool,
}

// Extension creates virtual entry
#[job]
async fn import_emails(ctx: &JobContext, imap: ImapConfig) -> JobResult<()> {
    let emails = fetch_from_imap(&imap).await?;

    for email in emails {
        // Create virtual entry
        let email_model = Email {
            from: email.from,
            to: email.to,
            subject: email.subject,
            body: email.body,
            received_at: email.date,
            persisted: ctx.config().persist_virtual_entries,
        };

        // Save to VDFS (in database + optionally on disk)
        ctx.vdfs().create_virtual_entry(email_model).await?;
    }

    Ok(())
}
```

**User Control:**

```json
// Extension settings
{
  "persist_virtual_entries": false,  // User choice
  "backup_includes_virtual": true
}
```

**Storage:**
- **Database:** Always (for queries and sync)
- **Disk:** Optional (`.sdlibrary/virtual/{extension_id}/{uuid}.json`)
- **Benefits:** Virtual entries can sync across devices even if not persisted to disk

---

### 1.7. The Sidecar → Tags Pattern

**KEY PATTERN:** Sidecars store detailed extraction results. Tags make them searchable.

**Rationale:**
- Sidecars = Source of truth (detailed, versioned JSON)
- Tags = Index for search (lightweight, core primitive)
- Regenerate tags when sidecar model upgrades

```rust
// Step 1: Extension saves detailed data to sidecar
#[job]
async fn detect_objects(ctx: &JobContext, photo: Photo) -> JobResult<()> {
    let detections = ctx.ai()
        .from_registered("object_detection:yolo_v8")
        .detect(&photo.file)
        .await?;

    // Save detailed sidecar (coords, confidence, etc.)
    ctx.save_sidecar(
        photo.content_uuid(),
        "objects",
        extension_id = "photos",
        &ObjectDetectionResult {
            objects: detections.iter().map(|d| ObjectBox {
                class: d.class.clone(),
                confidence: d.confidence,
                bbox: d.bbox,
            }).collect(),
            model_version: "yolo_v8_v1",
        }
    ).await?;

    Ok(())
}

// Step 2: Bulk generate tags from sidecars
#[job]
async fn generate_tags_from_objects(ctx: &JobContext, location: SdPath) -> JobResult<()> {
    let photos = ctx.vdfs()
        .query_entries()
        .in_location(location)
        .with_sidecar("objects")
        .collect()
        .await?;

    for photo in photos {
        let objects: ObjectDetectionResult = ctx.read_sidecar(
            photo.content_uuid(),
            "objects"
        ).await?;

        // Generate tags from detailed sidecar
        for obj in objects.objects {
            if obj.confidence > 0.8 {
                ctx.vdfs()
                    .add_tag(photo.metadata_id(), &format!("#object:{}", obj.class))
                    .await?;
            }
        }
    }

    Ok(())
}

// On model upgrade
#[job]
async fn regenerate_tags_after_model_upgrade(
    ctx: &JobContext,
    location: SdPath,
) -> JobResult<()> {
    // Delete old sidecars
    ctx.vdfs()
        .delete_sidecars_by_model_version(
            location,
            "objects",
            old_version = "yolo_v8_v1"
        )
        .await?;

    // Re-run analysis with new model
    ctx.run(detect_objects, (location,)).await?;

    // Regenerate tags
    ctx.run(generate_tags_from_objects, (location,)).await?;

    Ok(())
}
```

**Why This Works:**
- **Sidecars preserve detail** (bounding boxes, confidence)
- **Tags enable search** ("show me all photos with dogs")
- **Versionable** (sidecar tracks model version)
- **Regenerable** (on model upgrade, redo analysis + tags)
- **Bulk operations** (tags generated efficiently in batches)

---

## Part 2: Core/Extension Boundary Clarification

### 2.1. What Core MUST Provide (The Perception Layer)

#### Generic Data Extraction (Core's Responsibility)

**DECISION:** Core only does generic extraction useful to ALL extensions.

| Capability | Core Does It? | Status | Storage |
|------------|---------------|--------|---------|
| **File Indexing** | Always | 95% | `entries` table |
| **Content Identity** | Always | 100% | `content_identity` |
| **EXIF/Media Metadata** | Always | 95% | `media_data` JSON |
| **Thumbnails** | Always | 90% | VSS `thumbs/*.webp` |
| **OCR (Documents)** | Generic documents | 70% | VSS `ocr/ocr.json` |
| **Embeddings** | For semantic search | 0% | VSS `embeddings/*.json` |
| **Object Detection** | Extension-triggered | 0% | Extension sidecar |
| **Face Detection** | Extension-triggered | 0% | Extension sidecar |
| **Transcription** | Extension-triggered | 0% | Extension sidecar |
| **Receipt Parsing** | Extension-triggered | 0% | Extension sidecar |

**The Rule:**
- **Core does:** Basic, universally useful extraction (EXIF, OCR for docs, embeddings for search)
- **Extensions do:** Specialized extraction (faces, receipts, custom analysis)

#### Infrastructure

- **Event Bus** - `Event::EntryCreated`, `Event::JobCompleted`, etc.
- **Job System** - Durable, resumable background work
- **Sync System** - HLC timestamps, CRDTs, transitive sync
- **Tags/Collections** - Generic organization primitives
- **Model Loaders** - Local (Ollama), API (OpenAI), Custom (ONNX)
- **VSS** - Sidecar storage and deduplication

### 2.2. What Extensions PROVIDE (The Experience Layer)

#### Domain-Specific Models

```rust
// Photos Extension
#[model] struct Album { ... }
#[model] struct Person { faces: Vec<Face> }
#[model] struct Place { geo: GeoLocation }

// CRM Extension
#[model] struct Contact { ... }
#[model] struct Company { ... }
#[model] struct Deal { ... }

// Chronicle Extension
#[model] struct Paper { ... }
#[model] struct ResearchProject { ... }
```

#### Specialized Agents & Memory

```rust
// Photos agent remembers faces, places, moments
#[agent_memory]
struct PhotosMind {
    faces: AssociativeMemory<FaceCluster>,
    places: AssociativeMemory<Location>,
    events: TemporalMemory<PhotoEvent>,
}

// CRM agent remembers interactions, deals
#[agent_memory]
struct CrmMind {
    contacts: AssociativeMemory<Contact>,
    interactions: TemporalMemory<Interaction>,
    pipeline: WorkingMemory<SalesPipeline>,
}
```

#### Custom Extraction (via `#[extractor]`)

- Face detection (Photos)
- Receipt parsing (Ledger)
- Contact extraction from emails (CRM)
- Citation parsing from PDFs (Chronicle)

#### UI Adaptations

- Photos: Album grid, face clusters, map view
- CRM: Contact list, pipeline board, interaction timeline
- Chronicle: Knowledge graph, reading list, gap analysis

---

## Part 3: Detailed Implementation Specifications

### 3.1. Model Manager Implementation Details

**Core Module:** `core/src/ai/manager.rs` (NEW)

**Full Implementation:**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use serde::{Serialize, Deserialize};

pub struct ModelManager {
    root_dir: PathBuf,  // ~/.spacedrive/models/
    registry: HashMap<ModelId, RegisteredModel>,
    loaded_models: HashMap<ModelId, Box<dyn LoadedModel>>,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct ModelId {
    category: String,  // "face_detection", "ocr", "llm"
    name: String,      // "photos_basic", "tesseract", "llama3"
}

struct RegisteredModel {
    id: ModelId,
    source: ModelSource,
    format: ModelFormat,
    memory_mb: u64,
    registered_by: String, // Extension ID
}

pub enum ModelSource {
    Bundled(Vec<u8>),
    Download { url: String, sha256: String },
    Ollama(String),
}

pub enum ModelFormat {
    Onnx,
    SafeTensors,
    Gguf,
    Ollama,
}

impl ModelManager {
    pub async fn register(
        &mut self,
        category: &str,
        name: &str,
        source: ModelSource,
        registered_by: &str,
    ) -> Result<ModelId> {
        let id = ModelId {
            category: category.to_string(),
            name: name.to_string(),
        };

        // Save to disk based on source
        match source {
            ModelSource::Bundled(bytes) => {
                let path = self.root_dir
                    .join(category)
                    .join(format!("{}.onnx", name));
                fs::create_dir_all(path.parent().unwrap()).await?;
                fs::write(&path, &bytes).await?;
            }
            ModelSource::Download { url, sha256 } => {
                self.download_model(&id, &url, &sha256).await?;
            }
            ModelSource::Ollama(model_name) => {
                // Verify Ollama has this model
                self.verify_ollama_model(&model_name).await?;
            }
        }

        // Register in memory
        self.registry.insert(id.clone(), RegisteredModel {
            id: id.clone(),
            source,
            format: self.detect_format(&id)?,
            memory_mb: self.estimate_memory(&id).await?,
            registered_by: registered_by.to_string(),
        });

        Ok(id)
    }

    pub async fn load(&mut self, id: &ModelId) -> Result<&dyn LoadedModel> {
        if !self.loaded_models.contains_key(id) {
            let model = match self.registry.get(id).unwrap().format {
                ModelFormat::Onnx => self.load_onnx(id).await?,
                ModelFormat::Ollama => self.load_ollama(id).await?,
                _ => return Err(anyhow!("Unsupported format")),
            };

            self.loaded_models.insert(id.clone(), model);
        }

        Ok(self.loaded_models.get(id).unwrap().as_ref())
    }
}
```

**Host Functions:**

```rust
// In core/src/infra/extension/host_functions.rs

#[no_mangle]
pub extern "C" fn model_register(
    category_ptr: u32,
    category_len: u32,
    name_ptr: u32,
    name_len: u32,
    source_ptr: u32,
    source_len: u32,
) -> u32 {
    // Deserialize from WASM memory
    // Call core.models.register()
    // Return model ID
}

#[no_mangle]
pub extern "C" fn model_infer(
    model_id_ptr: u32,
    input_ptr: u32,
    input_len: u32,
) -> u32 {
    // Load model
    // Run inference
    // Return result pointer
}
```

---

### 3.2. Context Window & Prompt Construction

**Extension-Managed:** Each extension controls how it builds prompts for AI models.

```rust
#[agent]
impl Chronicle {
    /// Extension defines how to build context from memory
    async fn build_research_context(&self, ctx: &AgentContext<ChronicleMind>) -> String {
        let memory = ctx.memory().read().await;

        // Get recent papers from temporal memory
        let recent = memory.history
            .query()
            .where_variant(ChronicleEvent::PaperAnalyzed)
            .since(Duration::days(7))
            .limit(10)
            .collect()
            .await
            .unwrap_or_default();

        // Get relevant concepts from associative memory
        let concepts = memory.knowledge
            .query_similar("current research focus")
            .top_k(5)
            .collect()
            .await
            .unwrap_or_default();

        // Build context string for LLM
        format!(
            "Recent papers: {}\nKey concepts: {}\nCurrent plan: {}",
            recent.iter().map(|p| &p.title).join(", "),
            concepts.iter().map(|c| &c.name).join(", "),
            memory.plan.read().await.priority_topics.join(", ")
        )
    }

    /// Use context in Jinja template
    #[on_query("suggest next paper")]
    async fn suggest(ctx: &AgentContext<ChronicleMind>) -> AgentResult<String> {
        let context = self.build_research_context(ctx).await;

        #[derive(Serialize)]
        struct PromptCtx { research_context: String }

        let suggestion = ctx.ai()
            .from_registered("llm:llama3")
            .prompt_template("suggest_paper.jinja")
            .render_with(&PromptCtx { research_context: context })?
            .generate_text()
            .await?;

        Ok(suggestion)
    }
}
```

**Key Point:** Extensions are responsible for:
- Querying their own memory
- Building context that fits model's window
- Managing prompt construction
- **Not Core's concern**

---

## Part 4: Whitepaper Updates Needed

### 4.1. Update Section 6 (AI Architecture)

**Add new subsection:**

```latex
\subsubsection{Multi-Agent Extension Architecture}

Spacedrive's AI layer is deliberately modular to enable specialized
intelligence without core bloat. The architecture separates:

\paragraph{Core AI Responsibilities}
\begin{itemize}
    \item \textbf{Model Loaders}: Unified interface for local (Ollama),
          API (OpenAI), and custom (ONNX) models
    \item \textbf{Index Observation}: Event bus for real-time index changes
    \item \textbf{Generic Extraction}: OCR, embeddings, object detection
    \item \textbf{Memory Primitives}: Base traits for temporal and associative memory
\end{itemize}

\paragraph{Extension AI Responsibilities}
\begin{itemize}
    \item \textbf{Specialized Agents}: Domain-specific reasoning (Photos,
          CRM, Research)
    \item \textbf{Custom Models}: Pre-trained models for specific tasks
    \item \textbf{Memory Structures}: Enum-based memories for multi-domain
          knowledge graphs
    \item \textbf{Experience Adaptation}: UI components and workflows
\end{itemize}

This enables a Photos extension to deploy a media organization agent
while a CRM extension deploys a contact management agent, both
leveraging the same VDFS primitives but providing distinct experiences.
```

### 4.2. Update Section 4.2.5 (Analysis Queueing Phase)

**Extend to mention extension extractors:**

```latex
\item \textbf{Analysis Queueing Phase}: After a file's content and
type are identified, this phase dispatches specialized jobs:

\paragraph{Core Analysis Jobs}
- \texttt{OcrJob} for document text extraction
- \texttt{EmbeddingJob} for semantic search preparation
- \texttt{ThumbnailJob} for preview generation
- \texttt{MediaAnalysisJob} for EXIF/codec metadata

\paragraph{Extension Extractor Jobs}
Extensions can register custom extractors that hook into this phase:
- Photos extension: \texttt{FaceDetectionJob}
- Ledger extension: \texttt{ReceiptParsingJob}
- Chronicle extension: \texttt{CitationExtractionJob}

Extension extractors run after core analysis and can depend on
core-generated sidecars (e.g., face detection uses object detection
results).
```

---

## Part 5: Implementation Roadmap (Prioritized)

### Week 1-2: Core API Completion (P0)
- [ ] Implement missing WASM host functions
  - [ ] `vdfs_write_tag()`
  - [ ] `vdfs_write_custom_field()`
  - [ ] `event_subscribe()` / `event_next()`
- [ ] Extend VSS schema for extension sidecars
- [ ] Update SDK_SPEC.md with `#[extension]` rename

### Week 3-4: AI Model Loader (P0)
- [ ] Design `ModelLoader` trait
- [ ] Implement local loader (Ollama integration)
- [ ] Implement API loader (OpenAI/Anthropic)
- [ ] Implement custom model loader (ONNX runtime)
- [ ] Add `Permission::UseModel` enforcement

### Week 5-6: Memory System (P1)
- [ ] Implement `TemporalMemory` trait + SQLite backend
- [ ] Implement `AssociativeMemory` trait + VSS backend
- [ ] Implement `WorkingMemory` trait + JSON backend
- [ ] Add enum support with `.where_variant()`
- [ ] Add progressive query support

### Week 7-8: Extractor System (P1)
- [ ] Design `#[extractor]` macro
- [ ] Modify indexer Analysis Queueing phase
- [ ] Extension extractor registration API
- [ ] Dependency checking for extractors
- [ ] Test with Photos face detection example

### Week 9-10: Polish & Examples (P2)
- [ ] UI integration system
- [ ] Agent trail implementation
- [ ] Complete Chronicle extension example
- [ ] Complete Photos extension example
- [ ] Documentation and tutorials

---

## Part 6: Design Decisions to Make

### Decision 1: Extension Job Persistence

**Question:** Where do extension jobs persist?

**Option A:** Shared `jobs.db` (simpler)
```sql
-- Single jobs.db with extension_id column
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    extension_id TEXT,  -- NULL for core jobs
    status TEXT,
    state BLOB
);
```

**Option B:** Separate per-extension (more isolated)
```
.sdlibrary/sidecars/extension/{id}/jobs.db
```

**Recommendation:** Option A - simpler, unified job monitoring UI.

### Decision 2: Model Packaging

**Question:** How do extensions ship custom models?

**Option A:** Bundled in WASM
```rust
// model.onnx compiled into WASM binary
ctx.ai().from_custom_model(include_bytes!("model.onnx"))
```

**Option B:** Downloaded on-demand
```rust
// Extension manifest specifies model URL
"models": [{
    "name": "face_detection",
    "url": "https://models.spacedrive.com/photos/faces-v1.onnx",
    "sha256": "abc123..."
}]
```

**Recommendation:** Support both - bundled for small models, downloaded for large.

### Decision 3: Memory Sync

**Question:** Should extension memory sync across devices?

**Option A:** Sync everything
- `TemporalMemory` syncs via CRDT log
- `AssociativeMemory` syncs via vector merge
- Heavy bandwidth, full experience

**Option B:** Device-local only
- Each device has own agent memory
- No sync overhead
- Inconsistent across devices

**Option C:** Selective sync
```rust
#[agent_memory]
struct ChronicleMind {
    #[sync(shared)] // Syncs across devices
    knowledge: AssociativeMemory<Concept>,

    #[sync(device_owned)] // Device-local
    plan: WorkingMemory<ResearchPlan>,
}
```

**Recommendation:** Option C - let developers choose per-field.

---

## Part 7: Validation Strategy

### Test Against Real Core Systems

**Create integration tests:**

```rust
// tests/extension_integration_test.rs
#[tokio::test]
async fn test_extension_reads_core_ocr() {
    let core = setup_test_core().await;
    let library = core.create_library("test").await?;

    // Core indexes a PDF
    let pdf = library.add_file("paper.pdf").await?;

    // Wait for OcrJob to complete
    wait_for_sidecar(&pdf, "ocr").await?;

    // Extension reads OCR
    let extension_ctx = create_extension_context("chronicle", &library);
    let ocr = extension_ctx.read_sidecar(pdf.content_uuid(), "ocr").await?;

    assert!(ocr.contains("machine learning"));
}
```

**Map to existing test infrastructure:**
- Leverage `core/tests/sync_integration_test.rs` (1,554 LOC passing)
- Add extension-specific scenarios

---

## Key Refined Decisions (James + Grok Synthesis)

### Confirmed Approaches

1. **On-Demand Analysis, Not Automatic**
   - No `#[extractor]` hooks that fire on every file
   - User-initiated jobs on scoped locations
   - Extensions control when/where processing happens

2. **Model Registration, Not Raw Loading**
   - `ctx.ai().from_custom_model(include_bytes!())`
   - `ctx.models().register()` on install + `ctx.ai().from_registered()`
   - Models in root dir (`~/.spacedrive/models/`), not library

3. **User-Scoped Permissions**
   - Extensions request broad capabilities
   - Users grant and scope to specific locations
   - Runtime enforcement on every operation

4. **UI via Manifest, Not Rust**
   - `#[ui_integration(...)]` attributes
   - `ui_manifest.json` parsed by frontend
   - Cleaner separation, no UI code in WASM

5. **Sidecars → Tags Pattern**
   - Sidecars store detailed results (source of truth)
   - Tags generated from sidecars (for search/indexing)
   - Regenerable on model upgrades

6. **Virtual Entries with Optional Persistence**
   - Extensions can create virtual entries (emails, notes)
   - Database always (for sync and queries)
   - Disk optional (user preference)

7. **Three Separate Logging Systems**
   - **Agent Trail:** Debug/tracing logs (`.sdlibrary/logs/extension/`)
   - **Agent Memory:** Cognitive state (`.sdlibrary/sidecars/extension/memory/`)
   - **VDFS Audit Log:** Filesystem mutations (`.sdlibrary/audit.db`)

8. **Core Does Generic, Extensions Do Specialized**
   - Core: OCR for docs, EXIF, thumbnails, embeddings for search
   - Extensions: Face detection, receipt parsing, citation extraction
   - No automatic face detection on every screenshot

### Rejected Approaches

1. ~~Automatic `#[extractor]` hooks~~ - Too automatic, wasteful
2. ~~Progressive query modes~~ - Unnecessary complexity
3. ~~UI integration in Rust~~ - Keep it in manifest
4. ~~Models in library~~ - Root dir only (no sync)
5. ~~Object/face detection in Core~~ - Extension responsibility

---

## Summary: Next Actions

**For You (Immediate):**
1. Create `SDK_ENHANCEMENTS.md` (this document)
2. Update `SDK_SPEC.md`:
   - Rename `#[app]` → `#[extension]`
   - Remove automatic `#[extractor]` - replace with on-demand jobs
   - Add user-scoped permission model
   - Add virtual entry persistence
   - Add sidecar → tags pattern
   - Add UI manifest approach
3. Update whitepaper Section 6 (multi-agent architecture)

**For Core Development (3-4 Weeks):**
1. **Week 1-2:** Complete WASM host functions (tags, events, custom fields)
2. **Week 3-4:** Build model loader system
3. **Week 5-6:** Implement memory trait foundations
4. **Week 7-8:** Add extractor hook system

**Key Principle:**
> **Core does the expensive, generic work once.
> Extensions consume and specialize.**

This keeps Core lean (87% → 100%) while enabling infinite adaptability through extensions. Photos, CRM, and Chronicle all leverage the same OCR/embeddings but create completely different experiences.

---

**Ready to implement. This is your November 2025 alpha roadmap.** 

