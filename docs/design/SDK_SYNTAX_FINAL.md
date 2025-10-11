# VDFS SDK Syntax Specification

**Version:** 2.1
**Date:** October 10, 2025
**Status:** Approved for Implementation

---

## 1. Guiding Principles

* **Declarative:** Developers declare *intent*, not *procedure*
* **Type-Safe:** Leverage Rust's compiler to prevent runtime errors
* **Developer-Centric:** Intuitive APIs that minimize boilerplate
* **Robust:** Built-in support for durability, resumability, and security

---

## 2. Core Primitives

* `#[app]` - Top-level extension definition and metadata
* `#[model]` - Syncable data structures with declarative strategies
* `#[agent]` - Long-running, stateful logic with lifecycle hooks
* `#[job]` - Durable, resumable units of work
* `#[task]` - A unit of work to be performed by a job
* `#[action]` - User-facing operations with preview/execute pattern

---

## 3. Finalized Design Decisions

### 3.1. Entry Type System: **Trait-Based**

```rust
ctx.vdfs()
    .entries()
    .of_type::<Pdf>()
    .collect()
    .await?;
```

### 3.2. Error Handling: **Custom Result Types**

```rust
async fn handler(ctx: &AgentContext) -> AgentResult<()>
async fn job(ctx: &JobContext) -> JobResult<()>
async fn action(ctx: &ActionContext) -> ActionResult<()>
```

### 3.3. Agent Lifecycle: **Dedicated Hooks**

```rust
#[on_startup]
async fn initialize(ctx: &AgentContext) -> AgentResult<()>

#[on_shutdown]
async fn cleanup(ctx: &AgentContext) -> AgentResult<()>
```

### 3.4. AI Integration & Dynamic Prompt Templating

The SDK provides a first-class, integrated system for managing **Jinja prompt templates**.

#### Prompt Template Discovery

Templates are auto-discovered from the `prompts/` directory:

```
chronicle/
├── src/
│   └── lib.rs
├── prompts/
│   ├── summarize_paper.jinja
│   └── find_gaps.jinja
└── manifest.json
```

#### Jinja Template Example

```jinja
You are an expert research assistant. Provide a concise summary of this paper.

Paper Title: {{ paper.title }}
Authors: {{ paper.authors | join(", ") }}

Text:
{{ paper.text | truncate(4000) }}

---
Summary:
```

#### Type-Safe Rust API

```rust
use serde::Serialize;

#[derive(Serialize)]
struct SummarizeContext<'a> {
    paper: PaperDetails<'a>,
}

#[derive(Serialize)]
struct PaperDetails<'a> {
    title: &'a str,
    authors: Vec<&'a str>,
    text: &'a str,
}

async fn get_summary(ctx: &JobContext, entry: &Entry, text: &str) -> JobResult<String> {
    let template_context = SummarizeContext {
        paper: PaperDetails {
            title: entry.title().unwrap_or("Untitled"),
            authors: entry.authors(),
            text,
        },
    };

    let summary = ctx.ai()
        .with_model("local_llm")
        .prompt_template("summarize_paper.jinja")
        .render_with(&template_context)?
        .generate_text()
        .await?;

    Ok(summary)
}
```

**Benefits:**
- ✅ **Separation of Concerns:** Prompts live in `.jinja` files
- ✅ **Dynamic & Interchangeable:** Update prompts without recompiling
- ✅ **Type-Safe:** Rust structs prevent template errors
- ✅ **Configurable:** Model selection respects user preferences

---

## 4. Extension Declaration

```rust
#[extension(
    id = "chronicle",
    name = "Chronicle Research Assistant",
    version = "1.0.0",
    permissions = [Permission::ReadFiles, Permission::UseAI]
)]
struct Chronicle {
    config: ChronicleConfig,
}

#[derive(Serialize, Deserialize)]
struct ChronicleConfig {
    #[setting(label = "Auto-analyze PDFs", default = true)]
    auto_analyze: bool,

    #[setting(label = "AI Model", default = "local_llm")]
    ai_model: String,
}
```

---

## 5. Data Models

### Basic Model with Sync Strategies

```rust
#[model]
#[sync_strategy(conflict = "union_merge", priority = "hlc")]
struct ResearchProject {
    // Entry references - device-owned
    #[entry(kind = "directory")]
    root_folder: Entry,

    // Shared data - HLC-ordered sync
    #[shared]
    #[indexed]
    title: String,

    #[shared]
    topics: Vec<Tag>,

    // Sidecar data - travels with entries
    #[sidecar(file = "graph.json")]
    knowledge_graph: KnowledgeGraph,

    // Agent memory - persistent, synced
    #[agent_memory]
    research_state: ResearchState,

    // Computed fields - derived, not stored
    #[computed]
    completion: f32,

    // Encrypted - zero-knowledge
    #[encrypted]
    private_notes: String,
}
```

### Data Model with Versioning

```rust
#[model]
#[version = "2.0.0"]
#[migrate_from = "1.0.0"]
struct Contact {
    #[shared] name: String,
    #[shared] email: String,

    #[since = "2.0.0"]
    social_media: Option<HashMap<String, String>>,
}

impl Migrate<ContactV1> for Contact {
    fn migrate(old: ContactV1) -> Self {
        Self {
            name: old.name,
            email: old.email,
            social_media: None,
        }
    }
}
```

---

## 6. Agents

### Complete Agent

```rust
#[agent]
#[memory(persistent = true, sync = true)]
impl Chronicle {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle initialized");
        ctx.restore_state().await?;
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        ctx.jobs()
            .dispatch(analyze_paper, entry)
            .on_device_with_capability(Capability::GPU)
            .when_idle()
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

    #[scheduled(cron = "0 9 * * *")]
    async fn daily_summary(ctx: &AgentContext) -> AgentResult<()> {
        let papers_read = ctx.memory()
            .reading_history()
            .since_yesterday()
            .count()
            .await?;

        if papers_read > 0 {
            ctx.notify()
                .on_active_device()
                .message(format!("Read {} papers yesterday", papers_read))
                .send()
                .await?;
        }

        Ok(())
    }
}
```

---

## 7. Jobs with Task Composition

Jobs compose multiple **tasks** with automatic checkpointing between steps.

### Job with AI Integration

```rust
#[job]
async fn analyze_paper(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    ctx.report_progress("Starting analysis...", 0)?;

    // Task 1: Extract text (auto-checkpointed after completion)
    let text = ctx.task(|| async {
        ctx.ai()
            .local_if_available()
            .ocr_document(&entry)
            .await
    }).await?;

    // Task 2: Generate summary using Jinja template
    let summary = ctx.task(|| async {
        #[derive(Serialize)]
        struct Context<'a> { paper: PaperDetails<'a> }

        #[derive(Serialize)]
        struct PaperDetails<'a> {
            title: &'a str,
            text: &'a str,
        }

        let context = Context {
            paper: PaperDetails {
                title: entry.name(),
                text: &text,
            },
        };

        ctx.ai()
            .with_model(&ctx.config().ai_model)
            .prompt_template("summarize_paper.jinja")
            .render_with(&context)?
            .generate_text()
            .await
    }).await?;

    // Task 3: Generate embedding
    let embedding = ctx.task(|| async {
        ctx.ai()
            .prefer_device_with(Capability::GPU)
            .embed_text(&text)
            .await
    }).await?;

    // Task 4: Save results
    ctx.task(|| async {
        entry.save_sidecar("summary", &summary).await?;
        entry.save_sidecar("embedding", &embedding).await?;

        ctx.memory()
            .graph()
            .add_paper(entry.id(), embedding, summary)
            .await
    }).await?;

    ctx.report_progress("Complete", 100)?;
    Ok(())
}
```

### Manual Checkpoints

```rust
#[job]
async fn process_batch(ctx: &JobContext, entries: Vec<Entry>) -> JobResult<()> {
    for (idx, entry) in entries.into_iter().enumerate() {
        process_entry(&entry).await?;

        // Manual checkpoint every 10 items
        if idx % 10 == 0 {
            ctx.check()?;
        }
    }
    Ok(())
}
```

### Parallel Tasks

```rust
#[job]
async fn parallel_processing(ctx: &JobContext, entries: Vec<Entry>) -> JobResult<()> {
    let embeddings = ctx
        .tasks_parallel(entries.iter().map(|entry| {
            let entry = entry.clone();
            async move {
                let text = entry.read_text().await?;
                ctx.ai().embed_text(&text).await
            }
        }))
        .max_concurrent(4)
        .await?;

    for (entry, embedding) in entries.iter().zip(embeddings) {
        entry.save_sidecar("embedding", &embedding).await?;
    }

    Ok(())
}
```

---

## 8. Actions

```rust
#[action]
async fn organize_by_topic(
    ctx: &ActionContext,
    entries: Vec<Entry>,
) -> ActionResult<ActionPreview> {
    let topics = extract_topics(&entries).await?;

    Ok(ActionPreview {
        title: "Organize papers by topic",
        description: format!("Organize {} papers into {} topics",
            entries.len(), topics.len()),
        changes: generate_changes(&entries, &topics),
        reversible: true,
    })
}

#[action_execute]
async fn organize_by_topic_execute(
    ctx: &ActionContext,
    preview: ActionPreview,
) -> ActionResult<ExecutionResult> {
    for change in preview.changes {
        apply_change(ctx, change).await?;
    }

    Ok(ExecutionResult {
        success: true,
        message: "Papers organized".to_string(),
    })
}
```

---

## 9. Device Selection & Routing

```rust
#[job]
async fn heavy_computation(ctx: &JobContext, data: Vec<Entry>) -> JobResult<()> {
    let device = ctx
        .select_device()
        .with_capability(Capability::GPU)
        .min_memory("4GB")
        .prefer_local()
        .fallback_to_cloud(CostLimit::dollars(0.50))
        .select()
        .await?;

    ctx.execute_on(device, || async {
        train_model(data).await
    }).await
}
```

---

## 10. Cross-Extension Communication

### Event Broadcasting

```rust
#[agent]
impl Chronicle {
    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        let analysis = analyze_paper(&entry).await?;

        // Broadcast to other extensions
        ctx.broadcast_event(
            ExtensionEvent::new("chronicle.paper_analyzed")
                .with_data(json!({
                    "entry_id": entry.id(),
                    "topics": analysis.topics,
                }))
        ).await?;

        Ok(())
    }
}

// In Ledger extension
#[agent]
impl Ledger {
    #[on_extension_event("chronicle.paper_analyzed")]
    async fn check_receipts(event: ExtensionEvent, ctx: &AgentContext) -> AgentResult<()> {
        let entry_id: EntryId = event.data["entry_id"].parse()?;
        let entry = ctx.vdfs().get_entry(entry_id).await?;

        if contains_receipts(&entry)? {
            extract_receipt(&entry, ctx).await?;
        }

        Ok(())
    }
}
```

---

## 11. Complete Example

```rust
use vdfs_sdk::prelude::*;
use serde::Serialize;

#[extension(id = "chronicle")]
struct Chronicle;

#[model]
struct Paper {
    #[entry] file: Entry,
    #[sidecar] embedding: Vec<f32>,
    #[sidecar] summary: String,
    #[shared] topics: Vec<String>,
}

#[agent]
impl Chronicle {
    #[on_startup]
    async fn initialize(ctx: &AgentContext) -> AgentResult<()> {
        tracing::info!("Chronicle ready");
        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_pdf(entry: Entry, ctx: &AgentContext) -> AgentResult<()> {
        ctx.jobs()
            .dispatch(analyze_paper, entry)
            .on_device_with_capability(Capability::GPU)
            .when_idle()
            .await
    }
}

#[job]
async fn analyze_paper(ctx: &JobContext, entry: Entry) -> JobResult<()> {
    // Task 1: OCR
    let text = ctx.task(|| async {
        ctx.ai().ocr_document(&entry).await
    }).await?;

    // Task 2: AI Summary with Jinja template
    let summary = ctx.task(|| async {
        #[derive(Serialize)]
        struct Context<'a> {
            title: &'a str,
            text: &'a str,
        }

        let context = Context {
            title: entry.name(),
            text: &text,
        };

        ctx.ai()
            .with_model("local_llm")
            .prompt_template("summarize.jinja")
            .render_with(&context)?
            .generate_text()
            .await
    }).await?;

    // Task 3: Embedding
    let embedding = ctx.task(|| async {
        ctx.ai()
            .prefer_device_with(Capability::GPU)
            .embed_text(&text)
            .await
    }).await?;

    // Task 4: Save
    ctx.task(|| async {
        entry.save_sidecar("summary", &summary).await?;
        entry.save_sidecar("embedding", &embedding).await?;

        ctx.memory()
            .graph()
            .add_paper(entry.id(), embedding, summary)
            .await
    }).await?;

    ctx.notify()
        .message(format!("Analyzed: {}", entry.name()))
        .send()
        .await?;

    Ok(())
}
```

---

## 12. Implementation Priorities

### Phase 1: Core (Weeks 1-4)
- Extension and data model macros
- Agent lifecycle hooks
- Job system with task composition (`ctx.task()`, `ctx.check()`)

### Phase 2: Contexts (Weeks 5-8)
- AgentContext with memory, jobs, and AI interfaces
- JobContext with task composition and AI templating
- ActionContext with preview/execute

### Phase 3: AI Integration (Weeks 9-12)
- Jinja template discovery and rendering
- AI model selection and routing
- Fluent `ctx.ai()` builder API

### Phase 4: Advanced (Weeks 13-16)
- Device selection and routing
- Cross-extension events
- Performance optimization

---

**This specification defines the complete VDFS SDK syntax, ready for implementation.**

