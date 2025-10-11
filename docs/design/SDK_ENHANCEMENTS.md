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

### 1.3. Add `#[extractor]` Primitive for Custom Extraction

**Rationale:** Extensions like Photos need to add custom extraction (face detection) without modifying Core's indexing pipeline.

```rust
#[model]
struct Photo {
    #[entry(filter = "*.{jpg,png,heic}")]
    file: Entry,

    // Core provides these automatically (from Analysis Queueing phase)
    #[sidecar(kind = "ocr")] text: Option<String>,
    #[sidecar(kind = "objects")] detected_objects: Vec<String>,

    // Extension adds custom extraction
    #[extractor(
        job = "detect_faces",
        hook = "after_content_id",  // When in pipeline to run
        priority = "low",
        requires = ["objects"]       // Run after core object detection
    )]
    faces: Vec<FaceDetection>,

    #[extractor(job = "identify_location")]
    location: Option<GeoLocation>,
}

#[job]
async fn detect_faces(ctx: &JobContext, photo: Photo) -> JobResult<Vec<FaceDetection>> {
    // Use extension's custom packaged model
    let faces = ctx.ai()
        .from_custom_model(include_bytes!("face_model.onnx"))
        .detect_faces(&photo.file)
        .await?;

    // Saved automatically to VSS:
    // .sdlibrary/sidecars/content/{uuid}/extensions/photos/faces.json
    Ok(faces)
}
```

**How It Works:**
1. Core's Indexer runs Analysis Queueing phase
2. Checks if any extensions have `#[extractor]` hooks for this file type
3. Dispatches extension's job (e.g., `detect_faces`)
4. Extension job saves to VSS under `extensions/{extension_id}/`
5. Next time Photo model is queried, `faces` field is populated from VSS

**Implementation in Core:**
```rust
// In core/src/ops/indexing/analysis_queueing.rs
async fn dispatch_analysis_jobs(&self, entry: &Entry) -> Result<()> {
    // Core's built-in analysis
    self.dispatch_ocr_job(entry).await?;
    self.dispatch_embedding_job(entry).await?;

    // NEW: Extension extractors
    let extractors = self.plugin_manager.get_extractors_for_entry(entry).await?;
    for extractor in extractors {
        self.dispatch_extension_extractor(entry, extractor).await?;
    }

    Ok(())
}
```

---

### 1.4. Separate Agent Logs from VDFS Audit Log

**Rationale:** VDFS audit log (fully implemented) tracks filesystem/index mutations. Agent reasoning/observations should be isolated to avoid pollution.

```rust
#[agent]
#[agent_trail(
    persist = true,
    max_entries = 10000,
    rotation = "daily",
    // Stored in: .sdlibrary/sidecars/extension/{id}/agent_trail/
)]
impl Chronicle {
    async fn on_paper_analyzed(&self, ctx: &AgentContext) -> AgentResult<()> {
        // Agent's reasoning goes to agent_trail, not VDFS audit log
        ctx.log_thought("Identified 3 research gaps in ML safety").await?;

        // Only when agent performs an ACTION does it hit VDFS audit log
        ctx.vdfs()
            .add_tag(paper_id, "#machine-learning")  // ← This goes to audit log
            .await?;

        Ok(())
    }
}
```

**Storage:**
- **Agent Trail:** `.sdlibrary/sidecars/extension/{id}/agent_trail/{date}.jsonl`
- **VDFS Audit Log:** `.sdlibrary/audit.db` (only for mutations)

---

### 1.5. Add Progressive Query Support

**Rationale:** Extensions should gracefully handle partial data as Core's analysis progresses.

```rust
impl ChronicleMind {
    async fn papers_about(&self, topic: &str) -> Vec<PaperAnalysisEvent> {
        self.history
            .query()
            .where_semantic("summary", similar_to(topic))
            // NEW: Progressive query support
            .progressive(ProgressiveMode::IncludePartial)
            .fallback_to_metadata() // If OCR not ready, use filename
            .collect()
            .await
            .unwrap_or_default()
    }
}

enum ProgressiveMode {
    RequireComplete,  // Only return entries with full analysis
    IncludePartial,   // Include entries with partial analysis
    MetadataOnly,     // Fall back to basic metadata
}
```

**Why This Matters:**
- User adds 1000 PDFs
- Core's OcrJob processes them over 10 minutes
- Extension can show **immediate** results (metadata-based)
- Results improve progressively as OCR completes
- **No UI blocking**, perfect async feel

---

### 1.6. Enhanced Permission Model

**Add model access permissions:**

```rust
pub enum Permission {
    // File system
    ReadEntries(glob: String),
    WriteEntries(glob: String),

    // Sidecars (VSS)
    ReadSidecars(kinds: Vec<String>),
    WriteSidecars(kinds: Vec<String>),

    // Metadata
    ReadTags,
    WriteTags,
    ReadCustomFields(namespace: String),
    WriteCustomFields(namespace: String),

    // Jobs
    DispatchJobs,
    DispatchCoreJobs(allowed: Vec<String>), // Can trigger core jobs

    // NEW: AI & Models
    UseModel {
        category: String,      // "ocr", "llm", "embedding"
        preference: String,    // "local", "api", "custom"
        max_tokens: Option<u64>,
    },
    LoadCustomModel {
        max_memory_mb: u64,
    },

    // Network
    AccessNetwork(domains: Vec<String>),

    // UI
    RegisterUIComponent,
    ModifySidebar,
}
```

---

## Part 2: Core Development Priorities

### 2.1. AI Model Loader System (P0 - Blocks AI Extensions)

**What to Build:**

```rust
// In core/src/ai/loader.rs (NEW MODULE)

pub struct ModelLoader {
    local_registry: HashMap<String, LocalModelConfig>,
    api_registry: HashMap<String, ApiModelConfig>,
    custom_models: HashMap<String, LoadedModel>,
}

impl ModelLoader {
    /// Load model from core's registry (Ollama, bundled models)
    pub async fn load_local(category: &str, name: &str) -> Result<LocalModel> {
        // Supports: "ocr:tesseract", "llm:llama3", "embedding:minilm"
    }

    /// Load API-based model (requires user API key in config)
    pub async fn load_api(category: &str, provider: &str) -> Result<ApiModel> {
        // Supports: "llm:openai:gpt-4", "embedding:openai:ada-002"
    }

    /// Load custom model from extension's packaged weights
    pub async fn load_custom(weights: &[u8], format: ModelFormat) -> Result<CustomModel> {
        // Supports: ONNX, SafeTensors
    }
}

pub enum ModelCategory {
    Ocr,
    Llm,
    Embedding,
    ObjectDetection,
    FaceDetection,
    Custom(String),
}
```

**Exposed to Extensions via:**
```rust
ctx.ai().from_core_loader("llm:local:llama3")
ctx.ai().from_custom_model(include_bytes!("model.onnx"))
```

**Timeline:** 3-4 weeks (as per project status)

---

### 2.2. Complete Extension API Surface (P0)

**Current Status:** 30% VDFS API complete

**Missing Host Functions:**

```rust
// In core/src/infra/extension/host_functions.rs (EXPAND)

#[link(wasm_import_module = "spacedrive")]
extern "C" {
    // ✅ Implemented
    fn vdfs_query_entries(filter_ptr: u32, filter_len: u32) -> u32;
    fn vdfs_read_sidecar(uuid_ptr: u32, kind_ptr: u32) -> u32;

    // ⚠️ Partially implemented
    fn vdfs_dispatch_job(job_ptr: u32, job_len: u32) -> u32;

    // ❌ Missing - MUST IMPLEMENT
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

**Extend VSS to support extension data:**

```
.sdlibrary/
  ├── sidecars/
  │   ├── content/{h0}/{h1}/{content_uuid}/
  │   │   ├── ocr/ocr.json               # Core-generated
  │   │   ├── embeddings/minilm.json     # Core-generated
  │   │   ├── thumbs/grid@2x.webp        # Core-generated
  │   │   └── extensions/
  │   │       └── {extension_id}/
  │   │           ├── faces.json         # Extension-generated (Photos)
  │   │           └── receipt.json       # Extension-generated (Ledger)
  │   │
  │   └── extension/{extension_id}/
  │       ├── memory/
  │       │   ├── history.db             # TemporalMemory (SQLite FTS5)
  │       │   ├── knowledge.vss          # AssociativeMemory (Vector Repo)
  │       │   └── plan.json              # WorkingMemory (JSON)
  │       ├── models/
  │       │   └── custom_model.onnx      # Custom packaged models
  │       ├── agent_trail/
  │       │   └── 2025-10-11.jsonl       # Agent reasoning log
  │       └── state.json                 # Extension checkpoint state
```

**Database Schema Addition:**

```sql
-- Extend sidecars table to track extension-generated sidecars
ALTER TABLE sidecars ADD COLUMN extension_id TEXT;
ALTER TABLE sidecars ADD COLUMN is_core BOOLEAN DEFAULT TRUE;

-- Index for extension sidecar queries
CREATE INDEX idx_sidecars_extension ON sidecars(extension_id, content_uuid, kind);
```

**Timeline:** 1-2 weeks (extends existing VSS at 70%)

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

### 1.5. Add UI Integration Attributes

**Rationale:** Extensions need to adapt Spacedrive's UI (sidebar, context menus) without core changes.

```rust
#[action]
#[ui_integration(
    sidebar_section = "Photos",          // Create sidebar section
    icon = "photo",
    context_menu = true,                 // Add to right-click menu
    keyboard_shortcut = "cmd+p",
    applies_to = ["image/*"]             // Only show for images
)]
async fn create_album(
    ctx: &ActionContext,
    photos: Vec<Photo>,
) -> ActionResult<ActionPreview> {
    // Action logic
}

#[query]
#[ui_integration(
    sidebar_section = "Photos",
    view_component = "AlbumGrid"         // Custom React component
)]
async fn list_albums(ctx: &QueryContext) -> QueryResult<Vec<Album>> {
    // Query logic
}
```

**Implementation:**
Extensions register UI components with Core's UI bridge (Tauri/React Native).

---

## Part 2: Core/Extension Boundary Clarification

### 2.1. What Core MUST Provide (The Perception Layer)

#### Generic Data Extraction (Available to All Extensions)

| Capability | Status | Storage | API |
|------------|--------|---------|-----|
| **File Indexing** | ✅ 95% | `entries` table | `ctx.vdfs().entries()` |
| **Content Identity** | ✅ 100% | `content_identity` table | `entry.content_uuid()` |
| **Basic Metadata** | ✅ 100% | `entries.metadata` JSON | `entry.metadata()` |
| **OCR (Documents)** | 🔄 70% | VSS `ocr/ocr.json` | `entry.sidecar("ocr")` |
| **Embeddings** | ❌ 0% | VSS `embeddings/{model}.json` | `entry.sidecar("embeddings")` |
| **Object Detection** | ❌ 0% | VSS `objects/objects.json` | `entry.sidecar("objects")` |
| **Thumbnails** | ✅ 90% | VSS `thumbs/{variant}.webp` | `entry.thumbnail()` |
| **Media Metadata** | ✅ 95% | `media_data` JSON | `entry.media_data()` |
| **Transcription** | ❌ 0% | VSS `transcript/transcript.json` | `entry.sidecar("transcript")` |

#### Infrastructure

- ✅ **Event Bus** - `Event::EntryCreated`, `Event::JobCompleted`, etc.
- ✅ **Job System** - Durable, resumable background work
- ✅ **Sync System** - HLC timestamps, CRDTs, transitive sync
- ✅ **Tags/Collections** - Generic organization primitives
- 🔄 **Model Loaders** - Local (Ollama), API (OpenAI), Custom (ONNX)
- ✅ **VSS** - Sidecar storage and deduplication

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

## Part 3: Detailed Enhancement Specifications

### 3.1. The `#[extractor]` System

**Full Specification:**

```rust
#[extractor(
    // Job to run for extraction
    job = "detect_faces",

    // When in indexing pipeline to run
    hook = "after_content_id" | "after_ocr" | "after_embedding",

    // Priority (relative to other extractors)
    priority = "low" | "normal" | "high",

    // Dependencies on core sidecars
    requires = ["ocr", "objects"],

    // File filter (only run on matching files)
    filter = "image/*",

    // Resource requirements
    requires_capability = Capability::GPU,
    max_memory_mb = 500,
)]
faces: Vec<FaceDetection>,
```

**Extractor Job Requirements:**

```rust
#[job]
#[extractor_job] // Special attribute for extractor jobs
async fn detect_faces(
    ctx: &JobContext,
    entry: Entry,        // The entry being analyzed
    deps: ExtractorDeps, // Core sidecars this extractor depends on
) -> JobResult<Vec<FaceDetection>> {
    // Access dependency sidecars
    let objects = deps.get::<Vec<String>>("objects")?;

    // Only process if objects include "person"
    if !objects.contains(&"person".to_string()) {
        return Ok(vec![]); // Skip - no faces likely
    }

    // Run face detection with custom model
    let faces = ctx.ai()
        .from_custom_model(include_bytes!("face_model.onnx"))
        .detect_faces(&entry)
        .await?;

    Ok(faces)
}
```

**Core Integration Point:**

```rust
// In core/src/ops/indexing/analysis_queueing.rs
async fn dispatch_extractors(&self, entry: &Entry) -> Result<()> {
    // Get all registered extractors that match this entry
    let extractors = self.extension_manager
        .get_extractors()
        .filter(|e| e.matches(entry))
        .sorted_by_hook_order()
        .collect::<Vec<_>>();

    for extractor in extractors {
        // Check if dependencies are ready
        if !self.check_deps_ready(entry, &extractor.requires).await? {
            continue; // Skip if OCR not ready yet
        }

        // Dispatch extractor job
        self.jobs.dispatch(
            ExtractorJobWrapper {
                extractor_id: extractor.id,
                entry_id: entry.id(),
            }
        ).await?;
    }

    Ok(())
}
```

---

### 3.2. Agent Trail System

**Separate from VDFS Audit Log:**

```rust
#[agent]
#[agent_trail(
    persist = true,
    format = "jsonl",              // Line-delimited JSON
    rotation = "daily",            // New file each day
    max_size_mb = 100,             // Auto-rotate at size limit
    retention_days = 30,           // Auto-delete old trails
)]
impl Chronicle {
    async fn some_handler(&self, ctx: &AgentContext) -> AgentResult<()> {
        // Log agent's reasoning (goes to agent trail)
        ctx.trail()
            .log_observation("Found 3 papers on ML safety")
            .log_decision("Will analyze gap in adversarial robustness")
            .log_action_proposed("Suggest reading paper X")
            .flush()
            .await?;

        // Actual mutation (goes to VDFS audit log)
        ctx.vdfs()
            .add_tag(paper_id, "#priority-read")
            .await?; // ← This creates audit log entry

        Ok(())
    }
}
```

**Storage Format:**

```jsonl
{"ts":"2025-10-11T09:00:00Z","type":"observation","msg":"Found 3 papers on ML safety"}
{"ts":"2025-10-11T09:00:01Z","type":"decision","msg":"Will analyze gap..."}
{"ts":"2025-10-11T09:00:02Z","type":"action","action_id":"tag_write_123"}
```

**Separate from:**
```sql
-- VDFS audit log in .sdlibrary/audit.db
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    timestamp TEXT,
    action_type TEXT,  -- "tag_added", "file_moved", "entry_deleted"
    entry_id BLOB,
    details TEXT,
    device_uuid BLOB
);
```

---

### 3.3. Progressive Query Implementation

```rust
pub trait TemporalMemoryQuery<T> {
    /// Enable progressive results
    fn progressive(self, mode: ProgressiveMode) -> Self;

    /// Fallback strategy when analysis incomplete
    fn fallback_to_metadata(self) -> Self;

    /// Only return fully analyzed entries
    fn require_complete(self) -> Self;
}

impl<T> TemporalMemoryQuery<T> {
    async fn collect(self) -> Result<Vec<T>> {
        match self.progressive_mode {
            ProgressiveMode::RequireComplete => {
                // Only entries with all sidecars ready
                self.filter(|e| e.analysis_complete)
                    .collect()
                    .await
            }
            ProgressiveMode::IncludePartial => {
                // Include entries with partial analysis
                // Mark which fields are available
                self.collect_with_availability()
                    .await
            }
            ProgressiveMode::MetadataOnly => {
                // Fall back to basic metadata
                self.from_entries_table_only()
                    .collect()
                    .await
            }
        }
    }
}
```

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

## Summary: Next Actions

**For You (Immediate):**
1. Create `SDK_ENHANCEMENTS.md` (this document) ✅
2. Update `SDK_SPEC.md`:
   - Rename `#[app]` → `#[extension]`
   - Add `#[extractor]` primitive
   - Add Core/Extension boundary section
   - Add progressive query examples
3. Update whitepaper Section 6 (AI multi-agent architecture)

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

**Ready to implement. This is your November 2025 alpha roadmap.** 🚀

