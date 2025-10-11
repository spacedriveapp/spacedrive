# VDFS SDK Syntax Specification & Finalized Design

**Version:** 2.0
**Date:** October 10, 2025
**Status:** Approved for Implementation

---

## Guiding Principles

This document outlines the finalized syntax for the VDFS SDK. All design decisions adhere to the following core principles:

* **Declarative:** Developers should declare *intent*, not *procedure*. The VDFS orchestrates the complex details of distributed execution.
* **Type-Safe:** Leverage the Rust compiler to prevent runtime errors. The syntax must avoid "stringly-typed" APIs in favor of verifiable, compile-time-checked constructs.
* **Developer-Centric:** The SDK must be intuitive and minimize boilerplate. It should provide clear, powerful abstractions that are a joy to use.
* **Robust:** The syntax must explicitly support the platform's core promises of durability, resumability, and security.

---

## Table of Contents

1. [Core Primitives](#core-primitives)
2. [Finalized Design Decisions](#finalized-design-decisions)
3. [Extension Declaration](#extension-declaration)
4. [Data Models](#data-models)
5. [Agents](#agents)
6. [Jobs](#jobs)
7. [Actions](#actions)
8. [Pipelines](#pipelines)
9. [Device Selection & Routing](#device-selection--routing)
10. [Cross-Extension Communication](#cross-extension-communication)
11. [Complete Extension Examples](#complete-extension-examples)

---

## Core Primitives

The foundational, macro-based primitives are affirmed and will be retained:

* `#[extension]`: For top-level extension definition and metadata
* `#[setting]`: For automatic configuration generation
* `#[data_model]`: For defining syncable data structures
* `#[agent]`: For long-running, stateful logic
* `#[job]`: For durable, resumable units of work (can compose multiple tasks)
* `#[action]`: For exposing functionality to the UI or other extensions

---

## Finalized Design Decisions

### 1. Entry Type System: **Trait-Based** ✅

**Decision:** Use trait-based type system for compile-time safety and extensibility.

```rust
// APPROVED: Trait-based (compile-time safe)
ctx.vdfs()
    .entries()
    .of_type::<Pdf>()
    .collect()
    .await?;

// DEPRECATED: String-based
.of_type("pdf")
```

### 2. Error Handling: **Custom Result Types** ✅

**Decision:** Use context-specific Result types for clearer error semantics.

```rust
// APPROVED: Custom Result types
async fn handler(ctx: &AgentContext) -> AgentResult<()>
async fn job(ctx: &JobContext) -> JobResult<()>
async fn action(ctx: &ActionContext) -> ActionResult<()>
```

### 3. Agent Lifecycle: **Dedicated Hooks** ✅

**Decision:** Support explicit lifecycle hooks for initialization and cleanup.

```rust
// APPROVED: Lifecycle hooks
#[on_startup]
async fn initialize(ctx: &AgentContext) -> AgentResult<()>

#[on_shutdown]
async fn cleanup(ctx: &AgentContext) -> AgentResult<()>
```

### 4. Versioning & Migration: **Declarative with Traits** ✅

**Decision:** Use attributes for version declaration and trait implementation for migration logic.

```rust
// APPROVED: Declarative versioning
#[data_model]
#[version = "2.0.0"]
#[migrate_from = "1.0.0"]
struct Contact {
    #[since = "2.0.0"]
    social_media: Option<HashMap<String, String>>,
}

impl Migrate<ContactV1> for Contact {
    fn migrate(old: ContactV1) -> Self { ... }
}
```

---

## Extension Declaration

```rust
#[extension(
    id = "chronicle",
    name = "Chronicle Research Assistant",
    version = "1.0.0",
    author = "your-name",
    description = "AI-powered research assistant",
    permissions = [
        Permission::ReadFiles,
        Permission::WriteFiles,
        Permission::AccessNetwork,
        Permission::UseAI,
    ]
)]
struct Chronicle {
    config: ChronicleConfig,
}

#[derive(Serialize, Deserialize)]
struct ChronicleConfig {
    #[setting(label = "Auto-analyze PDFs", default = true)]
    auto_analyze: bool,

    #[setting(label = "Max papers per project", default = 1000)]
    max_papers: usize,

    #[setting(label = "AI Provider", default = "local")]
    ai_provider: AiProvider,
}
```

---

## Data Models

### Basic Data Model with All Decorators

```rust
#[data_model]
#[sync_strategy(conflict = "union_merge", priority = "hlc")]
struct ResearchProject {
    // Entry references - device-owned, state-based sync
    #[entry(kind = "directory")]
    root_folder: Entry,

    #[entry(kind = "file", extension = "pdf")]
    papers: Vec<Entry>,

    // Shared data - HLC-ordered log sync
    #[shared]
    #[indexed]
    title: String,

    #[shared]
    #[indexed]
    topics: Vec<Tag>,

    #[shared]
    collaborators: Vec<ContactId>,

    // Sidecar data - travels with entries
    #[sidecar(file = "knowledge_graph.json")]
    knowledge_graph: KnowledgeGraph,

    #[sidecar(file = "embeddings.bin")]
    embeddings: EmbeddingIndex,

    // Agent memory - persistent across sessions, synced
    #[agent_memory]
    research_state: ResearchState,

    #[agent_memory]
    reading_history: Vec<ReadingEvent>,

    // Computed fields - derived, not stored
    #[computed]
    completion: f32,

    #[computed]
    paper_count: usize,

    // Encrypted fields - zero-knowledge
    #[encrypted]
    private_notes: String,

    #[encrypted]
    api_keys: HashMap<String, String>,

    // Metadata
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
}

// Computed field implementations
impl ResearchProject {
    fn compute_completion(&self) -> f32 {
        let read = self.reading_history.len() as f32;
        let total = self.papers.len() as f32;
        if total == 0.0 { 0.0 } else { read / total }
    }

    fn compute_paper_count(&self) -> usize {
        self.papers.len()
    }
}
```

### Specialized Data Models

```rust
// Contact model with device-specific data
#[data_model]
#[sync_strategy(conflict = "last_write_wins", priority = "device_authority")]
struct Contact {
    #[shared]
    #[indexed]
    name: String,

    #[shared]
    #[indexed]
    email: String,

    #[shared]
    phone: Option<String>,

    // This field is device-owned - each device tracks separately
    #[device_owned]
    last_contacted_from: DeviceId,

    #[device_owned]
    local_notes: String,

    #[encrypted]
    private_info: String,

    #[sidecar(file = "profile_photo.jpg")]
    photo: Option<Vec<u8>>,
}

// Receipt model for financial tracking
#[data_model]
struct Receipt {
    #[entry(kind = "file", extensions = ["pdf", "jpg", "png"])]
    scan: Entry,

    #[shared]
    merchant: String,

    #[shared]
    amount: Decimal,

    #[shared]
    currency: String,

    #[shared]
    date: NaiveDate,

    #[shared]
    #[indexed]
    category: Category,

    #[sidecar(file = "ocr.json")]
    extracted_data: OcrResult,

    #[computed]
    tax_deductible: bool,
}
```

---

## Agents

### Complete Agent with All Features

```rust
#[agent]
#[memory(persistent = true, sync = true)]
#[runs_when(device_idle = true, battery_sufficient = true)]
impl Chronicle {
    // Lifecycle hooks - runs when extension is enabled or system starts
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle agent initializing...");

        // Restore persistent state
        ctx.restore_state().await?;

        // Schedule periodic tasks
        ctx.schedule_periodic("daily_summary", Duration::hours(24)).await?;

        tracing::info!("Chronicle agent initialized");
        Ok(())
    }

    #[on_shutdown]
    async fn cleanup(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle agent shutting down...");

        // Save state before shutdown
        ctx.save_state().await?;

        Ok(())
    }

    // Event handlers - triggered by VDFS events
    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!(entry_id = %entry.id, "New paper detected");

        // Dispatch analysis job to device with GPU
        ctx.jobs()
            .dispatch(analyze_paper, entry.clone())
            .on_device_with_capability(Capability::GPU)
            .when_idle()
            .priority(Priority::Low)
            .await?;

        // Update agent memory
        ctx.memory()
            .update("papers_seen", |count: u64| count + 1)
            .await?;

        Ok(())
    }

    #[on_event(EntryModified)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_paper_updated(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        // Re-analyze if paper content changed
        let last_analyzed = ctx.memory()
            .get::<DateTime<Utc>>(&format!("last_analyzed:{}", entry.id()))
            .await?;

        if should_reanalyze(&entry, last_analyzed) {
            ctx.jobs()
                .dispatch(analyze_paper, entry)
                .await?;
        }

        Ok(())
    }

    #[on_event(EntryDeleted)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_paper_deleted(entry_id: EntryId, ctx: &AgentContext) -> AgentResult<()> {
        // Clean up related data
        ctx.vdfs()
            .delete_sidecar(entry_id, "embedding").await?;

        ctx.memory()
            .graph()
            .remove_paper(entry_id).await?;

        Ok(())
    }

    // Query handlers - respond to user requests
    #[on_query("what am I missing?")]
    #[on_query("find research gaps")]
    async fn find_gaps(ctx: &AgentContext, project: Option<String>) -> AgentResult<Response> {
        // Aggregate data from all devices (trait-based type filtering)
        let my_papers = ctx.vdfs()
            .entries()
            .of_type::<Pdf>()
            .in_project(project.unwrap_or("default"))
            .across_all_devices()
            .collect::<Vec<Entry>>()
            .await?;

        // Query AI for canonical papers in topic
        let topics = extract_topics(&my_papers)?;
        let canonical_papers = ctx.ai()
            .local_if_available()
            .fallback_to_cloud()
            .canonical_papers_for_topics(&topics)
            .await?;

        // Find the difference
        let gaps = canonical_papers
            .into_iter()
            .filter(|p| !my_papers.contains_by_title(p))
            .collect::<Vec<_>>();

        Ok(Response::Papers {
            gaps,
            message: format!("Found {} research gaps", gaps.len()),
        })
    }

    #[on_query("summarize project {project}")]
    async fn summarize_project(ctx: &AgentContext, project: String) -> AgentResult<Response> {
        let papers = ctx.vdfs()
            .entries()
            .of_type::<Pdf>()
            .in_project(&project)
            .collect::<Vec<Entry>>()
            .await?;

        let graph = ctx.memory()
            .graph()
            .for_project(&project)
            .await?;

        let summary = ctx.ai()
            .summarize_research(papers, graph)
            .await?;

        Ok(Response::Summary(summary))
    }

    // Intent handlers - natural language understanding
    #[on_intent("analyze documents")]
    #[on_intent("process my PDFs")]
    async fn analyze_intent(ctx: &IntentContext, query: NaturalLanguage) -> AgentResult<Action> {
        // Parse natural language into structured params
        let params = ctx.parse_intent(query).await?;

        let entries = ctx.vdfs()
            .entries()
            .of_type::<Pdf>()
            .since(params.time_range.start)
            .until(params.time_range.end)
            .collect::<Vec<Entry>>()
            .await?;

        // Propose action to user (preview before execute)
        Ok(Action::new("analyze_papers")
            .with_entries(entries)
            .with_description(format!("Analyze {} PDFs from {}",
                entries.len(),
                params.time_range.describe()))
            .with_preview(PreviewMode::ShowList))
    }

    // Scheduled tasks
    #[scheduled(cron = "0 9 * * *")]  // Every day at 9am
    async fn daily_summary(ctx: &AgentContext) -> AgentResult<()> {
        // Generate daily research summary
        let papers_read_yesterday = ctx.memory()
            .reading_history()
            .since(Utc::now() - Duration::days(1))
            .count()
            .await?;

        if papers_read_yesterday > 0 {
            let summary = format!("You read {} papers yesterday", papers_read_yesterday);

            ctx.notify()
                .on_active_device()
                .with_title("Daily Research Summary")
                .with_body(summary)
                .send()
                .await?;
        }

        Ok(())
    }

    // Background processing
    #[background(interval = "1h")]
    async fn update_knowledge_graph(ctx: &AgentContext) -> AgentResult<()> {
        // Periodic graph maintenance
        let graph = ctx.memory().graph().await?;

        // Find papers that need re-embedding (model updated)
        let stale_papers = graph.find_stale_embeddings().await?;

        if !stale_papers.is_empty() {
            ctx.jobs()
                .dispatch(regenerate_embeddings, stale_papers)
                .when_idle()
                .await?;
        }

        Ok(())
    }
}
```

### Minimal Agent Example

```rust
#[agent]
impl Chronicle {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle initialized");
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn analyze(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        // Dispatch job for analysis
        ctx.jobs()
            .dispatch(analyze_paper, entry)
            .await
    }

    #[on_query("what am I missing?")]
    async fn find_gaps(ctx: &AgentContext) -> AgentResult<Vec<Paper>> {
        ctx.memory()
           .graph()
           .identify_gaps()
           .rank_by_relevance()
           .await
    }
}
```

---

## Jobs

### Durable Job with Full Attributes

```rust
#[job]
#[runs_on(prefer = "gpu", fallback = "cpu")]
#[checkpoint_every(100)]
#[retry_on_failure(max_attempts = 3, backoff = "exponential")]
#[timeout(minutes = 30)]
#[requires(disk_space = "1GB", memory = "2GB")]
async fn analyze_paper(
    ctx: &JobContext,
    entry: Entry,
) -> JobResult<PaperAnalysis> {
    ctx.report_progress("Starting analysis...", 0)?;

    // Step 1: OCR (if needed)
    ctx.report_progress("Extracting text...", 10)?;
    let text = if entry.has_sidecar("ocr")? {
        entry.load_sidecar("ocr").await?
    } else {
        let ocr_result = ctx.ai()
            .local_if_available()
            .ocr_document(&entry)
            .await?;
        entry.save_sidecar("ocr", &ocr_result).await?;
        ocr_result.text
    };

    ctx.check()?;  // Checkpoint

    // Step 2: Generate embeddings
    ctx.report_progress("Generating embeddings...", 40)?;
    let embedding = ctx.ai()
        .prefer_device_with(Capability::GPU)
        .embed_text(&text)
        .await?;

    entry.save_sidecar("embedding", &embedding).await?;

    ctx.check()?;  // Checkpoint

    // Step 3: Extract entities and relationships
    ctx.report_progress("Extracting entities...", 70)?;
    let entities = ctx.ai()
        .extract_entities(&text)
        .await?;

    // Step 4: Update knowledge graph
    ctx.report_progress("Updating knowledge graph...", 90)?;
    ctx.memory()
        .graph()
        .add_paper(entry.id(), entities, embedding)
        .await?;

    ctx.report_progress("Complete!", 100)?;

    Ok(PaperAnalysis {
        entry_id: entry.id(),
        entity_count: entities.len(),
        word_count: text.split_whitespace().count(),
        topics: extract_topics_from_entities(&entities),
    })
}
```

### Batch Job with Parallel Processing

```rust
#[job]
#[runs_on(prefer = "desktop", fallback = "any")]
#[checkpoint_every(10)]
async fn process_paper_batch(
    ctx: &JobContext,
    papers: Vec<Entry>,
) -> JobResult<BatchResult> {
    let total = papers.len();
    let mut results = Vec::new();

    for (idx, paper) in papers.into_iter().progress(ctx).enumerate() {
        ctx.report_progress(
            format!("Processing paper {}/{}", idx + 1, total),
            (idx * 100 / total) as u8
        )?;

        // Process individual paper
        let result = analyze_paper(ctx, paper).await?;
        results.push(result);

        ctx.check()?;  // Checkpoint every iteration
    }

    Ok(BatchResult { results })
}

#[job]
#[runs_on(prefer = "gpu")]
#[parallel(max_concurrent = 4)]
async fn parallel_embeddings(
    ctx: &JobContext,
    entries: Vec<Entry>,
) -> JobResult<Vec<Embedding>> {
    // Process up to 4 entries in parallel
    let embeddings = entries
        .into_iter()
        .progress(ctx)
        .map_parallel(|entry| async move {
            let text = entry.read_text().await?;
            ctx.ai().embed_text(&text).await
        })
        .collect::<Result<Vec<_>>>()
        .await?;

    Ok(embeddings)
}
```

---

### Jobs with Multi-Step Task Composition

Jobs in VDFS are built on a multithreaded task system. Jobs can compose multiple **tasks** that automatically checkpoint between steps. This provides durability without requiring a separate workflow layer.

#### Complete Job with Task Composition

```rust
use vdfs_sdk::prelude::*;

#[job]
async fn analyze_paper(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    ctx.report_progress("Starting analysis...", 0)?;

    // Task 1: Extract text (automatically checkpointed after completion)
    ctx.report_progress("Extracting text...", 10)?;
    let text = ctx.task(|| async {
        let ocr_result = ctx.ai()
            .local_if_available()
            .ocr_document(&entry)
            .await?;
        entry.save_sidecar("ocr", &ocr_result).await?;
        Ok::<_, JobError>(ocr_result.text)
    }).await?;

    // Automatic checkpoint here - if job crashes, resumes from here

    // Task 2: Generate embeddings (checkpointed after completion)
    ctx.report_progress("Generating embeddings...", 50)?;
    let embedding = ctx.task(|| async {
        ctx.ai()
            .prefer_device_with(Capability::GPU)
            .embed_text(&text)
            .await
    }).await?;

    // Automatic checkpoint here

    // Task 3: Update knowledge graph (checkpointed after completion)
    ctx.report_progress("Updating knowledge graph...", 80)?;
    ctx.task(|| async {
        entry.save_sidecar("embedding", &embedding).await?;

        ctx.memory()
            .graph()
            .add_paper(entry.id(), embedding)
            .await
    }).await?;

    ctx.report_progress("Complete!", 100)?;

    // Notify user
    ctx.notify()
        .on_active_device()
        .message(format!("Analyzed: {}", entry.name()))
        .send()
        .await?;

    Ok(())
}
```

#### Manual Checkpoints Within Tasks

Developers can also manually trigger checkpoints within a task for fine-grained control:

```rust
#[job]
async fn process_large_batch(ctx: &JobContext, entries: Vec<Entry>) -> JobResult<()> {
    let total = entries.len();

    for (idx, entry) in entries.into_iter().enumerate() {
        // Process entry
        let result = analyze_entry(&entry).await?;

        // Manual checkpoint every 10 items
        if idx % 10 == 0 {
            ctx.check()?;  // Explicit checkpoint
        }

        ctx.report_progress(
            format!("Processed {}/{}", idx + 1, total),
            ((idx + 1) * 100 / total) as u8
        )?;
    }

    Ok(())
}
```

#### Conditional Task Execution

```rust
#[job]
async fn conditional_analysis(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    // Check if OCR already exists
    let text = if entry.has_sidecar("ocr")? {
        entry.load_sidecar("ocr").await?
    } else {
        // Task: Perform OCR (checkpointed after completion)
        ctx.task(|| async {
            ctx.ai().ocr_document(&entry).await
        }).await?
    };

    // Only generate embeddings for papers longer than 1000 words
    if text.split_whitespace().count() > 1000 {
        // Task: Generate embedding (checkpointed)
        let embedding = ctx.task(|| async {
            ctx.ai().embed_text(&text).await
        }).await?;

        // Task: Save embedding (checkpointed)
        ctx.task(|| async {
            entry.save_sidecar("embedding", &embedding).await
        }).await?;
    }

    Ok(())
}
```

#### Parallel Task Execution

```rust
#[job]
async fn parallel_processing(ctx: &JobContext, entries: Vec<Entry>) -> JobResult<()> {
    // Process multiple entries in parallel using task parallelism
    let results = ctx
        .tasks_parallel(entries.iter().map(|entry| {
            let entry = entry.clone();
            async move {
                // Each parallel task is independently checkpointed
                let text = entry.read_text().await?;
                ctx.ai().embed_text(&text).await
            }
        }))
        .max_concurrent(4)
        .await?;

    // Checkpoint after all parallel tasks complete

    // Continue with sequential processing
    for (entry, embedding) in entries.iter().zip(results) {
        entry.save_sidecar("embedding", &embedding).await?;
    }

    Ok(())
}
```

#### Error Handling and Recovery

```rust
#[job]
async fn robust_processing(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    // Task with fallback on error
    let text = match ctx.task(|| async {
        ctx.ai().ocr_document(&entry).await
    }).await {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!("OCR failed, using simpler extraction: {}", e);
            // Fallback task (also checkpointed)
            ctx.task(|| async {
                entry.read_text().await
            }).await?
        }
    };

    // Continue processing with extracted text
    let embedding = ctx.task(|| async {
        ctx.ai().embed_text(&text).await
    }).await?;

    entry.save_sidecar("embedding", &embedding).await?;

    Ok(())
}
```

**Key Features:**
- **Automatic Checkpointing**: Each `ctx.task()` call creates a checkpoint boundary
- **Manual Checkpoints**: Use `ctx.check()` for fine-grained control
- **Resumability**: Jobs resume from the last completed task after crashes
- **Built on Task System**: Leverages existing multithreaded infrastructure
- **No Extra Layer**: No separate workflow abstraction needed

---

## Actions

### Action with Preview

```rust
#[action]
#[preview_mode = "detailed"]
async fn organize_by_topic(
    ctx: &ActionContext,
    entries: Vec<Entry>,
) -> ActionResult<ActionPreview> {
    // Generate preview before executing
    let topics = extract_all_topics(&entries).await?;

    let preview = ActionPreview {
        title: "Organize papers by topic",
        description: format!("Will organize {} papers into {} topics",
            entries.len(),
            topics.len()),
        changes: topics.iter().map(|topic| {
            Change::CreateDirectory {
                name: topic.name.clone(),
                parent: entries[0].parent(),
            }
        }).chain(
            entries.iter().map(|entry| {
                let topic = find_primary_topic(entry, &topics);
                Change::MoveEntry {
                    entry: entry.clone(),
                    destination: topic_directory(topic),
                }
            })
        ).collect(),
        reversible: true,
    };

    Ok(preview)
}

// Execute function called after user approves preview
#[action_execute]
async fn organize_by_topic_execute(
    ctx: &ActionContext,
    preview: ActionPreview,
) -> ActionResult<ExecutionResult> {
    let mut applied = Vec::new();

    for change in preview.changes {
        match change {
            Change::CreateDirectory { name, parent } => {
                ctx.vdfs().create_directory(parent, &name).await?;
                applied.push(change);
            }
            Change::MoveEntry { entry, destination } => {
                ctx.vdfs().move_entry(&entry, destination).await?;
                applied.push(change);
            }
            _ => {}
        }
    }

    Ok(ExecutionResult {
        success: true,
        changes_applied: applied,
        message: "Papers organized by topic".to_string(),
    })
}
```

---

## Pipelines

### Declarative Multi-Device Pipeline

```rust
#[pipeline]
async fn research_pipeline(ctx: &PipelineContext) -> PipelineResult<()> {
    // Step 1: Capture on mobile (where user is)
    let voice_note = ctx
        .on_device(DeviceType::Mobile)
        .capture_voice_note()
        .await?;

    // Step 2: Transcribe on any device (lightweight)
    let transcript = ctx
        .on_device(DeviceType::Any)
        .when_available()
        .transcribe(voice_note)
        .await?;

    // Step 3: Process on desktop (where compute is)
    let analysis = ctx
        .on_device(DeviceType::Desktop)
        .when_idle()
        .generate_embeddings(transcript)
        .update_knowledge_graph()
        .await?;

    // Step 4: Archive on NAS (where storage is)
    ctx.on_device(DeviceType::NAS)
        .when_online()
        .backup_project()
        .verify_integrity()
        .await?;

    // Step 5: Notify on active device (wherever user is now)
    ctx.on_active_device()
        .notify("Research updated: 3 new gaps identified")
        .await?;

    Ok(())
}
```

### Data Pipeline

```rust
#[pipeline]
async fn process_research_pipeline(ctx: &PipelineContext) -> PipelineResult<()> {
    ctx.stream()
        // Source: Capture voice notes on mobile
        .from_device(ctx.user_device())
        .voice_notes()

        // Transform: Transcribe on any device
        .pipe_to(ctx.device_with_availability())
        .transcribe()

        // Transform: Generate embeddings on GPU device
        .pipe_to(ctx.device_with_capability(Capability::GPU))
        .generate_embeddings()
        .extract_entities()

        // Store: Update graph in memory
        .update(|data| async move {
            ctx.memory()
                .graph()
                .add_research_note(data)
                .await
        })

        // Archive: Compress and store on NAS
        .pipe_to(ctx.device(DeviceRole::Storage))
        .compress()
        .encrypt()
        .store()

        // Notify: Alert user on active device
        .notify(ctx.active_device(), "Research processed")
        .await?;

    Ok(())
}
```

### Reactive Pipeline

```rust
#[pipeline]
#[trigger(on = "EntryCreated")]
#[filter = ".of_type::<Pdf>()"]
async fn auto_process_pipeline(ctx: &PipelineContext, entry: Entry) -> PipelineResult<()> {
    // Automatically triggered when PDF is added

    entry
        .ocr()
        .await?

        .pipe(|text| async move {
            ctx.ai().embed_text(&text).await
        })

        .pipe(|embedding| async move {
            ctx.memory()
                .graph()
                .add_paper_embedding(entry.id(), embedding)
                .await
        })

        .pipe(|_| async move {
            ctx.notify()
                .message(format!("Analyzed: {}", entry.name()))
                .send()
                .await
        })
        .await?;

    Ok(())
}
```

---

## Device Selection & Routing

### Declarative Device Selection

```rust
#[job]
async fn heavy_computation(ctx: &JobContext, data: Vec<Entry>) -> JobResult<()> {
    // Explicit device selection with fallback chain
    let device = ctx
        .select_device()
        .with_capability(Capability::GPU)
        .with_capability(Capability::IntelAI)  // NPU preferred
        .min_memory("4GB")
        .prefer_local()
        .prefer_idle()
        .fallback_to_cloud(CostLimit::dollars(0.50))
        .select()
        .await?;

    // Execute on selected device
    ctx.execute_on(device, || async {
        train_model(data).await
    }).await
}
```

### Smart Routing Based on Data Locality

```rust
#[job]
async fn process_photos(ctx: &JobContext, album: Album) -> JobResult<()> {
    // Group entries by device location
    let grouped = ctx.vdfs()
        .entries()
        .in_collection(album)
        .group_by_device()
        .await?;

    // Process on each device where data lives (avoid transfer)
    for (device, entries) in grouped {
        ctx.execute_on(device, || async {
            // Process locally where photos already exist
            for entry in entries {
                generate_thumbnail(entry).await?;
            }
            Ok(())
        }).await?;
    }

    Ok(())
}
```

### Conditional Device Routing

```rust
#[job]
async fn adaptive_processing(ctx: &JobContext, task: Task) -> JobResult<()> {
    match task.size() {
        size if size < 1_000_000 => {
            // Small task: run on current device
            ctx.execute_locally(|| process_small(task)).await?;
        }
        size if size < 100_000_000 => {
            // Medium task: prefer device with GPU
            ctx.select_device()
                .prefer_capability(Capability::GPU)
                .execute(|| process_medium(task))
                .await?;
        }
        _ => {
            // Large task: require desktop or cloud
            ctx.select_device()
                .require_device_type(DeviceType::Desktop)
                .fallback_to_cloud(CostLimit::dollars(5.0))
                .execute(|| process_large(task))
                .await?;
        }
    }

    Ok(())
}
```

---

## Cross-Extension Communication

### Agent-to-Agent Communication

```rust
#[agent]
impl Chronicle {
    async fn suggest_next_reading(ctx: &AgentContext) -> AgentResult<Action> {
        // Query Ledger agent for budget
        let budget_response = ctx
            .call_agent("ledger")
            .method("get_research_budget")
            .params(json!({
                "category": "books",
                "month": "current"
            }))
            .await?;

        let budget: Decimal = budget_response.parse()?;

        // Query Atlas agent for collaborators
        let collab_response = ctx
            .call_agent("atlas")
            .method("get_project_team")
            .params(json!({
                "project": "ai-safety"
            }))
            .await?;

        let collaborators: Vec<Contact> = collab_response.parse()?;

        // Query Cipher for reading preferences (encrypted)
        let prefs_response = ctx
            .call_agent("cipher")
            .method("get_user_preferences")
            .params(json!({
                "category": "reading"
            }))
            .await?;

        let preferences: ReadingPreferences = prefs_response.parse()?;

        // Combine insights
        let suggestions = self.recommend_papers(
            budget,
            &collaborators,
            &preferences
        ).await?;

        // Propose coordinated action across extensions
        Ok(Action::composite()
            .step(
                "ledger",
                "reserve_budget",
                json!({ "amount": suggestions.total_cost() })
            )
            .step(
                "atlas",
                "notify_collaborators",
                json!({ "contacts": collaborators, "message": "New reading list" })
            )
            .step(
                "chronicle",
                "create_reading_list",
                json!({ "papers": suggestions })
            )
            .with_preview())
    }
}
```

### Event Broadcasting

```rust
#[agent]
impl Chronicle {
    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        // Process the paper
        let analysis = analyze_paper(&entry).await?;

        // Broadcast event to other extensions
        ctx.broadcast_event(
            ExtensionEvent::new("chronicle.paper_analyzed")
                .with_data(json!({
                    "entry_id": entry.id(),
                    "topics": analysis.topics,
                    "entities": analysis.entities,
                }))
        ).await?;

        // Other extensions can listen:
        // - Ledger: Extract prices/receipts if found
        // - Atlas: Extract contact information if found
        // - Cipher: Flag sensitive information

        Ok(())
    }
}

// In another extension (Ledger)
#[agent]
impl Ledger {
    #[on_extension_event("chronicle.paper_analyzed")]
    async fn check_for_receipts(event: ExtensionEvent, ctx: &AgentContext) -> AgentResult<()> {
        let entry_id: EntryId = event.data["entry_id"].parse()?;
        let entry = ctx.vdfs().get_entry(entry_id).await?;

        // Check if paper contains financial information
        if contains_prices_or_receipts(&entry)? {
            let receipt = extract_receipt_data(&entry).await?;
            ctx.save_model(receipt).await?;

            ctx.notify()
                .message(format!("Found receipt in paper: ${}", receipt.amount))
                .send()
                .await?;
        }

        Ok(())
    }
}
```

### Shared Data Models

```rust
// Defined in core VDFS
#[data_model]
#[shared_across_extensions]
struct Contact {
    #[shared] name: String,
    #[shared] email: String,
    #[shared] phone: Option<String>,
}

// Chronicle extension uses Contact
#[agent]
impl Chronicle {
    async fn extract_author_contacts(paper: Entry) -> AgentResult<Vec<Contact>> {
        let text = paper.read_text().await?;
        let authors = extract_authors(&text)?;

        // Create Contact models that Atlas can see
        authors.into_iter()
            .map(|author| Contact {
                name: author.name,
                email: author.email,
                phone: None,
            })
            .collect()
    }
}

// Atlas extension also uses Contact
#[agent]
impl Atlas {
    #[on_query("show author contacts")]
    async fn list_author_contacts(ctx: &AgentContext) -> AgentResult<Vec<Contact>> {
        // Query Contacts created by Chronicle
        ctx.vdfs()
            .models::<Contact>()
            .created_by_extension("chronicle")
            .collect()
            .await
    }
}
```

---

## Complete Extension Examples

### Example 1: Chronicle (Research Assistant)

```rust
#[extension(
    id = "chronicle",
    name = "Chronicle Research Assistant",
    version = "1.0.0",
    permissions = [Permission::ReadFiles, Permission::UseAI]
)]
struct Chronicle;

#[data_model]
struct ResearchProject {
    #[entry] papers: Vec<Entry>,
    #[sidecar] graph: KnowledgeGraph,
    #[agent_memory] state: ResearchState,
}

#[data_model]
struct Paper {
    #[entry] file: Entry,
    #[sidecar] embedding: Vec<f32>,
    #[shared] topics: Vec<String>,
    #[shared] citations: Vec<String>,
    #[computed] read_status: ReadStatus,
}

#[agent]
impl Chronicle {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle initialized");
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        // Auto-analyze new papers using job
        ctx.jobs()
            .dispatch(analyze_paper, entry)
            .on_device_with_capability(Capability::GPU)
            .when_idle()
            .await
    }

    #[on_query("what am I missing in {topic}?")]
    async fn find_gaps(ctx: &AgentContext, topic: String) -> AgentResult<Vec<String>> {
        let my_papers = ctx.memory().graph().papers_for_topic(&topic).await?;
        let canonical = ctx.ai().canonical_papers_for_topic(&topic).await?;

        Ok(canonical.difference(&my_papers))
    }
}

#[job]
async fn analyze_paper(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    // Task 1: Extract text (checkpointed after completion)
    let text = ctx.task(|| async {
        entry.ocr().await
    }).await?;

    // Task 2: Generate embeddings and extract topics (checkpointed)
    let (embedding, topics) = ctx.task(|| async {
        let embedding = ctx.ai().embed_text(&text).await?;
        let topics = ctx.ai().extract_topics(&text).await?;
        Ok::<_, JobError>((embedding, topics))
    }).await?;

    // Task 3: Save results (checkpointed)
    ctx.task(|| async {
        entry.save_sidecar("embedding", &embedding).await?;
        entry.add_tags(&topics).await?;
        ctx.memory().graph().add_paper(entry.id(), topics, embedding).await
    }).await
}
```

### Example 2: Ledger (Financial Tracker)

```rust
#[extension(
    id = "ledger",
    name = "Ledger Financial Tracker",
    version = "1.0.0",
    permissions = [Permission::ReadFiles, Permission::UseAI]
)]
struct Ledger;

#[data_model]
struct Receipt {
    #[entry] scan: Entry,
    #[shared] merchant: String,
    #[shared] amount: Decimal,
    #[shared] date: NaiveDate,
    #[shared] #[indexed] category: Category,
    #[sidecar] ocr_data: OcrResult,
    #[computed] tax_deductible: bool,
}

#[data_model]
struct Budget {
    #[shared] category: Category,
    #[shared] monthly_limit: Decimal,
    #[agent_memory] current_spending: Decimal,
    #[computed] remaining: Decimal,
}

#[agent]
impl Ledger {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Ledger initialized");
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Image>()"]
    async fn on_new_image(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        // Check if image contains a receipt
        if looks_like_receipt(&entry)? {
            ctx.jobs()
                .dispatch(extract_receipt, entry)
                .await?;
        }
        Ok(())
    }

    #[on_query("spending in {category} this month")]
    async fn monthly_spending(
        ctx: &AgentContext,
        category: Category
    ) -> AgentResult<Decimal> {
        let start_of_month = Utc::now().date_naive().with_day(1).unwrap();

        ctx.vdfs()
            .models::<Receipt>()
            .filter(|r| r.category == category && r.date >= start_of_month)
            .sum(|r| r.amount)
            .await
    }

    #[on_query("tax deductible expenses")]
    async fn tax_deductible(ctx: &AgentContext) -> AgentResult<Vec<Receipt>> {
        ctx.vdfs()
            .models::<Receipt>()
            .filter(|r| r.tax_deductible)
            .collect()
            .await
    }
}

#[job]
async fn extract_receipt(ctx: &JobContext, entry: Entry) -> JobResult<Receipt> {
    let ocr_result = ctx.ai().ocr_document(&entry).await?;
    let extracted = parse_receipt_data(&ocr_result)?;

    let receipt = Receipt {
        scan: entry,
        merchant: extracted.merchant,
        amount: extracted.amount,
        date: extracted.date,
        category: classify_merchant(&extracted.merchant),
        ocr_data: ocr_result,
        tax_deductible: false,  // Computed later
    };

    ctx.save_model(receipt).await?;

    // Check budget
    let budget = ctx.memory().budget_for_category(receipt.category).await?;
    if budget.would_exceed(receipt.amount) {
        ctx.notify()
            .warning()
            .message(format!(
                "Receipt for ${} would exceed {} budget",
                receipt.amount, receipt.category
            ))
            .send()
            .await?;
    }

    Ok(receipt)
}
```

### Example 3: Atlas (Contact Manager)

```rust
#[extension(
    id = "atlas",
    name = "Atlas Contact Manager",
    version = "1.0.0",
    permissions = [Permission::ReadFiles, Permission::AccessNetwork]
)]
struct Atlas;

#[data_model]
struct Contact {
    #[shared] #[indexed] name: String,
    #[shared] #[indexed] email: String,
    #[shared] phone: Option<String>,
    #[shared] company: Option<String>,
    #[device_owned] last_contacted: Option<DateTime<Utc>>,
    #[sidecar] photo: Option<Vec<u8>>,
    #[encrypted] private_notes: String,
    #[computed] frequency_score: f32,
}

#[data_model]
struct Interaction {
    #[shared] contact: ContactId,
    #[shared] date: DateTime<Utc>,
    #[shared] type_: InteractionType,  // email, call, meeting
    #[entry] related_files: Vec<Entry>,  // Email thread, meeting notes
    #[shared] summary: String,
}

#[agent]
impl Atlas {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Atlas initialized");
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Email>()"]
    async fn on_new_email(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        let email = parse_email(&entry).await?;

        // Find or create contact
        let contact = ctx.vdfs()
            .models::<Contact>()
            .filter(|c| c.email == email.from)
            .first_or_create(|| Contact {
                name: email.from_name,
                email: email.from,
                ..Default::default()
            })
            .await?;

        // Record interaction
        ctx.save_model(Interaction {
            contact: contact.id(),
            date: Utc::now(),
            type_: InteractionType::Email,
            related_files: vec![entry],
            summary: email.subject,
        }).await?;

        Ok(())
    }

    #[on_query("contacts I haven't talked to recently")]
    async fn stale_contacts(ctx: &AgentContext) -> AgentResult<Vec<Contact>> {
        let cutoff = Utc::now() - Duration::days(90);

        ctx.vdfs()
            .models::<Contact>()
            .filter(|c| c.last_contacted < Some(cutoff))
            .sort_by(|c| c.frequency_score)
            .limit(10)
            .collect()
            .await
    }
}
```

### Example 4: Cipher (Security & Encryption)

```rust
#[extension(
    id = "cipher",
    name = "Cipher Security Manager",
    version = "1.0.0",
    permissions = [Permission::Encryption, Permission::Biometric]
)]
struct Cipher;

#[data_model]
struct Vault {
    #[entry] folder: Entry,
    #[encrypted] key: EncryptionKey,
    #[shared] name: String,
    #[shared] requires_biometric: bool,
    #[agent_memory] unlock_history: Vec<UnlockEvent>,
}

#[data_model]
struct SecureNote {
    #[encrypted] content: String,
    #[shared] title: String,
    #[shared] tags: Vec<Tag>,
    #[encrypted] attachments: Vec<Entry>,
}

#[agent]
impl Cipher {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Cipher initialized");
        Ok(())
    }

    #[on_event(VaultUnlocked)]
    async fn on_vault_unlocked(vault_id: VaultId, ctx: &AgentContext) -> AgentResult<()> {
        // Record unlock event
        ctx.memory()
            .vault(vault_id)
            .record_unlock(Utc::now(), ctx.device_id())
            .await?;

        // Auto-lock after timeout
        ctx.schedule_delayed(
            "lock_vault",
            Duration::minutes(15),
            vault_id
        ).await?;

        Ok(())
    }

    #[on_query("scan for sensitive data")]
    async fn scan_sensitive(ctx: &AgentContext) -> AgentResult<Vec<Entry>> {
        // Scan all files for sensitive patterns (SSN, credit cards, etc.)
        let all_files = ctx.vdfs().entries().of_type::<File>().collect().await?;

        let mut sensitive = Vec::new();
        for file in all_files {
            if contains_sensitive_data(&file).await? {
                sensitive.push(file);
            }
        }

        if !sensitive.is_empty() {
            ctx.notify()
                .warning()
                .message(format!("Found {} files with sensitive data", sensitive.len()))
                .action("encrypt_files", "Encrypt Now")
                .send()
                .await?;
        }

        Ok(sensitive)
    }
}

#[action]
async fn encrypt_files(ctx: &ActionContext, files: Vec<Entry>) -> ActionResult<ActionPreview> {
    Ok(ActionPreview {
        title: "Encrypt Sensitive Files",
        description: format!("Will encrypt {} files", files.len()),
        changes: files.iter().map(|f| Change::EncryptFile {
            file: f.clone(),
            method: EncryptionMethod::AES256,
        }).collect(),
        reversible: true,
    })
}
```

---

## Complete Example: Putting It All Together

This example demonstrates the finalized syntax in a real-world scenario, showcasing all key features:

```rust
use vdfs_sdk::prelude::*;

// Define extension
#[extension(id = "chronicle")]
struct Chronicle;

// Define agent
#[agent]
struct ChronicleAgent;

#[agent]
impl ChronicleAgent {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle Research Assistant is ready.");
        Ok(())
    }

    // Trigger job when a new PDF is added (trait-based filtering!)
    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_pdf(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!(entry_id = %entry.id, "New PDF detected, starting analysis");

        // Dispatch the job for execution by the VDFS
        ctx.jobs()
            .dispatch(analyze_paper, entry)
            .on_device_with_capability(Capability::GPU)
            .when_idle()
            .await?;

        Ok(())
    }

    #[on_query("what am I missing?")]
    async fn find_gaps(ctx: &AgentContext) -> AgentResult<Vec<Paper>> {
        ctx.memory()
           .graph()
           .identify_gaps()
           .rank_by_relevance()
           .await
    }
}

// Define durable job with task composition
#[job]
async fn analyze_paper(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    ctx.report_progress("Starting analysis...", 0)?;

    // Task 1: Extract text (automatically checkpointed after completion)
    ctx.report_progress("Extracting text...", 10)?;
    let text = ctx.task(|| async {
        ctx.ai()
            .local_if_available()
            .ocr_document(&entry)
            .await
    }).await?;

    // Automatic checkpoint here - if job crashes, resumes from here

    // Task 2: Generate embeddings (checkpointed after completion)
    ctx.report_progress("Generating embeddings...", 50)?;
    let embedding = ctx.task(|| async {
        ctx.ai()
            .prefer_device_with(Capability::GPU)
            .embed_text(&text)
            .await
    }).await?;

    // Automatic checkpoint here

    // Task 3: Save and update graph (checkpointed after completion)
    ctx.report_progress("Updating knowledge graph...", 80)?;
    ctx.task(|| async {
        entry.save_sidecar("embedding", &embedding).await?;

        ctx.memory()
            .graph()
            .add_paper(entry.id(), embedding)
            .await
    }).await?;

    ctx.report_progress("Complete!", 100)?;

    // Notify user
    ctx.notify()
        .on_active_device()
        .message(format!("Analyzed: {}", entry.name()))
        .send()
        .await?;

    tracing::info!(entry_id = %entry.id, "Analysis complete");
    Ok(())
}
```

**Key Features Demonstrated:**
- **Task-Based Composition**: Jobs compose multiple tasks with automatic checkpointing
- **Custom Result Types**: `JobResult`, `AgentResult` for clear error semantics
- **Trait-Based Filtering**: `.of_type::<Pdf>()` provides compile-time safety
- **Lifecycle Hooks**: `#[on_startup]` for agent initialization
- **Durable Execution**: Jobs automatically checkpoint between tasks
- **Declarative Routing**: `on_device_with_capability()` for smart device selection
- **Built on Task System**: Leverages existing multithreaded infrastructure

---

## Implementation Roadmap

All major syntax decisions have been finalized. The remaining work involves:

### Phase 1: Core Primitives (Weeks 1-4)
- [ ] Implement `#[extension]` macro and registration system
- [ ] Implement `#[data_model]` with sync strategy decorators
- [ ] Implement `#[agent]` with lifecycle hooks (`#[on_startup]`, `#[on_shutdown]`)
- [ ] Implement `#[job]` with task composition and automatic checkpointing
- [ ] Implement `#[action]` with preview/execute pattern

### Phase 2: Context APIs (Weeks 5-8)
- [ ] `AgentContext` - agent capabilities and memory
- [ ] `JobContext` - progress reporting, task composition, and checkpointing
  - [ ] `ctx.task()` for automatic checkpoint boundaries
  - [ ] `ctx.tasks_parallel()` for parallel task execution
  - [ ] `ctx.check()` for manual checkpoints
- [ ] `ActionContext` - preview generation and execution
- [ ] `PipelineContext` - data flow and multi-device routing

### Phase 3: Type System (Weeks 9-12)
- [ ] Trait-based entry type system (`.of_type::<Pdf>()`)
- [ ] Custom Result types (`AgentResult`, `JobResult`, `ActionResult`)
- [ ] Entry type traits (`Pdf`, `Image`, `Email`, `File`, etc.)
- [ ] Device capability system (`Capability::GPU`, `Capability::IntelAI`)

### Phase 4: Device Routing (Weeks 13-16)
- [ ] Device selection builder API
- [ ] Data locality-based routing
- [ ] Capability-based routing
- [ ] Cloud fallback system with cost limits

### Phase 5: Extension Communication (Weeks 17-20)
- [ ] Agent-to-agent communication API
- [ ] Event broadcasting system
- [ ] Shared data models across extensions
- [ ] Extension permission system

### Phase 6: Testing & Documentation (Weeks 21-24)
- [ ] Testing utilities and mock contexts
- [ ] Extension template generator
- [ ] Interactive tutorials
- [ ] API documentation
- [ ] Migration guides

### Open Questions for Future Iteration

#### 1. Fine-Grained Permission System
```rust
// How granular should permissions be?
#[extension(permissions = [
    Permission::ReadFiles(glob = "*.pdf"),  // Granular
    Permission::UseAI(models = ["local"]),  // Granular
    Permission::AccessNetwork(domains = ["arxiv.org"]),  // Granular
])]

// vs simpler approach
#[extension(permissions = [
    Permission::ReadFiles,  // Broader
    Permission::UseAI,
    Permission::AccessNetwork,
])]
```

#### 2. Testing Utilities API Design
```rust
// What's the ideal testing API?
#[test]
async fn test_paper_analysis() {
    let test_env = VdfsTestEnvironment::new();
    let ctx = test_env.agent_context();
    let entry = test_env.mock_entry("paper.pdf", include_bytes!("test.pdf"));

    analyze_paper_workflow(entry.clone(), &ctx).await?;

    assert!(entry.has_sidecar("embedding")?);
    assert_eq!(test_env.events_emitted().len(), 1);
}
```

#### 3. Hot-Reload & Development Experience
- How do we support hot-reloading of extensions during development?
- How do we handle extension updates with data model migrations?
- What's the debugging experience for distributed workflows?

---

**This document represents the complete vision for the VDFS SDK syntax. It should guide implementation decisions and serve as a reference for extension developers.**

