# VDFS SDK Specification (Grounded in Spacedrive Core)

**Version:** 3.0
**Date:** October 11, 2025
**Status:** Implementation-Ready

This specification is **grounded in the actual Spacedrive architecture**, mapping SDK concepts to real core systems.

---

## 1. Core Architecture (What Actually Exists)

### Spacedrive Core Provides:

1. **Entry System** - Files/directories in `entries` table
2. **Content Identity** - BLAKE3 CAS IDs for deduplication
3. **UserMetadata** - Tags, labels, notes (in `user_metadata`, `tags`, `metadata_tag` tables)
4. **Virtual Sidecar System (VSS)** - Stores OCR, embeddings, transcripts in `.sdlibrary/sidecars/`
5. **Job System** - Durable jobs built on `task-system` crate
6. **Event Bus** - Emits `EntryCreated`, `EntryModified`, `JobCompleted` events
7. **Indexer** - 5-phase pipeline ending in "Analysis Queueing" phase
8. **Search** - Hybrid FTS5 (temporal) + VSS embeddings (semantic)

### What Extensions Get:

Extensions run as **WASM modules** (sandboxed) and consume the **outputs of Core's perception pipeline**:

- **Pre-computed sidecars** from VSS (OCR, embeddings, transcripts)
- **Events** from the EventBus
- **Query access** to entries, tags, and semantic search
- **Job dispatch** to run background work

---

## 2. The SDK: Extensions as Data Consumers

### Core Insight

**The Core does the heavy lifting (indexing, OCR, embeddings).
Extensions consume this intelligence and add domain-specific behavior.**

This is the "Perception Layer" concept:
- Core's **Indexer Analysis Queueing Phase** → Dispatches OCR, embedding, thumbnail jobs
- Core's **VSS** → Stores results as sidecars
- Extensions → Query and act on this pre-computed intelligence

---

## 3. SDK Primitives (Grounded)

### `#[app]` - Maps to WASM Plugin Entry Point

```rust
#[app(
    id = "com.spacedrive.chronicle",
    name = "Chronicle",
    version = "1.0.0",
    permissions = [
        // Permission to read entries and their sidecars
        Permission::ReadEntries(filter = "*.pdf"),
        // Permission to read OCR sidecars from VSS
        Permission::ReadSidecars(kinds = ["ocr", "embeddings"]),
        // Permission to write tags to UserMetadata
        Permission::WriteTags,
        // Permission to dispatch jobs
        Permission::DispatchJobs,
        // Permission to use AI models
        Permission::UseAI(models = ["local"]),
    ]
)]
struct Chronicle;
```

**Maps to:**
- WASM plugin loaded by `PluginManager` (PLUG-001)
- Permissions enforced by capability-based security
- Installed to user's library

### `#[model]` - Custom Typed Wrapper Around Entry + Sidecars

```rust
#[model]
struct Paper {
    /// References an Entry in the `entries` table
    #[entry(filter = "*.pdf")]
    file: Entry,

    /// Reads OCR sidecar from VSS (.sdlibrary/sidecars/content/{uuid}/ocr/ocr.json)
    #[sidecar(kind = "ocr", variant = "default")]
    full_text: Option<String>,

    /// Reads embedding sidecar from VSS
    #[sidecar(kind = "embeddings", variant = "all-MiniLM-L6-v2")]
    embedding: Option<Vec<f32>>,

    /// Reads/writes tags to UserMetadata.tags via metadata_tag junction table
    #[user_metadata]
    tags: Vec<Tag>,

    /// Extension-specific metadata stored in UserMetadata.custom_fields JSON
    #[custom_field]
    research_notes: Option<String>,
}
```

**Maps to:**
- `Entry` from `entries` table
- Sidecars from `.sdlibrary/sidecars/` (VSS)
- Tags from `tags` table + `metadata_tag` junction
- Custom data in `UserMetadata.custom_fields` JSON blob

### `#[agent]` - Event Listener on EventBus

```rust
#[agent]
#[memory(persistent = true)]
impl Chronicle {
    #[on_startup]
    async fn initialize(ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        tracing::info!("Chronicle initialized");
        Ok(())
    }

    /// Listens to EventBus for Event::EntryCreated
    #[on_event(EntryCreated)]
    #[filter = ".extension() == 'pdf'"]
    async fn on_new_pdf(entry: Entry, ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        // Check if Core has already generated OCR sidecar
        let has_ocr = ctx.vdfs()
            .sidecar_exists(entry.content_uuid(), "ocr")
            .await?;

        if has_ocr {
            // Core already did OCR - extension just consumes it
            let paper = ctx.vdfs().get::<Paper>(entry.id()).await?;

            // Dispatch extension's own analysis job
            ctx.jobs().dispatch(analyze_paper, paper).await?;
        } else {
            // OCR not ready yet - wait for sidecar ready event
            // (Core's Analysis Queueing phase will generate it)
        }

        Ok(())
    }
}
```

**Maps to:**
- Subscribes to `EventBus` via `ctx.events.subscribe()`
- Receives `Event::EntryCreated`, `Event::JobCompleted`, etc.
- Agent memory stored in extension's own state (persisted by WASM runtime)

### `#[job]` - Implements Job + JobHandler Traits

```rust
#[job(parallelism = 4)]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    ctx.progress(Progress::indeterminate("Starting analysis"));

    // Task 1: Read OCR from VSS (already generated by Core)
    let text = ctx.task(|| async {
        paper.full_text
            .ok_or_else(|| JobError::missing_sidecar("ocr"))
    }).await?;

    ctx.check_interrupt().await?; // Checkpoint

    // Task 2: AI summarization with Jinja template
    let summary = ctx.task(|| async {
        #[derive(Serialize)]
        struct PromptCtx<'a> { title: &'a str, text: &'a str }

        ctx.ai()
            .model_preference("local_llm")
            .prompt_template("summarize.jinja") // From prompts/ directory
            .render_with(&PromptCtx {
                title: &paper.file.name(),
                text: &text,
            })?
            .generate_text()
            .await
    }).await?;

    ctx.check_interrupt().await?; // Checkpoint

    // Task 3: Save as custom tag
    ctx.task(|| async {
        ctx.vdfs()
            .add_tag(paper.file.id(), &format!("summary:{}", summary))
            .await
    }).await?;

    ctx.progress(Progress::complete("Analysis complete"));
    Ok(())
}
```

**Maps to:**
- Implements `Job` and `JobHandler` traits
- Dispatched via `JobManager::dispatch()`
- Progress via `ctx.progress()` (emits `Event::JobProgress`)
- Checkpoints via `ctx.check_interrupt()`
- Persisted in `jobs.db`

---

## 4. The Perception Layer (How Core Prepares Data)

### Core's Analysis Pipeline (Whitepaper Section 4.2.5)

After indexing a file, Core's **Analysis Queueing Phase** dispatches:

1. **ThumbnailJob** - Generates WebP thumbnails → VSS `thumbs/{variant}.webp`
2. **OcrJob** - Extracts text from PDFs/images → VSS `ocr/ocr.json`
3. **EmbeddingJob** - Generates vectors → VSS `embeddings/{model}.json`
4. **TranscriptJob** - Transcribes audio/video → VSS `transcript/transcript.json`
5. **MediaAnalysisJob** - Extracts EXIF, duration, codec → `media_data` JSON in DB

### Extensions Consume, Don't Regenerate

```rust
// ❌ WRONG: Extension shouldn't do OCR (Core already did it)
#[job]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    let text = ctx.ai().ocr_document(&paper.file).await?; // WASTEFUL!
}

// ✅ RIGHT: Extension reads pre-computed OCR from VSS
#[job]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    let text = paper.full_text
        .ok_or_else(|| JobError::missing_sidecar("ocr"))?;

    // Now do extension-specific work (summarization, topic extraction, etc.)
    let summary = analyze_text(&text).await?;
}
```

---

## 5. Agent Memory System (Grounded)

### Where Agent Memory Actually Lives

```rust
#[agent_memory]
struct ChronicleMind {
    /// TemporalMemory stored in VSS as:
    /// .sdlibrary/sidecars/extension/{app_id}/memory/history.json
    history: TemporalMemory<PaperAnalysisEvent>,

    /// AssociativeMemory stored as vector sidecar:
    /// .sdlibrary/sidecars/extension/{app_id}/memory/knowledge.vss
    knowledge: AssociativeMemory<Concept>,

    /// WorkingMemory stored in WASM instance state (ephemeral)
    /// or persisted to: .sdlibrary/sidecars/extension/{app_id}/state.json
    plan: WorkingMemory<ResearchPlan>,
}
```

### Rich Memory Query API (Using VSS Infrastructure)

```rust
impl ChronicleMind {
    /// Query temporal memory with filters
    async fn papers_about(&self, topic: &str, days: u64) -> Vec<PaperAnalysisEvent> {
        self.history
            .query()
            .since(Duration::days(days))
            // Field filtering using serde_json path queries
            .where_field("title", contains(topic))
            // Semantic filtering using VSS embeddings
            .where_semantic("summary", similar_to(topic))
            .min_similarity(0.7)
            .sort_by_relevance()
            .limit(20)
            .collect()
            .await
            .unwrap_or_default()
    }

    /// Query associative memory (semantic graph)
    async fn concepts_related_to(&self, concept_name: &str) -> Vec<Concept> {
        self.knowledge
            // Uses VSS vector search infrastructure
            .query_similar(concept_name)
            .top_k(10)
            // Traverse concept graph
            .and_related_concepts(depth = 2)
            .collect()
            .await
            .unwrap_or_default()
    }

    /// Transactional update to working memory
    async fn update_research_plan(&mut self, new_topic: String) -> Result<()> {
        self.plan.update(|mut plan| {
            plan.priority_topics.push(new_topic);
            Ok(plan) // Atomic commit
        }).await
    }
}
```

**Implementation Details:**
- `TemporalMemory` uses **SQLite FTS5** for text queries + **VSS** for semantic
- `AssociativeMemory` uses **VSS vector repositories** (same as semantic search)
- `WorkingMemory` uses **transactional updates** with rollback on error
- All persisted to `.sdlibrary/sidecars/extension/{app_id}/`

---

## 6. Real Examples (Grounded in Core)

### Example 1: Chronicle Reads Core-Generated OCR

```rust
#[agent]
impl Chronicle {
    #[on_event(EntryCreated)]
    #[filter = ".extension() == 'pdf'"]
    async fn on_new_pdf(entry: Entry, ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        // Core's indexer has already:
        // 1. Created Entry in `entries` table
        // 2. Generated CAS ID (content_uuid)
        // 3. Dispatched OcrJob → created sidecar at:
        //    .sdlibrary/sidecars/content/{h0}/{h1}/{content_uuid}/ocr/ocr.json

        // Extension waits for sidecar to be ready
        ctx.on_sidecar_ready(entry.content_uuid(), "ocr", |ocr_text| async move {
            // Now do extension-specific analysis
            let topics = extract_topics(&ocr_text).await?;

            // Write tags to UserMetadata (metadata_tag junction table)
            ctx.vdfs()
                .add_tags(entry.metadata_id(), topics)
                .await?;

            // Store analysis in extension's memory (in VSS)
            ctx.memory().write().await.history.append(PaperAnalysisEvent {
                paper_id: entry.id(),
                title: entry.name().to_string(),
                summary: ocr_text.truncate(500),
            }).await?;

            Ok(())
        }).await
    }
}
```

### Example 2: Ledger Uses Core's OCR for Receipts

```rust
#[app(id = "com.spacedrive.ledger")]
struct Ledger;

#[model]
struct Receipt {
    #[entry(filter = "*.{pdf,jpg,png}")]
    scan: Entry,

    /// Reads OCR from VSS (Core already did OCR in Analysis Queueing phase)
    #[sidecar(kind = "ocr")]
    ocr_text: Option<String>,

    /// Extension-computed fields stored in custom_fields JSON
    #[custom_field]
    merchant: Option<String>,

    #[custom_field]
    amount: Option<f64>,

    #[custom_field]
    date: Option<String>,
}

#[agent]
impl Ledger {
    #[on_sidecar_ready(kind = "ocr")]
    async fn on_ocr_ready(entry: Entry, ocr: String, ctx: &AgentContext) -> AgentResult<()> {
        // Check if OCR contains receipt patterns
        if contains_price_pattern(&ocr) {
            // Extract structured data
            let receipt_data = parse_receipt(&ocr).await?;

            // Save to Entry's custom_fields JSON in user_metadata table
            ctx.vdfs()
                .update_custom_fields(entry.metadata_id(), json!({
                    "merchant": receipt_data.merchant,
                    "amount": receipt_data.amount,
                    "date": receipt_data.date,
                }))
                .await?;

            // Add semantic tag
            ctx.vdfs()
                .add_tag(entry.metadata_id(), "#receipt")
                .await?;
        }

        Ok(())
    }
}
```

---

## 7. AI Integration (Grounded in VSS)

### Jinja Templates for Extensions

Extensions ship with `prompts/` directory:

```
chronicle.wasm
prompts/
  ├── summarize_paper.jinja
  └── extract_concepts.jinja
```

Templates rendered using VSS-stored OCR:

```rust
#[task]
async fn summarize_paper(ctx: &TaskContext, paper: &Paper) -> TaskResult<String> {
    // Paper.full_text comes from VSS sidecar (Core already did OCR)
    let text = paper.full_text
        .as_ref()
        .ok_or_else(|| TaskError::missing_sidecar("ocr"))?;

    #[derive(Serialize)]
    struct PromptCtx<'a> {
        title: &'a str,
        text: &'a str,
    }

    // Use AI with Jinja template
    let summary = ctx.ai()
        .model_preference("local_llm") // User configures in Spacedrive settings
        .prompt_template("summarize_paper.jinja")
        .render_with(&PromptCtx {
            title: &paper.file.name(),
            text,
        })?
        .generate_text()
        .await?;

    Ok(summary)
}
```

**Maps to:**
- OCR sidecar from VSS at `.sdlibrary/sidecars/content/{uuid}/ocr/ocr.json`
- AI models run locally (Ollama) or cloud (API keys in user config)
- Jinja templates bundled with WASM extension

---

## 8. Memory Queries (Using Real Search Infrastructure)

### TemporalMemory Uses FTS5 + VSS

```rust
#[agent_memory]
struct ChronicleMind {
    /// Stored at: .sdlibrary/sidecars/extension/chronicle/memory/history.db
    /// Uses SQLite FTS5 for temporal queries
    history: TemporalMemory<PaperAnalysisEvent>,

    /// Stored at: .sdlibrary/sidecars/extension/chronicle/memory/knowledge.vss
    /// Uses Vector Repository format (same as semantic search)
    knowledge: AssociativeMemory<Concept>,
}

impl ChronicleMind {
    async fn papers_about_ml(&self) -> Vec<PaperAnalysisEvent> {
        self.history
            .query()
            // Temporal filter (FTS5 on history.db)
            .since(Duration::days(30))
            // Text search (FTS5)
            .where_field("title", contains("machine learning"))
            // Semantic re-ranking (VSS vector search on embeddings)
            .where_semantic("summary", similar_to("neural networks"))
            .min_similarity(0.75)
            // Final results
            .sort_by_relevance()
            .limit(10)
            .collect()
            .await
            .unwrap_or_default()
    }
}
```

**Maps to:**
- **FTS5 virtual tables** for fast temporal queries
- **VSS vector search** for semantic similarity (uses same infrastructure as Lightning Search)
- Stored in extension's own sidecar directory

---

## 9. Permission Model (Capability-Based)

### Extensions Declare Permissions

```rust
#[app(
    id = "com.spacedrive.chronicle",
    permissions = [
        // Can read entries matching glob
        Permission::ReadEntries(glob = "**/*.pdf"),

        // Can read specific sidecar kinds from VSS
        Permission::ReadSidecars(kinds = ["ocr", "embeddings"]),

        // Can write to user_metadata.tags
        Permission::WriteTags,

        // Can write to user_metadata.custom_fields["chronicle"]
        Permission::WriteCustomFields(namespace = "chronicle"),

        // Can dispatch jobs
        Permission::DispatchJobs,

        // Can use AI models
        Permission::UseAI(models = ["local"]),
    ]
)]
struct Chronicle;
```

### Runtime Enforcement

```rust
// In Core's WASM plugin host:
impl WasmPluginHost {
    fn check_permission(&self, plugin_id: &str, permission: Permission) -> Result<()> {
        let manifest = self.load_manifest(plugin_id)?;

        if !manifest.permissions.contains(&permission) {
            return Err(PermissionDenied {
                plugin: plugin_id,
                requested: permission,
            });
        }

        Ok(())
    }
}

// Every ctx operation checks permissions:
impl VdfsContext {
    pub async fn add_tag(&self, metadata_id: Uuid, tag: &str) -> Result<()> {
        // Check permission before allowing tag write
        self.host.check_permission(&self.plugin_id, Permission::WriteTags)?;

        // Execute operation
        self.db.add_tag(metadata_id, tag).await
    }
}
```

---

## 10. Complete Grounded Example

```rust
// Extension: Chronicle Research Assistant
// Consumes: OCR from Core's VSS, Entry metadata, Tags
// Produces: Research summaries, semantic tags, analysis in agent memory

#[app(
    id = "com.spacedrive.chronicle",
    permissions = [
        Permission::ReadEntries(glob = "**/*.pdf"),
        Permission::ReadSidecars(kinds = ["ocr", "embeddings"]),
        Permission::WriteTags,
        Permission::WriteCustomFields(namespace = "chronicle"),
        Permission::UseAI(models = ["local"]),
    ]
)]
struct Chronicle;

#[model]
struct Paper {
    #[entry] file: Entry,
    #[sidecar(kind = "ocr")] full_text: Option<String>,
    #[sidecar(kind = "embeddings")] embedding: Option<Vec<f32>>,
    #[user_metadata] tags: Vec<Tag>,
}

#[agent_memory]
struct ChronicleMind {
    history: TemporalMemory<PaperAnalysisEvent>,
    knowledge: AssociativeMemory<Concept>,
}

#[agent]
impl Chronicle {
    /// Triggered when Core emits Event::EntryCreated
    #[on_event(EntryCreated)]
    #[filter = ".extension() == 'pdf'"]
    async fn on_new_pdf(entry: Entry, ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        // Wait for Core's OcrJob to complete
        ctx.on_sidecar_ready(entry.content_uuid(), "ocr", |_| async move {
            // Dispatch extension's analysis job
            let paper = ctx.vdfs().get::<Paper>(entry.id()).await?;
            ctx.jobs().dispatch(analyze_paper, paper).await
        }).await
    }

    /// User queries extension via UI
    #[on_query("papers about {topic}")]
    async fn find_papers(ctx: &AgentContext<ChronicleMind>, topic: String) -> AgentResult<Vec<Paper>> {
        let memory = ctx.memory().read().await;

        // Query agent's memory using semantic search
        let relevant_events = memory.history
            .query()
            .where_semantic("summary", similar_to(&topic))
            .top_k(10)
            .collect()
            .await?;

        // Load full Paper models from VDFS
        let mut papers = Vec::new();
        for event in relevant_events {
            if let Ok(paper) = ctx.vdfs().get::<Paper>(event.paper_id).await {
                papers.push(paper);
            }
        }

        Ok(papers)
    }
}

#[task]
async fn extract_topics(ctx: &TaskContext, text: &str) -> TaskResult<Vec<String>> {
    #[derive(Serialize)]
    struct Ctx<'a> { text: &'a str }

    ctx.ai()
        .prompt_template("extract_topics.jinja")
        .render_with(&Ctx { text })?
        .generate_json::<Vec<String>>()
        .await
}

#[job]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    // Read Core-generated OCR from VSS
    let text = paper.full_text.ok_or(JobError::missing_sidecar("ocr"))?;

    // Extension-specific analysis
    let topics = ctx.run(extract_topics, (&text,)).await?;

    // Save to UserMetadata tags
    for topic in topics {
        ctx.vdfs().add_tag(paper.file.metadata_id(), &topic).await?;
    }

    // Store in agent memory (persisted to VSS)
    ctx.memory().write().await.history.append(PaperAnalysisEvent {
        paper_id: paper.file.id(),
        title: paper.file.name().to_string(),
        summary: text.chars().take(500).collect(),
    }).await?;

    Ok(())
}
```

---

## Key Mappings Summary

| SDK Concept | Spacedrive Core Reality |
|-------------|------------------------|
| `#[app]` | WASM plugin loaded by `PluginManager` |
| `#[model]` | Wrapper around `Entry` + VSS sidecars + `UserMetadata` |
| `#[agent]` | Event listener on `EventBus`, memory in VSS |
| `#[job]` | Implements `Job` + `JobHandler`, persisted in `jobs.db` |
| `#[task]` | Unit of work within job, built on `task-system` |
| `#[sidecar]` | Reads from `.sdlibrary/sidecars/` (VSS) |
| `#[user_metadata]` | Reads/writes `user_metadata`, `tags`, `labels` tables |
| `TemporalMemory` | Uses SQLite FTS5 + VSS, stored in extension's VSS dir |
| `AssociativeMemory` | Uses VSS vector repositories (same as semantic search) |
| Permissions | Capability-based, enforced by WASM host |

---

## 11. Implementation Architecture

### WASM Boundary & Host Functions

Extensions run as WASM modules. The `PluginManager` (in `CoreContext`) provides host functions:

```rust
// In Spacedrive Core (Host side)
impl WasmPluginHost {
    fn expose_host_functions() -> Linker {
        linker.func_wrap("spacedrive", "vdfs_query_entries", |...| { ... });
        linker.func_wrap("spacedrive", "vdfs_read_sidecar", |...| { ... });
        linker.func_wrap("spacedrive", "vdfs_write_tag", |...| { ... });
        linker.func_wrap("spacedrive", "job_dispatch", |...| { ... });
        linker.func_wrap("spacedrive", "event_subscribe", |...| { ... });
    }
}
```

### Extension Context (Passed Across WASM Boundary)

```rust
// Extension sees this (via FFI)
pub struct ExtensionContext {
    library_id: Uuid,
    plugin_id: String,
    permissions: PermissionSet,
}

impl ExtensionContext {
    /// Query entries (calls host function vdfs_query_entries)
    pub async fn query_entries(&self) -> QueryBuilder<Entry> {
        // Serializes query, calls WASM import, deserializes result
    }

    /// Read sidecar (calls host function vdfs_read_sidecar)
    pub async fn read_sidecar(
        &self,
        content_uuid: Uuid,
        kind: &str,
    ) -> Result<Vec<u8>> {
        // Permission check + VSS read
    }

    /// Add tag (calls host function vdfs_write_tag)
    pub async fn add_tag(&self, metadata_id: Uuid, tag: &str) -> Result<()> {
        // Permission check + DB write to metadata_tag
    }

    /// Dispatch job (calls host function job_dispatch)
    pub async fn dispatch_job<J: Job>(&self, job: J) -> Result<JobHandle> {
        // Serializes job, calls JobManager::dispatch
    }
}
```

### Extension Memory Persistence

```rust
// Agent memory stored in library's VSS:
// .sdlibrary/sidecars/extension/{app_id}/
//   ├── memory/
//   │   ├── history.db        # TemporalMemory (SQLite FTS5)
//   │   ├── knowledge.vss      # AssociativeMemory (Vector Repository)
//   │   └── plan.json          # WorkingMemory (JSON state)
//   └── state.json             # Extension state checkpoint

#[agent_memory]
struct ChronicleMind {
    // Persisted to .sdlibrary/sidecars/extension/chronicle/memory/history.db
    history: TemporalMemory<PaperAnalysisEvent>,

    // Persisted to .sdlibrary/sidecars/extension/chronicle/memory/knowledge.vss
    knowledge: AssociativeMemory<Concept>,

    // Persisted to .sdlibrary/sidecars/extension/chronicle/memory/plan.json
    plan: WorkingMemory<ResearchPlan>,
}
```

### Job Integration (Extensions Run Jobs Through Core)

```rust
// Extension job is serialized, passed to Core, and executed by JobManager
#[job]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    // ctx.library provides access to:
    // - ctx.library.db() → Database for queries
    // - ctx.library.jobs() → Dispatch child jobs
    // - ctx.library.event_bus() → Emit custom events

    // Job is persisted in library's jobs.db
    // Progress emitted via Event::JobProgress
    // On crash, resumes from last checkpoint

    ctx.progress(Progress::simple(0.0, "Reading OCR"));

    // Read from VSS (permission-checked by host)
    let text = paper.full_text.ok_or(JobError::missing_sidecar("ocr"))?;

    ctx.check_interrupt().await?; // Checkpoint to jobs.db

    ctx.progress(Progress::simple(0.5, "Analyzing"));

    let summary = ctx.task(|| async {
        ctx.ai()
            .prompt_template("summarize.jinja")
            .render_with(&json!({ "text": text }))?
            .generate_text()
            .await
    }).await?;

    ctx.check_interrupt().await?; // Checkpoint

    // Write tag (permission-checked)
    ctx.vdfs()
        .add_tag(paper.file.metadata_id(), &format!("summary:{}", summary))
        .await?;

    Ok(())
}
```

**Maps to:**
- Job implements `Job` + `JobHandler` traits (core/src/infra/job/traits.rs)
- Dispatched via `library.jobs.dispatch()` (core/src/infra/job/manager.rs)
- Persisted in library's `jobs.db`
- Uses `task-system` for execution and checkpointing

---

## 12. Real Data Flows

### Flow 1: User Adds PDF → Extension Analyzes

```
1. User drops paper.pdf into Spacedrive
   ↓
2. Core IndexerJob runs (5 phases):
   - Discovery: Finds paper.pdf
   - Processing: Creates Entry in `entries` table
   - Aggregation: Updates parent directory stats
   - Content ID: Generates BLAKE3 CAS ID
   - Analysis Queueing: Dispatches OcrJob
   ↓
3. Core OcrJob runs:
   - Extracts text from PDF
   - Saves to VSS: .sdlibrary/sidecars/content/{uuid}/ocr/ocr.json
   - Updates `sidecars` table (status = "ready")
   - Emits Event::Custom { event_type: "SidecarReady", data: {...} }
   ↓
4. Extension Chronicle (WASM) receives event:
   - Reads OCR from VSS via host function
   - Runs AI summarization with Jinja template
   - Writes tags to metadata_tag table via host function
   - Stores analysis in extension's TemporalMemory (.sdlibrary/sidecars/extension/chronicle/memory/)
```

### Flow 2: User Queries "papers about machine learning"

```
1. User types query in UI
   ↓
2. Chronicle extension's query handler runs:

   ctx.memory().read().await.history
       .query()
       .where_semantic("summary", similar_to("machine learning"))
       .top_k(10)
       .collect()
       .await?
   ↓
3. Under the hood:
   - TemporalMemory uses FTS5 on extension's history.db
   - Semantic filtering uses VSS vector search on knowledge.vss
   - Same infrastructure as Core's Lightning Search
   ↓
4. Results returned to UI with matched papers
```

---

## 13. Security Model (Grounded in WASM Sandboxing)

### Permission Enforcement

```rust
// Extension declares in manifest
permissions = [
    Permission::ReadEntries(glob = "**/*.pdf"),
    Permission::ReadSidecars(kinds = ["ocr"]),
    Permission::WriteTags,
]

// Host enforces on every operation
impl WasmHost {
    fn vdfs_add_tag(&self, metadata_id: Uuid, tag: &str) -> Result<()> {
        // 1. Check permission
        if !self.plugin_permissions.contains(&Permission::WriteTags) {
            return Err(PermissionDenied);
        }

        // 2. Execute operation
        self.library.db
            .insert_tag(metadata_id, tag)
            .await
    }
}
```

### Resource Limits

WASM provides natural sandboxing:
- **Memory limit**: 100MB per extension (configurable)
- **CPU quota**: Interruptible via `ctx.check_interrupt()`
- **Storage quota**: Tracked in extension's VSS directory
- **No filesystem access**: Can only read through host functions
- **No network access**: Unless explicitly permitted

---

**This SDK is now grounded in Spacedrive's actual architecture, mapping every concept to real systems.**


