Of course. Here is the complete v2.3 specification for the VDFS SDK, integrating all of the concepts we've designed. This document is intended to be the definitive guide for implementation.

```rust
#![allow(unused)]
// VDFS SDK Specification v2.3
// Date: October 10, 2025
// Status: Finalized for Implementation

// ---
//
// ## 1. Guiding Principles
//
// This SDK enables developers to build local-first, AI-native applications.
// The syntax is guided by four principles:
//
// - **Declarative:** Describe intent, not procedure.
// - **Type-Safe:** Leverage the Rust compiler to eliminate runtime errors.
// - **Developer-Centric:** Abstract away complexity with intuitive, powerful APIs.
// - **Robust:** Natively support durability, security, and multi-device sync.
//
// ---
//
// ## 2. Core Primitives
//
// The SDK is built on five powerful, high-level primitives that map
// directly to the core components of a distributed application.
//
// - `#[app]`: The top-level definition for your application.
// - `#[model]`: The schema for a custom, typed, and syncable VDFS entry.
// - `#[agent]`: The autonomous logic layer, with a structured, persistent "mind".
// - `#[job]`: The durable compute layer for background processing.
// - `#[task]`: The specialist unit of work, composed within jobs.
// - `#[action]`: The user-interaction layer for UI-driven operations.
//
// ## Sections
//
// 3. The `#[app]` Primitive: Application Definition
// 4. The `#[model]` Primitive: Typed Data Layer
// 5. The `#[agent]` Primitive: Autonomous Logic Layer
//    5.1. Memory Query API Reference
// 6. AI Integration: Dynamic Prompt Templating with Jinja
// 7. Jobs & Tasks: Durable Compute Layer
// 8. The `#[action]` Primitive: User Interaction Layer
// 9. Fluent Builders: Device & AI Orchestration
//
// ---

//==============================================================================
// ## 3. The `#[app]` Primitive: Application Definition
//==============================================================================

/// The `#[app]` attribute is the entry point for your application.
/// It defines top-level metadata, permissions, and configuration schema.
#[app(
    // A unique, reverse-DNS identifier for the application.
    id = "com.spacedrive.chronicle",
    // The human-readable name of the application.
    name = "Chronicle",
    // The current version, following SemVer.
    version = "0.1.0",
    // A brief description of the application's purpose.
    description = "An AI-powered research assistant.",
    // A list of permissions the app requires to function. The user must grant these.
    permissions = [
        Permission::ReadFiles(glob = "**/*.pdf"),
        Permission::UseAI(model_selector = "local"),
        Permission::AccessNetwork(domains = ["arxiv.org"]),
        Permission::SendNotifications,
    ]
)]
struct Chronicle {
    /// The app's configuration, defined in a separate struct.
    /// The SDK uses this to automatically generate settings UI.
    config: ChronicleConfig,
}

/// Defines the user-configurable settings for the application.
#[derive(Serialize, Deserialize)]
struct ChronicleConfig {
    /// The `#[setting]` attribute makes a field configurable in the Spacedrive UI.
    #[setting(
        label = "Auto-analyze new PDFs",
        description = "Automatically start analysis when a new PDF is found.",
        default = true
    )]
    auto_analyze: bool,

    /// Example of a string setting with a default value.
    #[setting(
        label = "Default AI Model Preference",
        description = "The preferred model for summarization tasks.",
        default = "local_llm"
    )]
    ai_model_selector: String,
}


//==============================================================================
// ## 4. The `#[model]` Primitive: Typed Data Layer
//==============================================================================

/// A `#[model]` defines a new, first-class entry type in the VDFS.
/// It combines raw data blobs, typed sidecars, and syncable metadata
/// into a single, cohesive unit.
#[model]
#[version = "1.0.0"]
struct Paper {
    /// `#[blob]` marks the field holding the primary raw data for this model.
    /// The VDFS handles content-addressing and storage.
    #[blob(mime_type = "application/pdf")]
    content: VdfsBlob,

    /// `#[sync]` controls how fields are synchronized across devices.
    /// `device_owned` is the default: data is local to its creation device.
    #[sync(device_owned)]
    local_metadata: String,

    /// `shared` makes a field sync across all user devices.
    /// `conflict = "last_writer_wins"` is a simple, automatic merge strategy.
    #[sync(shared, conflict = "last_writer_wins")]
    title: String,

    /// `union_merge` is ideal for collections. If two devices add different
    /// authors, the final result will contain both.
    #[sync(shared, conflict = "union_merge")]
    authors: Vec<String>,

    /// `#[vectorized]` tells the VDFS to create a semantic vector for this field's
    /// content, making it searchable by agents.
    #[vectorized(strategy = "chunk", model = "text-embedding-ada-002")]
    full_text: String,

    /// This method defines the primary "meaning" of a Paper for AI recall.
    /// The VDFS will execute this function and vectorize its output to create
    /// a holistic embedding for the entire `Paper` object.
    #[vector_representation]
    fn main_embedding_source(&self) -> String {
        format!(
            "A paper titled '{}' written by {}.",
            self.title,
            self.authors.join(", ")
        )
    },
}

/// Models support versioning and migration.
#[model]
#[version = "1.1.0"]
#[migrate_from = "1.0.0"]
struct PaperV1_1 {
    // ... fields from v1.0.0 ...
    content: VdfsBlob,
    title: String,
    authors: Vec<String>,
    full_text: String,

    // A new field added in version 1.1.0.
    #[since = "1.1.0"]
    #[sync(shared, conflict = "last_writer_wins")]
    publication_year: Option<u16>,
}

impl Migrate<Paper> for PaperV1_1 {
    /// The `migrate` function provides the logic to upgrade an old version
    /// of a model to the new one.
    fn migrate(old: Paper) -> Self {
        Self {
            content: old.content,
            title: old.title,
            authors: old.authors,
            full_text: old.full_text,
            publication_year: None, // Default value for the new field.
        }
    }
}


//==============================================================================
// ## 5. The `#[agent]` Primitive: Autonomous Logic Layer
//==============================================================================

/// An `#[agent_memory]` struct defines the "mind" of an agent by composing
/// specialized, platform-provided memory types.
///
/// ## Multiple Memory Instances via Enums
///
/// Use enums when you need one TemporalMemory to track different event types,
/// or one AssociativeMemory to store different concept domains:

#[derive(Serialize, Deserialize, Clone)]
enum ChronicleEvent {
    PaperAnalyzed(PaperAnalysisEvent),
    CitationExtracted(CitationEvent),
    ResearchSessionCompleted(SessionEvent),
}

#[derive(Serialize, Deserialize, Clone)]
enum KnowledgeDomain {
    Concept(Concept),
    Person(PersonEntity),
    Organization(OrgEntity),
}

#[agent_memory]
#[memory_config(
    // Controls how quickly associative memories fade if not reinforced.
    decay_rate = 0.05,
    // After how many new events should the temporal memory be summarized?
    summarization_trigger = 100
)]
struct ChronicleMind {
    /// Single TemporalMemory with enum for different event types
    /// Each event variant can be queried independently
    history: TemporalMemory<ChronicleEvent>,

    /// Single AssociativeMemory with enum for different knowledge domains
    /// Enables cross-domain semantic queries
    knowledge: AssociativeMemory<KnowledgeDomain>,

    /// Simple struct for short-term state
    plan: WorkingMemory<ResearchPlan>,
}

// Developers can define custom query methods on their memory types.
// These methods are available on the agent's mind and provide domain-specific
// memory access patterns.
impl ChronicleMind {
    /// Query only PaperAnalyzed events from the enum-based history
    async fn papers_related_to(&self, question: &str) -> Vec<PaperAnalysisEvent> {
        self.history
            .query()
            // Filter to only PaperAnalyzed variant
            .where_variant(ChronicleEvent::PaperAnalyzed)
            .time_range(Duration::days(30))
            .where_semantic("summary", similar_to(question))
            .sort_by_relevance()
            .limit(10)
            .collect()
            .await
            .unwrap_or_default()
            // Extract inner PaperAnalysisEvent from enum
            .into_iter()
            .filter_map(|e| match e {
                ChronicleEvent::PaperAnalyzed(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    /// Query across all event types
    async fn recent_activity(&self, days: u64) -> Vec<ChronicleEvent> {
        self.history
            .query()
            .since(Duration::days(days))
            // No variant filter - returns all event types
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp))
            .limit(50)
            .collect()
            .await
            .unwrap_or_default()
    }

    /// Query specific knowledge domain from enum-based AssociativeMemory
    async fn find_people(&self, name_query: &str) -> Vec<PersonEntity> {
        self.knowledge
            .query_similar(name_query)
            // Filter to only Person variant
            .where_variant(KnowledgeDomain::Person)
            .top_k(10)
            .collect()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|k| match k {
                KnowledgeDomain::Person(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    /// Cross-domain query: Find all knowledge related to a concept
    async fn knowledge_graph(&self, concept: &str) -> Vec<KnowledgeDomain> {
        self.knowledge
            .query_similar(concept)
            // No variant filter - searches across Concept, Person, Organization
            .min_similarity(0.7)
            .top_k(20)
            .and_related_concepts(depth = 2)
            .collect()
            .await
            .unwrap_or_default()
    }
}

/// An `#[agent]` is a long-running entity that responds to events.
/// It uses its structured memory to make decisions and dispatch jobs.
#[agent]
impl Chronicle {
    /// `#[on_startup]` is a lifecycle hook that runs when the app is enabled.
    #[on_startup]
    async fn initialize(ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        tracing::info!("Chronicle agent initialized and ready.");
        Ok(())
    }

    /// `#[on_event]` triggers logic when a specific VDFS event occurs.
    /// This handler runs when a new `Paper` entry is created.
    #[on_event(EntryCreated)]
    #[filter = ".is_a::<Paper>()"]
    async fn on_new_paper(paper: Paper, ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        if ctx.config().auto_analyze {
            tracing::info!(paper_title = %paper.title, "New paper detected, dispatching analysis job.");

            // The agent's primary role is to dispatch jobs based on events and state.
            ctx.jobs()
                .dispatch(analyze_paper, (paper,))
                .with_priority(Priority::Low)
                .on_device_with_capability(Capability::GPU)
                .when_idle()
                .await?;
        }
        Ok(())
    }

    /// Example: Job completion stores enum variant in TemporalMemory
    #[on_job_completed(analyze_paper)]
    async fn on_analysis_complete(
        paper: Paper,
        summary: String,
        ctx: &AgentContext<ChronicleMind>,
    ) -> AgentResult<()> {
        // Store different event types in single TemporalMemory via enum
        let mut memory = ctx.memory().write().await;

        // Store paper analysis
        memory.history.append(
            ChronicleEvent::PaperAnalyzed(PaperAnalysisEvent {
                paper_id: paper.file.id(),
                title: paper.title.clone(),
                summary: summary.clone(),
            })
        ).await?;

        // Extract and store citations as separate events
        let citations = extract_citations(&summary)?;
        for citation in citations {
            memory.history.append(
                ChronicleEvent::CitationExtracted(CitationEvent {
                    citing_paper_id: paper.file.id(),
                    cited_paper_id: citation.paper_id,
                    context: citation.context,
                })
            ).await?;
        }

        // Store entities in AssociativeMemory via enum
        let entities = extract_entities(&summary)?;
        for entity in entities {
            match entity {
                Entity::Person(person) => {
                    memory.knowledge.add(
                        KnowledgeDomain::Person(PersonEntity {
                            name: person.name,
                            affiliation: person.affiliation,
                            papers_authored: vec![paper.file.id()],
                        })
                    ).await?;
                }
                Entity::Organization(org) => {
                    memory.knowledge.add(
                        KnowledgeDomain::Organization(OrgEntity {
                            name: org.name,
                            domain: org.domain,
                            associated_papers: vec![paper.file.id()],
                        })
                    ).await?;
                }
            }
        }

        Ok(())
    }

    /// `#[scheduled]` runs logic on a cron schedule.
    #[scheduled(cron = "0 9 * * MON")]
    async fn weekly_report(ctx: &AgentContext<ChronicleMind>) -> AgentResult<()> {
        let memory = ctx.memory().read().await;

        // Query all events across variants (papers, citations, sessions)
        let all_events = memory.history
            .query()
            .time_range(Duration::days(7))
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp))
            .limit(100)
            .collect()
            .await?;

        // Count by variant type
        let papers_analyzed = all_events.iter()
            .filter(|e| matches!(e, ChronicleEvent::PaperAnalyzed(_)))
            .count();
        let citations_found = all_events.iter()
            .filter(|e| matches!(e, ChronicleEvent::CitationExtracted(_)))
            .count();

        // Query specific variant using where_variant filter
        let recent_papers = memory.history
            .query()
            .where_variant(ChronicleEvent::PaperAnalyzed)
            .since(Duration::days(7))
            .collect()
            .await?;

        // Cross-domain semantic query on AssociativeMemory
        let trending = memory.knowledge
            .query_similar("recent research trends")
            .min_similarity(0.7)
            .top_k(5)
            .collect()
            .await?;

        // Extract Concept variants
        let trending_concepts: Vec<_> = trending.iter()
            .filter_map(|k| match k {
                KnowledgeDomain::Concept(c) => Some(c),
                _ => None,
            })
            .collect();

        ctx.notify()
            .title("Weekly Research Summary")
            .message(format!(
                "Papers: {}, Citations: {}\nTrending: {}",
                papers_analyzed,
                citations_found,
                trending_concepts.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            ))
            .send()
            .await?;

        Ok(())
    }
}

// Event types used by the agent's temporal memory
#[derive(Serialize, Deserialize, Clone)]
struct PaperAnalysisEvent {
    paper_id: EntryId,
    title: String,
    summary: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct CitationEvent {
    citing_paper_id: EntryId,
    cited_paper_id: EntryId,
    context: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct SessionEvent {
    session_id: Uuid,
    papers_read: Vec<EntryId>,
    duration_minutes: u32,
}

// Knowledge domain types for associative memory
#[derive(Serialize, Deserialize, Clone)]
struct Concept {
    name: String,
    definition: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PersonEntity {
    name: String,
    affiliation: Option<String>,
    papers_authored: Vec<EntryId>,
}

#[derive(Serialize, Deserialize, Clone)]
struct OrgEntity {
    name: String,
    domain: String,
    associated_papers: Vec<EntryId>,
}

// Working memory state
#[derive(Serialize, Deserialize, Clone, Default)]
struct ResearchPlan {
    priority_topics: Vec<String>,
    papers_to_read: Vec<EntryId>,
}

#[derive(Clone)]
struct ActivitySummary {
    count: usize,
    most_recent: Option<EntryId>,
    topics: Vec<Concept>,
}


//==============================================================================
// ## 5.0.1. Alternative: Multiple Memory Fields (Less Recommended)
//==============================================================================

// If you absolutely need separate memory instances, you can define multiple fields.
// However, the enum approach above is preferred for most use cases.

#[agent_memory]
struct AlternativeChronicleMind {
    // Separate temporal memories for different event streams
    paper_events: TemporalMemory<PaperAnalysisEvent>,
    citation_events: TemporalMemory<CitationEvent>,
    session_events: TemporalMemory<SessionEvent>,

    // Separate associative memories for different domains
    concepts: AssociativeMemory<Concept>,
    people: AssociativeMemory<PersonEntity>,
    organizations: AssociativeMemory<OrgEntity>,

    plan: WorkingMemory<ResearchPlan>,
}

// This works but results in:
// - More storage overhead (7 separate memory instances)
// - No cross-stream queries (can't easily correlate papers with citations)
// - More complex API (7 different query entry points)
//
// The enum approach above is cleaner and more powerful.

//==============================================================================
// ## 5.0.2. Enum Approach Benefits
//==============================================================================

// ### Why Enums Are Preferred:
//
// 1. **Single Source of Truth**
//    - One TemporalMemory with all event types
//    - Correlate across events: "Show papers AND citations from last week"
//
// 2. **Simpler Storage**
//    - One history.db file instead of three
//    - One knowledge.vss file instead of three
//    - Easier backup and sync
//
// 3. **Flexible Queries**
//    ```rust
//    // Query specific variant
//    memory.history.query().where_variant(ChronicleEvent::PaperAnalyzed)
//
//    // Query all variants
//    memory.history.query().since(Duration::days(7))
//
//    // Cross-variant correlation
//    memory.history.query()
//        .where_variant(ChronicleEvent::PaperAnalyzed)
//        .with_related_events(ChronicleEvent::CitationExtracted)
//    ```
//
// 4. **Type-Safe Pattern Matching**
//    ```rust
//    for event in all_events {
//        match event {
//            ChronicleEvent::PaperAnalyzed(p) => handle_paper(p),
//            ChronicleEvent::CitationExtracted(c) => handle_citation(c),
//            ChronicleEvent::ResearchSessionCompleted(s) => handle_session(s),
//        }
//    }
//    ```
//
// 5. **Easier to Extend**
//    - Add new variant: `ChronicleEvent::ConceptDiscovered(ConceptEvent)`
//    - Existing queries still work
//    - No new storage files needed


//==============================================================================
// ## 5.1. Memory Query API Reference
//==============================================================================

// The SDK provides three specialized memory types, each with rich query capabilities:

// ### TemporalMemory<T> - Time-Series Event Log
//
// ```rust
// memory.history
//     .query()
//     // Enum variant filtering (when T is an enum)
//     .where_variant(ChronicleEvent::PaperAnalyzed)
//
//     // Time filtering
//     .since(Duration::days(7))
//     .until(DateTime::now())
//     .time_range(start..end)
//
//     // Field filtering (within enum variant)
//     .where_field("title", contains("neural"))
//     .where_field("authors", contains("Smith"))
//
//     // Semantic filtering (uses embeddings from VSS)
//     .where_semantic("summary", similar_to("machine learning"))
//
//     // Correlation queries (across enum variants)
//     .with_related_events(ChronicleEvent::CitationExtracted)
//
//     // Sorting and limiting
//     .sort_by(|a, b| a.timestamp.cmp(&b.timestamp))
//     .sort_by_relevance()
//     .limit(10)
//
//     // Aggregation
//     .collect().await?
//     .count().await?
//     .group_by(|e| match e {
//         ChronicleEvent::PaperAnalyzed(_) => "paper",
//         ChronicleEvent::CitationExtracted(_) => "citation",
//         ChronicleEvent::ResearchSessionCompleted(_) => "session",
//     }).await?
// ```

// ### AssociativeMemory<T> - Semantic Knowledge Graph
//
// ```rust
// memory.knowledge
//     // Similarity search (works across all enum variants by default)
//     .query_similar("quantum computing")
//     .query_vector(embedding_vec)
//
//     // Variant filtering (when T is an enum)
//     .where_variant(KnowledgeDomain::Concept)
//     .where_variant(KnowledgeDomain::Person)
//
//     // Context filtering
//     .within_context(&recent_events)
//     .related_to(&concept)
//
//     // Relevance filtering
//     .min_similarity(0.8)
//     .top_k(5)
//
//     // Relationship traversal (across variants!)
//     .and_related_concepts(depth = 2)
//
//     .collect().await?
// ```
//
// With enums, AssociativeMemory can discover relationships across domains:
// - Find concepts related to a person
// - Find organizations related to a research topic
// - Build knowledge graphs spanning multiple entity types

// ### WorkingMemory<T> - Transactional Short-Term State
//
// ```rust
// // Atomic read
// let plan = memory.plan.read().await;
//
// // Transactional write
// memory.plan.update(|mut plan| {
//     plan.priority_topics.push("AI safety".to_string());
//     Ok(plan) // Commit on Ok, rollback on Err
// }).await?
// ```


//==============================================================================
// ## 6. AI Integration: Dynamic Prompt Templating with Jinja
//==============================================================================

// The SDK provides a first-class system for managing AI prompts using Jinja templates.
// This separates the "art" of prompt engineering from application logic, enabling
// rapid iteration and user customization without recompiling.

// ### Template Discovery
//
// The VDFS runtime automatically discovers prompt templates in the `prompts/`
// directory of your application package:
//
// chronicle/
// ├── src/
// │   └── lib.rs
// ├── prompts/
// │   ├── summarize_paper.jinja
// │   └── extract_concepts.jinja
// └── manifest.json

// ### Example Jinja Template (prompts/summarize_paper.jinja)
//
// ```jinja
// You are an expert research assistant. Provide a concise, structured summary.
//
// Paper Title: {{ title }}
// Authors: {{ authors | join(", ") }}
//
// Full Text:
// {{ text | truncate(4000) }}
//
// ---
// Provide a three-point summary:
// 1.
// 2.
// 3.
// ```

// ### Type-Safe Template Rendering
//
// Templates are rendered using strongly-typed Rust structs, preventing runtime errors.
// The fluent `ctx.ai()` builder provides the interface:

#[derive(Serialize)]
struct SummarizePromptContext<'a> {
    title: &'a str,
    authors: Vec<&'a str>,
    text: &'a str,
}

async fn example_ai_call(ctx: &TaskContext, paper: &Paper) -> TaskResult<String> {
    let prompt_ctx = SummarizePromptContext {
        title: &paper.title,
        authors: paper.authors.iter().map(|s| s.as_str()).collect(),
        text: &paper.full_text,
    };

    ctx.ai()
        // Model preference respects user configuration
        .model_preference(&ctx.config().ai_model_selector)
        // Auto-discovered template from prompts/ directory
        .prompt_template("summarize_paper.jinja")
        // Type-safe rendering with our struct
        .render_with(&prompt_ctx)?
        // Generate text using the selected model
        .generate_text()
        .await
}

// Benefits of this approach:
// - **Separation of Concerns:** Prompts are separate files, not embedded strings
// - **Dynamic Updates:** Change prompts without recompiling the app
// - **User Customization:** Advanced users can modify or add their own templates
// - **Type Safety:** Rust structs ensure all template variables are provided
// - **Model Agnostic:** Works with local LLMs, cloud APIs, or hybrid setups


//==============================================================================
// ## 7. Jobs & Tasks: Durable Compute Layer
//==============================================================================

/// A `#[task]` is the smallest, specialist unit of durable work.
/// It has its own configuration for retries and timeouts.
#[task(
    retries = 3,
    timeout_ms = 120000,
    required_capability = Capability::TextExtraction
)]
async fn extract_full_text(paper: &Paper) -> TaskResult<String> {
    let bytes = paper.content.read().await?;
    // ... complex OCR and text extraction logic ...
    Ok("Extracted text content.".to_string())
}

/// Another task, this one for AI processing.
#[task(retries = 2, required_capability = Capability::AI)]
async fn generate_summary(ctx: &TaskContext, title: &str, text: &str) -> TaskResult<String> {
    #[derive(Serialize)]
    struct PromptContext<'a> { title: &'a str, text: &'a str }
    let context = PromptContext { title, text };

    ctx.ai()
        // The developer specifies a *preference* or required capability.
        // The end-user's configuration in Spacedrive determines the actual model.
        .model_preference(&ctx.config().ai_model_selector)
        .prompt_template("summarize_paper.jinja")
        .render_with(&context)?
        .generate_text()
        .await
}

/// A `#[job]` is the orchestrator, composing multiple tasks to achieve a goal.
/// It has its own configuration, like `parallelism`, for processing collections.
#[job(parallelism = 4)]
async fn analyze_paper(ctx: &JobContext, paper: Paper) -> JobResult<()> {
    ctx.report_progress("Starting analysis...", 0.1)?;

    // A job's primary function is to `run` tasks in a sequence.
    // Each `ctx.run` call is automatically checkpointed. If the job fails
    // after this line, it will resume *after* this task on the next run.
    let text = ctx.run(extract_full_text, (&paper,)).await?;
    ctx.report_progress("Text extracted.", 0.4)?;

    let summary = ctx.run(generate_summary, (&paper.title, &text)).await?;
    ctx.report_progress("Summary generated.", 0.8)?;

    // A final step to update the model with the results.
    let mut paper_mut = ctx.vdfs().get_mut::<Paper>(paper.id).await?;
    paper_mut.full_text = text;
    paper_mut.summary = Some(summary);
    paper_mut.save().await?;

    ctx.report_progress("Analysis complete.", 1.0)?;
    Ok(())
}


//==============================================================================
// ## 8. The `#[action]` Primitive: User Interaction Layer
//==============================================================================

/// An `#[action]` is a user-invokable operation. It follows a two-step
/// pattern: first, it generates a `preview` of the changes it will make.
#[action]
async fn organize_by_topic(
    ctx: &ActionContext,
    papers: Vec<Paper>,
) -> ActionResult<ActionPreview> {
    // 1. Perform read-only analysis to determine the plan.
    let topics = determine_topics_from_papers(&papers).await?;
    let changes = papers.iter().map(|p| Change {
        entry_id: p.id,
        new_topic: topics.get(&p.id).cloned(),
        description: format!("Set topic for '{}'", p.title),
    }).collect();

    // 2. Return a preview for the user to confirm.
    Ok(ActionPreview {
        title: "Organize Papers by Topic",
        description: format!("This will assign topics to {} papers.", papers.len()),
        changes,
        reversible: true,
    })
}

/// After the user confirms the preview, the `#[action_execute]` function runs,
/// applying the changes described in the preview.
#[action_execute]
async fn organize_by_topic_execute(
    ctx: &ActionContext,
    preview: ActionPreview,
) -> ActionResult<ExecutionResult> {
    for change in preview.changes {
        let mut paper = ctx.vdfs().get_mut::<Paper>(change.entry_id).await?;
        if let Some(topic) = change.new_topic {
            paper.topics.push(topic);
        }
        paper.save().await?;
    }

    Ok(ExecutionResult {
        success: true,
        message: format!("Successfully organized {} papers.", preview.changes.len()),
    })
}


//==============================================================================
// ## 9. Fluent Builders: Device & AI Orchestration
//==============================================================================

// The SDK provides powerful, fluent builders for complex operations like
// selecting the optimal device to run a computation on.

async fn device_selection_example(ctx: &JobContext, data: Vec<u8>) {
    // This builder finds the best device based on a cascade of requirements.
    let target_device = ctx.select_device()
        .with_capability(Capability::GPU)
        .min_memory("8GB")
        .prefer_local() // Prefer devices on the same LAN first.
        .fallback_to_cloud(CostLimit::dollars(1.50)) // If no local device, use cloud.
        .select()
        .await?;

    // Execute a closure on the selected device. The SDK handles serialization,
    // remote execution, and returning the result.
    let result: ComputationResult = ctx.execute_on(&target_device, || async {
        // This code runs on the remote device.
        run_heavy_computation(data).await
    }).await?;
}
```
