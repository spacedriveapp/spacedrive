# Extension Agent System Design

**Version:** 1.0
**Status:** Design Phase
**Author:** Spacedrive Team
**Date:** October 2025

---

## Executive Summary

This document specifies Spacedrive's extension-based agent system—a WASM-sandboxed architecture where domain-specific AI agents observe the Virtual Distributed File System (VDFS), maintain persistent knowledge, and propose safe, verifiable actions. Unlike general-purpose AI assistants, Spacedrive agents are specialized extensions (Photos, Finance, Storage) that bring deep domain expertise while operating within strict security boundaries.

**Core Principles:**
1. **Extension-Native**: Agents live inside WASM extensions, not as separate processes
2. **Event-Driven**: React to VDFS changes in real-time, not through polling
3. **Memory-First**: Persistent knowledge enables learning and pattern recognition
4. **Action-Based**: All modifications use the existing Transactional Action System
5. **Permission-Scoped**: Fine-grained access control to specific library locations

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Spacedrive Core                          │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │   Indexer    │  │  Event Bus   │  │  Action System  │  │
│  │  (VDFS)      │──│              │──│                 │  │
│  └──────────────┘  └──────┬───────┘  └─────────────────┘  │
│                           │                                 │
│                           │ Events                          │
│  ┌────────────────────────┼───────────────────────────┐    │
│  │   Extension Runtime (WASM)                         │    │
│  │                        │                            │    │
│  │   ┌────────────────────▼─────────────────────┐    │    │
│  │   │  Agent Instance (Photos.wasm)            │    │    │
│  │   │                                          │    │    │
│  │   │  ┌──────────────┐  ┌──────────────┐   │    │    │
│  │   │  │  Lifecycle   │  │    Memory    │   │    │    │
│  │   │  │   Hooks      │  │   Systems    │   │    │    │
│  │   │  └──────┬───────┘  └──────┬───────┘   │    │    │
│  │   │         │                  │            │    │    │
│  │   │         ▼                  ▼            │    │    │
│  │   │  ┌─────────────────────────────────┐  │    │    │
│  │   │  │      Agent Context              │  │    │    │
│  │   │  │  • VDFS Access                  │  │    │    │
│  │   │  │  • AI Models                    │  │    │    │
│  │   │  │  • Job Dispatcher               │  │    │    │
│  │   │  │  • Memory Handle                │  │    │    │
│  │   │  │  • Permission Checker           │  │    │    │
│  │   │  └─────────────────────────────────┘  │    │    │
│  │   └──────────────────────────────────────┘    │    │
│  └────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Agent Instance

An agent is a stateful component within a WASM extension that observes the VDFS and coordinates intelligent processing.

```rust
/// The Photos extension agent
pub struct PhotosAgent {
    config: PhotosConfig,
    // No direct state - accessed via Context
}

impl PhotosAgent {
    #[on_startup]
    async fn initialize(&self, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // Called once when extension loads
        ctx.trace("Photos agent starting");

        // Register any runtime resources
        self.ensure_models_available(ctx).await?;

        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = "entry.is_image() && ctx.in_granted_scope(entry.path())"]
    async fn handle_new_photo(&self, entry: Entry, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // Queue for batch processing
        ctx.memory().update(|mut memory| {
            memory.plan.photos_pending.push(entry.id());

            // Batch dispatch threshold
            if memory.plan.photos_pending.len() >= 50 {
                ctx.jobs().dispatch(
                    AnalyzePhotosBatch,
                    memory.plan.photos_pending.drain(..).collect()
                ).priority(Priority::Low).await?;
            }

            Ok(memory)
        }).await?;

        Ok(())
    }

    #[scheduled(cron = "0 3 * * *")]  // Daily at 3am
    async fn maintenance(&self, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // Prune old temporal events, compact knowledge graph
        ctx.memory().cleanup(retention_days = 90).await?;
        Ok(())
    }
}
```

**Key Characteristics:**
- **Stateless struct** - all state in memory systems
- **Declarative hooks** - attributes define when code runs
- **Scoped access** - filter ensures permission boundaries
- **Async throughout** - no blocking operations

---

### 2. Agent Context

The runtime environment provided to every agent function. This is the primary API surface.

```rust
pub struct AgentContext<M: AgentMemory> {
    // Core services
    vdfs: VdfsHandle,
    ai: AiHandle,
    jobs: JobHandle,
    memory: MemoryHandle<M>,

    // Agent metadata
    extension_id: String,
    permissions: PermissionSet,

    // Utilities
    trace: TraceHandle,
    notify: NotifyHandle,
}

impl<M: AgentMemory> AgentContext<M> {
    // VDFS operations (read-only unless permission granted)
    pub fn vdfs(&self) -> &VdfsHandle;

    // AI model inference
    pub fn ai(&self) -> &AiHandle;

    // Job dispatch
    pub fn jobs(&self) -> &JobHandle;

    // Persistent memory access
    pub fn memory(&self) -> &MemoryHandle<M>;

    // Check if path is in granted scope
    pub fn in_granted_scope(&self, path: &str) -> bool;

    // Debug logging (stored in extension trail)
    pub fn trace(&self, message: impl Into<String>);

    // User notifications
    pub fn notify(&self) -> NotificationBuilder;
}
```

**Design Rationale:**
- **Immutable access** - prevents accidental state corruption
- **Capability-based** - services injected, not globally accessed
- **Type-safe memory** - generic over agent's memory type
- **Permission checking** built-in

---

### 3. Memory System

Agents maintain three types of persistent memory, each optimized for different access patterns.

#### Temporal Memory (Event Log)

Append-only log of timestamped events with temporal queries.

```rust
pub struct TemporalMemory<T: Event> {
    storage: EventLog,  // SQLite with FTS5 index
}

impl<T: Event> TemporalMemory<T> {
    /// Append new event to timeline
    pub async fn append(&mut self, event: T) -> Result<EventId>;

    /// Query events by time and content
    pub fn query(&self) -> TemporalQuery<T>;
}

pub struct TemporalQuery<T> {
    // Private state
}

impl<T: Event> TemporalQuery<T> {
    /// Filter by event variant
    pub fn variant(self, name: &str) -> Self;

    /// Filter by time range
    pub fn since(self, duration: Duration) -> Self;
    pub fn between(self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self;

    /// Filter by content (uses FTS5)
    pub fn where_text(self, field: &str, matches: &str) -> Self;
    pub fn where_field(self, field: &str, equals: serde_json::Value) -> Self;

    /// Sort and limit
    pub fn order_by_time(self, ascending: bool) -> Self;
    pub fn limit(self, n: usize) -> Self;

    /// Execute query
    pub async fn collect(self) -> Result<Vec<T>>;
    pub async fn count(self) -> Result<usize>;
}
```

**Storage:**
- Location: `.sdlibrary/sidecars/extension/{ext_id}/memory/temporal.db`
- Format: SQLite with JSON columns for event data
- Indexes: Timestamp, event variant, FTS5 on text fields

**Example:**
```rust
#[derive(Serialize, Deserialize)]
enum PhotoEvent {
    PhotoAnalyzed {
        photo_id: Uuid,
        faces_detected: u32,
        scenes: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    PersonIdentified {
        person_id: Uuid,
        photo_id: Uuid,
        confidence: f32,
    },
}

// Query usage
let recent_analyses = memory.history
    .query()
    .variant("PhotoAnalyzed")
    .since(Duration::days(7))
    .where_text("scenes", "beach")
    .order_by_time(false)  // Newest first
    .collect()
    .await?;
```

#### Associative Memory (Knowledge Graph)

Vector-based storage for semantic relationships and similarity search.

```rust
pub struct AssociativeMemory<T: Knowledge> {
    storage: VectorStore,  // Uses existing Vector Repository system
}

impl<T: Knowledge> AssociativeMemory<T> {
    /// Add knowledge with automatic embedding
    pub async fn add(&mut self, knowledge: T) -> Result<KnowledgeId>;

    /// Update existing knowledge
    pub async fn update(&mut self, id: KnowledgeId, knowledge: T) -> Result<()>;

    /// Remove knowledge
    pub async fn remove(&mut self, id: KnowledgeId) -> Result<()>;

    /// Query by similarity or structure
    pub fn query(&self) -> AssociativeQuery<T>;
}

pub struct AssociativeQuery<T> {
    // Private state
}

impl<T: Knowledge> AssociativeQuery<T> {
    /// Semantic similarity search
    pub fn similar_to(self, text: &str) -> Self;
    pub fn similar_to_embedding(self, embedding: &[f32]) -> Self;

    /// Filter by variant
    pub fn variant(self, name: &str) -> Self;

    /// Structural filters
    pub fn where_field(self, field: &str, predicate: FieldPredicate) -> Self;

    /// Graph traversal
    pub fn related_to(self, id: KnowledgeId, max_depth: u32) -> Self;

    /// Result controls
    pub fn min_similarity(self, threshold: f32) -> Self;
    pub fn top_k(self, k: usize) -> Self;

    /// Execute
    pub async fn collect(self) -> Result<Vec<T>>;
}
```

**Storage:**
- Location: `.sdlibrary/sidecars/extension/{ext_id}/memory/knowledge.vss`
- Format: Vector Repository (reuses core's vector storage)
- Embeddings: Generated automatically using registered models

**Example:**
```rust
#[derive(Serialize, Deserialize)]
enum PhotoKnowledge {
    FaceCluster {
        person_id: Uuid,
        representative_embedding: Vec<f32>,
        photo_ids: Vec<Uuid>,
    },
    PlaceCluster {
        place_id: Uuid,
        name: String,
        center: (f64, f64),  // lat, lon
        photo_ids: Vec<Uuid>,
    },
}

// Query usage
let alice_places = memory.knowledge
    .query()
    .variant("FaceCluster")
    .where_field("person_id", equals(alice_id))
    .related_to(/* PlaceCluster */, depth = 1)
    .collect()
    .await?;
```

#### Working Memory (Transactional State)

Current operational state with transactional updates.

```rust
pub struct WorkingMemory<T: Default + Serialize + DeserializeOwned> {
    storage: AtomicState,  // File-backed with atomic writes
}

impl<T> WorkingMemory<T> {
    /// Read current state
    pub async fn read(&self) -> T;

    /// Update state transactionally
    pub async fn update<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(T) -> Result<T>;

    /// Replace entire state
    pub async fn set(&mut self, state: T) -> Result<()>;

    /// Reset to default
    pub async fn reset(&mut self) -> Result<()>;
}
```

**Storage:**
- Location: `.sdlibrary/sidecars/extension/{ext_id}/memory/working.json`
- Format: JSON with atomic write (write to temp, rename)
- Transactions: Function-based, automatic rollback on error

**Example:**
```rust
#[derive(Default, Serialize, Deserialize)]
struct AnalysisPlan {
    photos_pending: Vec<Uuid>,
    clustering_needed: bool,
    last_sync: Option<DateTime<Utc>>,
}

// Usage
memory.plan.update(|mut plan| {
    plan.photos_pending.push(photo_id);
    plan.last_sync = Some(Utc::now());
    Ok(plan)
}).await?;
```

---

## Event System

Agents react to VDFS events through a type-safe subscription mechanism.

### Event Flow

```
VDFS Indexer           Event Bus              Extension Runtime
     │                     │                          │
     │─ IndexComplete ────│                          │
     │─ EntryCreated ─────│──────────────────────────│
     │─ EntryModified ────│                          │
     │                     │                   ┌──────▼──────┐
     │                     │                   │ Event Router│
     │                     │                   └──────┬──────┘
     │                     │                          │
     │                     │                   ┌──────▼──────────┐
     │                     │                   │ Permission Check│
     │                     │                   └──────┬──────────┘
     │                     │                          │
     │                     │                   ┌──────▼──────────┐
     │                     │                   │  Photos Agent   │
     │                     │                   │  on_event()     │
     │                     │                   └─────────────────┘
```

### Event Types

```rust
/// Events agents can subscribe to
#[derive(Clone, Serialize, Deserialize)]
pub enum VdfsEvent {
    /// New entry indexed
    EntryCreated {
        entry: Entry,
        location_id: Uuid,
    },

    /// Entry metadata updated
    EntryModified {
        entry: Entry,
        changed_fields: Vec<String>,
    },

    /// Entry removed from index
    EntryDeleted {
        entry_id: Uuid,
        path: String,
    },

    /// Location scan completed
    IndexComplete {
        location_id: Uuid,
        entries_processed: usize,
        duration_ms: u64,
    },

    /// Tag applied to entry
    TagAdded {
        entry_id: Uuid,
        tag: String,
    },
}
```

### Event Filters

Agents specify which events they care about using attribute filters:

```rust
#[on_event(EntryCreated)]
#[filter = "entry.extension() == 'pdf' && entry.size() > 1024"]
async fn on_pdf_created(&self, entry: Entry, ctx: &AgentContext<M>) -> Result<()> {
    // Only called for PDFs > 1KB
}

#[on_event(TagAdded)]
#[filter = "tag.starts_with('#project:')"]
async fn on_project_tag(&self, entry_id: Uuid, tag: String, ctx: &AgentContext<M>) -> Result<()> {
    // Only called for project tags
}
```

**Filter Expressions:**
- Simple DSL compiled to Rust predicates
- Access to event fields (entry, tag, location_id, etc.)
- Common operations: `==`, `!=`, `>`, `<`, `starts_with`, `contains`, `matches`

---

## Tools System

Tools are LLM-callable functions that enable conversational interfaces to agent capabilities.

### Tool Definition

```rust
use spacedrive_sdk::prelude::*;

#[tool("Search photos by person name")]
async fn search_person(
    ctx: &ToolContext,
    person_name: String,
    limit: Option<u32>,
) -> ToolResult<Vec<PhotoMetadata>> {
    // Validate inputs
    if person_name.is_empty() {
        return ToolResult::error("Person name cannot be empty");
    }

    // Query VDFS
    let photos = ctx.vdfs()
        .query_entries()
        .with_tag(&format!("#person:{}", person_name))
        .limit(limit.unwrap_or(20))
        .collect()
        .await?;

    // Return structured result
    Ok(ToolResult::success(photos))
}
```

### Tool Context

```rust
pub struct ToolContext {
    // Read-only VDFS access
    vdfs: VdfsHandle,

    // AI model access
    ai: AiHandle,

    // Job dispatch (for long-running operations)
    jobs: JobHandle,

    // Extension metadata
    extension_id: String,
    call_id: String,  // Unique per invocation

    // Permission checking
    permissions: PermissionSet,
}
```

### Tool Result

```rust
pub enum ToolResult<T> {
    Success(T),
    Error {
        message: String,
        recoverable: bool,
    },
    NeedsApproval {
        action: ActionPreview,
        reason: String,
    },
}
```

### Tool Registry

```rust
/// Auto-generated by #[tool] macro
impl SearchPersonTool {
    pub fn schema() -> ToolSchema {
        ToolSchema {
            name: "search_person",
            description: "Search photos by person name",
            parameters: ParameterSchema::object([
                ("person_name", ParameterSchema::string("Name of person to search for")),
                ("limit", ParameterSchema::integer("Maximum results").optional()),
            ]),
        }
    }
}

/// Extension registers tools on initialization
#[extension_init]
fn register_tools(registry: &mut ToolRegistry) {
    registry.register(SearchPersonTool::schema(), SearchPersonTool::execute);
    registry.register(IdentifyFaceTool::schema(), IdentifyFaceTool::execute);
    registry.register(CreateAlbumTool::schema(), CreateAlbumTool::execute);
}
```

---

## Agent Memory Implementation

### Memory Trait

```rust
pub trait AgentMemory: Send + Sync + 'static {
    /// Save memory to disk
    async fn persist(&self, base_path: &Path) -> Result<()>;

    /// Load memory from disk
    async fn restore(base_path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Clean up old data
    async fn cleanup(&mut self, retention_policy: RetentionPolicy) -> Result<()>;
}
```

### Memory Handle

```rust
pub struct MemoryHandle<M: AgentMemory> {
    inner: Arc<RwLock<M>>,
    persistence: PersistenceManager,
}

impl<M: AgentMemory> MemoryHandle<M> {
    /// Read memory immutably
    pub async fn read(&self) -> MemoryReadGuard<M>;

    /// Update memory with automatic persistence
    pub async fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(M) -> Result<M>;

    /// Force persistence (auto-persists every 30s anyway)
    pub async fn sync(&self) -> Result<()>;

    /// Clean old data
    pub async fn cleanup(&self, retention_days: u32) -> Result<()>;
}
```

### Example: Photos Agent Memory

```rust
#[agent_memory]
pub struct PhotosMind {
    /// Timeline of photo analysis
    pub history: TemporalMemory<PhotoEvent>,

    /// Face clusters and place knowledge
    pub knowledge: AssociativeMemory<PhotoKnowledge>,

    /// Current processing queue
    pub plan: WorkingMemory<AnalysisPlan>,
}

#[derive(Serialize, Deserialize)]
pub enum PhotoEvent {
    PhotoAnalyzed {
        photo_id: Uuid,
        faces_detected: u32,
        scenes: Vec<String>,
        #[serde(default = "Utc::now")]
        timestamp: DateTime<Utc>,
    },
    MomentCreated {
        moment_id: Uuid,
        photo_count: usize,
        date_range: (DateTime<Utc>, DateTime<Utc>),
    },
}

impl Event for PhotoEvent {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            PhotoEvent::PhotoAnalyzed { timestamp, .. } => *timestamp,
            PhotoEvent::MomentCreated { date_range, .. } => date_range.0,
        }
    }

    fn variant_name(&self) -> &'static str {
        match self {
            PhotoEvent::PhotoAnalyzed { .. } => "PhotoAnalyzed",
            PhotoEvent::MomentCreated { .. } => "MomentCreated",
        }
    }
}
```

---

## Job and Task Integration

Agents dispatch work through the existing job system.

### Job Dispatch

```rust
#[on_event(EntryCreated)]
async fn on_new_photo(&self, entry: Entry, ctx: &AgentContext<PhotosMind>) -> Result<()> {
    // Collect batch
    let batch = ctx.memory().read().await.plan.photos_pending.clone();

    if batch.len() >= 50 {
        // Dispatch to job system
        ctx.jobs()
            .dispatch(AnalyzePhotosBatch { photo_ids: batch })
            .priority(Priority::Low)
            .when_idle()  // Run when system not busy
            .await?;

        // Clear queue
        ctx.memory().update(|mut m| {
            m.plan.photos_pending.clear();
            Ok(m)
        }).await?;
    }

    Ok(())
}
```

### Job Definition (Existing System)

```rust
#[job(name = "analyze_photos_batch")]
pub struct AnalyzePhotosBatch {
    photo_ids: Vec<Uuid>,

    #[job(state)]  // Persisted for resumability
    current_index: usize,
}

impl Job for AnalyzePhotosBatch {
    async fn execute(&mut self, ctx: &JobContext) -> JobResult<()> {
        let total = self.photo_ids.len();

        // Resume from checkpoint
        for i in self.current_index..total {
            let photo_id = self.photo_ids[i];

            // Run task
            ctx.run_task(DetectFaces { photo_id }).await?;

            // Update progress
            self.current_index = i + 1;
            ctx.progress((i + 1) as f32 / total as f32);

            // Checkpoint (can resume from here)
            ctx.checkpoint().await?;
        }

        Ok(())
    }
}
```

**Key Pattern:** Agents trigger jobs, jobs execute tasks, results flow back to agent memory.

---

## Action Integration

Agents propose modifications through the Action System.

### Action Proposal Flow

```rust
#[tool("Create album from photos")]
async fn create_album(
    ctx: &ToolContext,
    name: String,
    photo_ids: Vec<Uuid>,
) -> ToolResult<ActionReceipt> {
    // Build action
    let action = CreateAlbumAction {
        name: name.clone(),
        photo_ids: photo_ids.clone(),
    };

    // Preview action (shows what will happen)
    let preview = ctx.actions()
        .preview(action)
        .await?;

    // Check if requires user approval
    if preview.impact_score > 0.5 {
        return ToolResult::NeedsApproval {
            action: preview,
            reason: format!("Creating album with {} photos", photo_ids.len()),
        };
    }

    // Auto-approve low-impact actions
    let receipt = ctx.actions()
        .execute(action)
        .await?;

    Ok(ToolResult::success(receipt))
}
```

**Integration Points:**
- Agents call `ctx.actions().preview()` and `ctx.actions().execute()`
- Action System handles validation, preview generation, audit logging
- Results flow through existing action receipts

---

## Permission System

### Permission Declaration

```rust
#[extension(
    id = "com.spacedrive.photos",
    permissions = [
        Permission::ReadEntries {
            filter: "*.{jpg,jpeg,png,heic,raw}",
        },
        Permission::WriteSidecars {
            kinds: vec!["faces", "places", "scene"],
        },
        Permission::WriteTags,
        Permission::UseModel {
            category: "face_detection",
            local_only: true,
        },
        Permission::DispatchJobs,
    ]
)]
struct Photos {
    config: PhotosConfig,
}
```

### Runtime Enforcement

```rust
impl AgentContext<M> {
    pub fn in_granted_scope(&self, path: &str) -> bool {
        // Check if path matches any granted location
        for location in &self.permissions.scoped_locations {
            if path.starts_with(&location.path) {
                return true;
            }
        }
        false
    }

    fn check_permission(&self, perm: Permission) -> Result<()> {
        if !self.permissions.has(perm) {
            return Err(Error::PermissionDenied(format!(
                "Extension {} lacks permission: {:?}",
                self.extension_id, perm
            )));
        }
        Ok(())
    }
}
```

### User Grants

When installing extension, user sees:

```
┌────────────────────────────────────────────┐
│  Photos Extension Permissions              │
├────────────────────────────────────────────┤
│  This extension requests:                  │
│                                            │
│  ✓ Read image files (jpg, png, heic, raw) │
│  ✓ Detect faces using local AI models     │
│  ✓ Write face detection sidecars          │
│  ✓ Add tags to photos                     │
│  ✓ Run background analysis jobs           │
│                                            │
│  Grant access to locations:               │
│  [x] /Users/alice/Photos                  │
│  [ ] /Volumes/External/Family Photos      │
│  [ ] /Users/alice/Downloads               │
│                                            │
│  [Cancel]  [Install]                      │
└────────────────────────────────────────────┘
```

**Enforcement:** Every VDFS operation checks both permission type and location scope.

---

## Lifecycle Management

### Agent States

```rust
pub enum AgentState {
    Unloaded,        // Extension not loaded
    Initializing,    // Running on_startup
    Active,          // Normal operation
    Suspended,       // Temporarily paused
    Error(String),   // Error state
    ShuttingDown,    // Running cleanup
}
```

### Lifecycle Hooks

```rust
pub trait AgentLifecycle {
    /// Called when extension is loaded
    #[on_startup]
    async fn on_startup(&self, ctx: &AgentContext<Self::Memory>) -> Result<()>;

    /// Called when extension is disabled
    #[on_shutdown]
    async fn on_shutdown(&self, ctx: &AgentContext<Self::Memory>) -> Result<()>;

    /// Called when permissions change
    #[on_permission_change]
    async fn on_permission_change(&self, ctx: &AgentContext<Self::Memory>) -> Result<()>;
}
```

### State Transitions

```
User enables extension
  ↓
Extension loaded into WASM runtime
  ↓
AgentState::Initializing
  ↓
on_startup() hook called
  ↓
AgentState::Active
  ↓
Events routed to agent
  ↓
User disables extension
  ↓
AgentState::ShuttingDown
  ↓
on_shutdown() hook called
  ↓
AgentState::Unloaded
```

---

## Agent Trace System

Structured logging for debugging and auditing agent behavior.

### Trace API

```rust
impl AgentContext<M> {
    /// Log trace message
    pub fn trace(&self, message: impl Into<String>) {
        self.trace_handle.log(TraceLevel::Debug, message);
    }

    /// Structured trace with fields
    pub fn trace_with(&self, level: TraceLevel, message: impl Into<String>, fields: Fields) {
        self.trace_handle.log_structured(level, message, fields);
    }
}

pub enum TraceLevel {
    Debug,    // Verbose internal details
    Info,     // Normal operations
    Warn,     // Potential issues
    Error,    // Failures
}
```

### Trace Storage

```
.sdlibrary/
  └── sidecars/extension/{ext_id}/
      └── traces/
          ├── 2024-10-11.log     # Daily rotation
          ├── 2024-10-12.log
          └── current.log
```

### Trace Format

```json
{
  "timestamp": "2024-10-11T14:32:15.123Z",
  "level": "info",
  "agent": "com.spacedrive.photos",
  "hook": "on_event",
  "message": "New photo detected: IMG_1234.jpg",
  "fields": {
    "entry_id": "550e8400-e29b-41d4-a716-446655440000",
    "file_size": 2457600
  }
}
```

**UI Integration:** Traces viewable in extension settings panel.

---

## Notification System

Agents can notify users through multiple channels.

```rust
#[on_event(EntryCreated)]
async fn on_low_redundancy(&self, entry: Entry, ctx: &AgentContext<M>) -> Result<()> {
    // Check redundancy
    let copies = ctx.vdfs().count_copies(entry.content_id()).await?;

    if copies == 1 && entry.size() > 100_000_000 {  // 100MB
        ctx.notify()
            .title("Data at Risk")
            .message(format!(
                "File {} exists only on this device ({})",
                entry.name(),
                format_bytes(entry.size())
            ))
            .severity(NotificationSeverity::Warning)
            .action("Backup Now", BackupAction { entry_id: entry.id() })
            .send()
            .await?;
    }

    Ok(())
}
```

**Channels:**
- Desktop: Native OS notifications
- Mobile: Push notifications
- Web: Browser notifications
- All: In-app notification center

---

## Security & Sandboxing

### WASM Isolation

```rust
/// Extension runtime configuration
pub struct ExtensionRuntime {
    wasm_engine: wasmer::Engine,
    memory_limit: usize,      // Default: 256MB per extension
    cpu_limit: Duration,       // Default: 5s per event handler
    allowed_hosts: Vec<String>, // For HTTP tools
}
```

**Limits Enforced:**
- **Memory**: 256MB per extension (configurable)
- **CPU**: 5 seconds per event handler
- **Storage**: 1GB for memory/sidecars (configurable)
- **Network**: Only to allowed hosts in manifest

### Host Function Boundary

```rust
// WASM → Host function call
extern "C" fn host_vdfs_query(
    query_ptr: *const u8,
    query_len: usize,
) -> *mut u8 {
    // 1. Deserialize query from WASM memory
    let query: VdfsQuery = decode_from_wasm(query_ptr, query_len)?;

    // 2. Check permissions
    let extension_id = get_current_extension_id();
    let perms = get_permissions(extension_id);

    if !perms.can_read_entries() {
        return encode_error("Permission denied: ReadEntries");
    }

    // 3. Validate scope
    for entry in query.results() {
        if !perms.in_scope(&entry.path) {
            // Filter out entries not in granted scope
            continue;
        }
    }

    // 4. Execute and return
    let result = execute_vdfs_query(query).await?;
    encode_to_wasm(result)
}
```

**Safety Guarantees:**
- No direct memory access outside WASM
- All VDFS operations permission-checked
- No file system access outside library
- No network access except via HTTP proxy

---

## Agent Coordination

Multiple agents can coexist and coordinate through the VDFS.

### Coordination Patterns

#### 1. Shared State via VDFS

```rust
// Photos agent creates person
let person = Person {
    id: Uuid::new_v4(),
    name: Some("Alice".into()),
    photo_count: 10,
};
ctx.vdfs().create_model(person).await?;

// Finance agent queries same person for receipt categorization
let person = ctx.vdfs()
    .query_models::<Person>()
    .where_field("name", "Alice")
    .first()
    .await?;
```

#### 2. Event-Based Coordination

```rust
// Storage agent emits custom event
ctx.emit_event(CustomEvent::DuplicatesFound {
    duplicate_sets: vec![/* ... */],
})?;

// Photos agent can subscribe
#[on_event(CustomEvent::DuplicatesFound)]
async fn handle_duplicates(&self, sets: Vec<DuplicateSet>, ctx: &AgentContext<M>) -> Result<()> {
    // Coordinate cleanup
}
```

#### 3. Shared Memory (Advanced)

```rust
// Extensions can create shared knowledge
ctx.vdfs()
    .write_shared_data(
        "person_registry",
        &PersonRegistry { people: vec![...] }
    )
    .await?;

// Other extensions read
let registry = ctx.vdfs()
    .read_shared_data::<PersonRegistry>("person_registry")
    .await?;
```

---

## Extension Manifest

Complete specification for extension registration.

```toml
[extension]
id = "com.spacedrive.photos"
name = "Photos"
version = "1.0.0"
description = "Intelligent photo management with faces, places, and moments"
author = "Spacedrive"
license = "MIT"

[extension.requirements]
min_core_version = "2.0.0"
features = ["exif_extraction", "ai_models"]

[extension.permissions]
read_entries = { filter = "*.{jpg,jpeg,png,heic,raw}" }
write_sidecars = { kinds = ["faces", "places", "scene"] }
write_tags = true
write_custom_fields = { namespace = "photos" }
use_models = [
    { category = "face_detection", local_only = true },
    { category = "scene_classification", local_only = true },
]
dispatch_jobs = true

[extension.resources]
max_memory_mb = 512
max_storage_mb = 2048
allowed_hosts = ["api.openweathermap.org"]  # For geocoding

[extension.agent]
# Agent-specific configuration
enable_memory = true
memory_retention_days = 365
trace_level = "info"

[[extension.models]]
name = "face_detection"
version = "photos_v1"
source = { url = "https://models.spacedrive.com/photos/retinaface_v1.onnx" }
sha256 = "abc123..."
size_mb = 12

[[extension.models]]
name = "scene_classification"
version = "resnet50"
source = { url = "https://models.spacedrive.com/photos/resnet50_places365.onnx" }
sha256 = "def456..."
size_mb = 95
```

---

## Error Handling

### Agent Errors

```rust
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Entry not in granted scope: {0}")]
    ScopeViolation(String),

    #[error("Memory operation failed: {0}")]
    MemoryError(String),

    #[error("Job dispatch failed: {0}")]
    JobDispatchError(String),

    #[error("VDFS operation failed: {0}")]
    VdfsError(String),
}
```

### Error Recovery

```rust
#[on_event(EntryCreated)]
async fn on_new_photo(&self, entry: Entry, ctx: &AgentContext<M>) -> Result<()> {
    // Try to process
    match self.process_photo(&entry, ctx).await {
        Ok(_) => Ok(()),
        Err(e) if e.is_recoverable() => {
            // Log and continue
            ctx.trace(format!("Recoverable error: {}", e));
            Ok(())
        }
        Err(e) => {
            // Critical error - propagate
            ctx.trace_with(TraceLevel::Error, "Photo processing failed", fields! {
                "entry_id" => entry.id(),
                "error" => e.to_string(),
            });
            Err(e)
        }
    }
}
```

**Error Strategies:**
- **Recoverable errors**: Log and continue (don't crash agent)
- **Critical errors**: Propagate to runtime, enter error state
- **Retry logic**: Handled by task system, not agent
- **Circuit breaker**: Suspend agent after repeated failures

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use spacedrive_sdk::testing::*;

    #[tokio::test]
    async fn test_face_detection_flow() {
        // Create mock context
        let ctx = MockAgentContext::new()
            .with_vdfs(MockVdfs::new())
            .with_memory(PhotosMind::default())
            .build();

        // Simulate event
        let entry = Entry::new("test.jpg");
        let agent = PhotosAgent::new(PhotosConfig::default());

        agent.on_new_photo(entry, &ctx).await.unwrap();

        // Verify memory updated
        let memory = ctx.memory().read().await;
        assert_eq!(memory.plan.photos_pending.len(), 1);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_workflow() {
    // Setup real VDFS with test library
    let library = TestLibrary::new().await?;

    // Load extension
    let extension = library.load_extension("photos.wasm").await?;

    // Grant permissions
    extension.grant_location(library.location("/test/photos")).await?;

    // Trigger workflow
    library.index_location("/test/photos").await?;

    // Verify results
    tokio::time::sleep(Duration::from_secs(5)).await;

    let tags = library.get_tags_for_entry(test_entry_id).await?;
    assert!(tags.iter().any(|t| t.starts_with("#person:")));
}
```

---

## Performance Considerations

### Memory Efficiency

**Design Decisions:**
- Event log stored in SQLite (compressed, indexed)
- Vector embeddings use f16 precision (50% size reduction)
- Working memory kept minimal (< 1MB typical)
- Automatic compaction on schedule

**Benchmarks (Target):**
- Memory overhead: < 50MB per active agent
- Event processing: < 10ms per event
- Memory query: < 100ms for temporal, < 200ms for associative
- State persistence: < 50ms

### Async Execution

All agent code is async to prevent blocking:

```rust
// BAD - blocks WASM thread
fn process_photo(data: &[u8]) -> Result<()> {
    expensive_computation(data);  // Blocks!
    Ok(())
}

// GOOD - yields to runtime
async fn process_photo(data: &[u8], ctx: &TaskContext) -> Result<()> {
    let result = tokio::task::spawn_blocking(|| {
        expensive_computation(data)
    }).await?;

    ctx.check_interrupt().await?;  // Cooperate with runtime
    Ok(())
}
```

### Resource Limits

```rust
pub struct ResourceLimits {
    max_memory_bytes: usize,        // Default: 256MB
    max_cpu_time_per_event: Duration,  // Default: 5s
    max_storage_bytes: usize,       // Default: 1GB
    max_events_per_second: usize,   // Default: 100
}
```

**Enforcement:**
- WASM runtime tracks memory allocations
- CPU time measured per event handler invocation
- Storage quotas checked on writes
- Event rate limiting prevents DOS

---

## Implementation Plan

### Phase 1: Foundation (3-4 weeks)

**Goal:** Basic agent runtime with simple memory

**Deliverables:**
- [ ] `AgentContext` trait and implementation
- [ ] `AgentHandle` trait for extension registration
- [ ] Event subscription and routing
- [ ] `WorkingMemory` implementation (JSON file-backed)
- [ ] `#[on_startup]`, `#[on_shutdown]` hook macros
- [ ] Permission checking integration
- [ ] Trace system with file rotation

**Validation:**
- Simple test extension that counts file types
- Memory persists across restarts
- Events delivered reliably
- Permission violations caught

### Phase 2: Memory Systems (3-4 weeks)

**Goal:** Full memory implementation

**Deliverables:**
- [ ] `TemporalMemory` backed by SQLite
- [ ] FTS5 indexing for temporal queries
- [ ] `AssociativeMemory` backed by Vector Repository
- [ ] Embedding generation integration
- [ ] `AgentMemory` trait with persist/restore
- [ ] Memory query builders
- [ ] Automatic persistence (every 30s)
- [ ] Cleanup/retention policies

**Validation:**
- Photos extension (simplified) can store face clusters
- Temporal queries return correct events
- Associative similarity search works
- Memory survives process restarts

### Phase 3: Tools System (2-3 weeks)

**Goal:** LLM-callable tools

**Deliverables:**
- [ ] `Tool` trait with schema generation
- [ ] `#[tool]` proc macro
- [ ] `ToolContext` with VDFS/AI access
- [ ] `ToolResult` types
- [ ] Tool registry and discovery
- [ ] Integration with agent planner
- [ ] Error handling and validation

**Validation:**
- Extension can define tools
- LLM can discover and call tools
- Parameters validated against schema
- Results flow back to LLM

### Phase 4: Advanced Features (3-4 weeks)

**Goal:** Production-ready features

**Deliverables:**
- [ ] `#[on_event]` with filter expressions
- [ ] `#[scheduled]` cron-based triggers
- [ ] Multi-channel event broadcasting
- [ ] Extension-to-extension coordination
- [ ] Resource monitoring and limits
- [ ] Agent state visualization UI
- [ ] Extension marketplace integration

**Validation:**
- Photos extension fully functional
- Multiple agents coexist without conflicts
- Resource limits enforced
- UI shows agent activity

---

## Example: Complete Photos Agent

This shows the full implementation pattern:

```rust
#[extension(
    id = "com.spacedrive.photos",
    name = "Photos",
    version = "1.0.0",
    permissions = [
        Permission::ReadEntries { filter = "*.{jpg,jpeg,png,heic,raw}" },
        Permission::WriteSidecars { kinds = vec!["faces", "places"] },
        Permission::WriteTags,
        Permission::UseModel { category = "face_detection", local_only: true },
    ]
)]
struct Photos {
    config: PhotosConfig,
}

#[agent_memory]
struct PhotosMind {
    history: TemporalMemory<PhotoEvent>,
    knowledge: AssociativeMemory<PhotoKnowledge>,
    plan: WorkingMemory<AnalysisPlan>,
}

impl Photos {
    #[on_startup]
    async fn initialize(&self, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        ctx.trace("Photos agent initialized");

        // Ensure models available
        if !ctx.ai().has_model("face_detection:photos_v1") {
            ctx.trace("Face detection model not found - will download on first use");
        }

        Ok(())
    }

    #[on_event(EntryCreated)]
    #[filter = "entry.is_image() && ctx.in_granted_scope(entry.path())"]
    async fn on_photo_added(&self, entry: Entry, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        ctx.trace(format!("New photo: {}", entry.name()));

        // Add to analysis queue
        let should_dispatch = ctx.memory().update(|mut m| {
            m.plan.photos_pending.push(entry.id());
            Ok(m.plan.photos_pending.len() >= 50)
        }).await?;

        if should_dispatch {
            self.dispatch_batch_analysis(ctx).await?;
        }

        Ok(())
    }

    #[scheduled(cron = "0 9 * * SUN")]
    async fn weekly_memories(&self, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // Query photos from last week
        let photos = ctx.memory().read().await
            .history
            .query()
            .variant("PhotoAnalyzed")
            .since(Duration::days(7))
            .collect()
            .await?;

        if photos.len() > 10 {
            ctx.jobs()
                .dispatch(CreateMemories { photos })
                .await?;
        }

        Ok(())
    }

    #[tool("Search for photos of a specific person")]
    async fn search_person(
        &self,
        ctx: &ToolContext,
        person_name: String,
    ) -> ToolResult<Vec<Photo>> {
        let photos = ctx.vdfs()
            .query_entries()
            .with_tag(&format!("#person:{}", person_name))
            .collect()
            .await?;

        Ok(ToolResult::success(photos))
    }
}

// Helper methods
impl Photos {
    async fn dispatch_batch_analysis(&self, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        let batch = ctx.memory().update(|mut m| {
            let batch = m.plan.photos_pending.drain(..).collect();
            Ok((m, batch))
        }).await?;

        ctx.jobs()
            .dispatch(AnalyzePhotosBatch { photo_ids: batch })
            .priority(Priority::Low)
            .when_idle()
            .await?;

        Ok(())
    }
}
```

---

## Comparison with Design Inspirations

### What We Adopted

**From rust-deep-agents-sdk:**
- State snapshot pattern with smart merging
- Checkpointer trait for persistence
- Event system with tagged enums
- Tool schema generation approach

**From rust-agentai:**
- Simple, ergonomic API surface
- ToolBox pattern for grouped tools
- Minimal configuration needed

**From ccswarm:**
- Agent lifecycle state machine
- Multi-channel coordination patterns
- Resource monitoring concepts

### What Makes Spacedrive Unique

**1. VDFS-Native:**
- Agents observe file system directly, not through API calls
- Tight integration with indexer pipeline
- Content-aware operations (dedup, redundancy tracking)

**2. Extension-Based:**
- WASM sandboxing (not separate processes)
- Permission-scoped to library locations
- Marketplace distribution model

**3. Three-Memory Architecture:**
- Temporal (events), Associative (knowledge), Working (state)
- Each optimized for different access patterns
- Unified query interface

**4. Action System Integration:**
- All modifications go through transactional actions
- Preview-before-commit for safety
- Automatic audit trails

**5. Multi-Device Awareness:**
- Agents understand device topology
- Can dispatch jobs to specific devices
- Memory syncs across paired devices

---

## Security Considerations

### Threat Model

**Threats Mitigated:**
1. **Malicious Extension** - WASM sandbox prevents filesystem access
2. **Data Exfiltration** - Network requests proxied and logged
3. **Resource Exhaustion** - CPU/memory limits enforced
4. **Privilege Escalation** - Permissions checked on every operation
5. **Cross-Extension Access** - Each agent has isolated memory

**Assumptions:**
- Core VDFS is trusted
- WASM runtime is secure
- User grants permissions intentionally

### Security Best Practices

```rust
// 1. Always check scope before operations
if !ctx.in_granted_scope(&entry.path()) {
    return Err(AgentError::ScopeViolation(entry.path()));
}

// 2. Sanitize user inputs
let safe_name = sanitize_filename(&person_name);

// 3. Limit query results
let results = ctx.vdfs()
    .query_entries()
    .limit(1000)  // Prevent memory exhaustion
    .collect()
    .await?;

// 4. Validate before dispatching jobs
if photo_ids.len() > 10_000 {
    return Err(AgentError::InvalidInput("Too many photos"));
}

// 5. Never store credentials in memory
// Use core credential manager instead
let api_key = ctx.credentials().get("openweather_api_key").await?;
```

---

## Monitoring & Observability

### Agent Metrics

```rust
pub struct AgentMetrics {
    // Event handling
    events_received: u64,
    events_processed: u64,
    events_failed: u64,
    avg_event_latency_ms: f64,

    // Memory usage
    temporal_events_count: usize,
    knowledge_items_count: usize,
    memory_size_bytes: usize,

    // Job dispatch
    jobs_dispatched: u64,
    jobs_completed: u64,
    jobs_failed: u64,

    // Resource usage
    cpu_time_ms: u64,
    peak_memory_bytes: usize,
}
```

### Metrics Collection

```rust
// Auto-collected by runtime
impl ExtensionRuntime {
    async fn track_event_handler(&self, agent_id: &str, event: &VdfsEvent) {
        let start = Instant::now();

        // Execute handler
        let result = self.route_event(agent_id, event).await;

        // Record metrics
        self.metrics.record(agent_id, Metric::EventProcessed {
            event_type: event.variant_name(),
            duration: start.elapsed(),
            success: result.is_ok(),
        });
    }
}
```

### Observability UI

```
Extension Settings → Photos → Activity
┌─────────────────────────────────────────┐
│  Photos Agent Activity (Last 7 Days)    │
├─────────────────────────────────────────┤
│  Events Processed: 1,247             │
│  Faces Detected: 432                 │
│  Places Identified: 28               │
│  Memory Size: 45.2 MB                │
│                                          │
│  Recent Activity:                        │
│  • 2m ago - Analyzed batch of 50 photos │
│  • 1h ago - Created moment "Beach Day"  │
│  • 3h ago - Identified person "Alice"   │
│                                          │
│  [View Trace Log]  [Memory Inspector]   │
└─────────────────────────────────────────┘
```

---

## Migration Path for Existing Extensions

For extensions currently using simple job-based processing:

### Before (Job-Only Pattern)

```rust
// Old approach - direct job dispatch
#[extension_init]
async fn init(ctx: &ExtensionContext) -> Result<()> {
    // Poll for new photos periodically
    tokio::spawn(async move {
        loop {
            let photos = ctx.vdfs().query_entries()
                .of_type("image")
                .limit(50)
                .collect()
                .await?;

            process_photos(photos).await?;

            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });

    Ok(())
}
```

### After (Agent Pattern)

```rust
// New approach - event-driven agent
impl PhotosAgent {
    #[on_event(EntryCreated)]
    #[filter = "entry.is_image()"]
    async fn on_photo(&self, entry: Entry, ctx: &AgentContext<M>) -> Result<()> {
        // Automatic, real-time processing
        ctx.memory().update(|mut m| {
            m.plan.photos_pending.push(entry.id());
            Ok(m)
        }).await?;

        self.dispatch_if_ready(ctx).await
    }
}
```

**Benefits:**
- No polling overhead
- Real-time response to changes
- Persistent knowledge (not stateless)
- Better resource utilization

---

## Advanced Patterns

### Pattern 1: Progressive Analysis

```rust
impl PhotosAgent {
    #[on_event(EntryCreated)]
    async fn on_photo(&self, entry: Entry, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // Stage 1: Quick EXIF extraction (immediate)
        let exif = ctx.vdfs().get_sidecar::<ExifData>(entry.content_id(), "exif").await?;

        if let Some(gps) = exif.gps {
            // Stage 2: Queue for place identification (batched)
            ctx.memory().update(|mut m| {
                m.plan.photos_needing_places.push(entry.id());
                Ok(m)
            }).await?;
        }

        // Stage 3: Queue for face detection (GPU job, when idle)
        ctx.jobs()
            .dispatch(DetectFaces { photo_id: entry.id() })
            .when_idle()
            .on_device_with_capability(Capability::GPU)
            .await?;

        Ok(())
    }
}
```

### Pattern 2: Cross-Agent Collaboration

```rust
// Photos agent exposes knowledge
impl PhotosAgent {
    #[tool("Get photos taken at a specific location")]
    async fn photos_at_location(
        &self,
        ctx: &ToolContext,
        lat: f64,
        lon: f64,
        radius_km: f32,
    ) -> ToolResult<Vec<Photo>> {
        // Finance agent could call this to find receipt photos near a store
        // Storage agent could use this for location-based tiering

        let places = ctx.memory().read().await
            .knowledge
            .query()
            .variant("PlaceCluster")
            .where_nearby((lat, lon), radius_km)
            .collect()
            .await?;

        Ok(ToolResult::success(places))
    }
}
```

### Pattern 3: Adaptive Behavior

```rust
impl PhotosAgent {
    async fn should_analyze_now(&self, ctx: &AgentContext<PhotosMind>) -> bool {
        let memory = ctx.memory().read().await;

        // Check historical patterns
        let recent_activity = memory.history
            .query()
            .variant("PhotoAnalyzed")
            .since(Duration::hours(24))
            .count()
            .await
            .unwrap_or(0);

        // Adapt based on user behavior
        if recent_activity > 100 {
            // User is actively organizing - process immediately
            true
        } else {
            // Low activity - wait for batch
            memory.plan.photos_pending.len() >= 50
        }
    }
}
```

---

## Future Enhancements

### Agent-to-Agent Communication

```rust
// Proposal: Shared knowledge bus
ctx.knowledge_bus()
    .publish("person_identified", PersonIdentifiedEvent {
        person_id,
        photo_ids,
    })
    .await?;

// Other agents subscribe
#[on_knowledge_event("person_identified")]
async fn on_person_identified(&self, event: PersonIdentifiedEvent, ctx: &AgentContext<M>) -> Result<()> {
    // Finance agent could use this to categorize receipts by person
}
```

### Federated Learning

```rust
// Agents can share models without sharing data
ctx.ai()
    .contribute_to_federated_model(
        "face_clustering:global",
        local_updates,
    )
    .await?;
```

### Agent Composition

```rust
// One agent can delegate to another
#[tool("Organize vacation photos")]
async fn organize_vacation(&self, ctx: &ToolContext, location: String) -> ToolResult<()> {
    // Photos agent identifies photos
    let photos = self.find_vacation_photos(location, ctx).await?;

    // Delegate to Storage agent for tiering
    ctx.tools()
        .call_external("com.spacedrive.storage", "suggest_tiering", json!({
            "entry_ids": photos.iter().map(|p| p.id).collect::<Vec<_>>(),
        }))
        .await?;

    Ok(ToolResult::success(()))
}
```

---

## Appendix A: SDK API Reference

### Core Types

```rust
// Agent definition
pub trait ExtensionAgent: Send + Sync {
    type Memory: AgentMemory;

    fn config(&self) -> &Self::Config;
}

// Memory bound
pub trait AgentMemory: Send + Sync + 'static {
    async fn persist(&self, path: &Path) -> Result<()>;
    async fn restore(path: &Path) -> Result<Self>;
}

// Event trait
pub trait Event: Serialize + DeserializeOwned + Clone {
    fn timestamp(&self) -> DateTime<Utc>;
    fn variant_name(&self) -> &'static str;
}

// Knowledge trait (for associative memory)
pub trait Knowledge: Serialize + DeserializeOwned + Clone {
    fn variant_name(&self) -> &'static str;
    fn generate_embedding(&self) -> Result<Vec<f32>>;
}
```

### Attribute Macros

```rust
#[extension(/* config */)]        // Extension definition
#[agent_memory]                   // Memory trait impl
#[on_startup]                     // Lifecycle hook
#[on_shutdown]                    // Lifecycle hook
#[on_event(VdfsEvent)]           // Event handler
#[scheduled(cron = "...")]       // Scheduled task
#[tool("description")]           // LLM-callable tool
#[filter = "expression"]         // Event filter
```

---

## Appendix B: Comparison Matrix

| Feature | ccswarm | rust-agentai | rust-deep-agents | Spacedrive |
|---------|---------|--------------|------------------|------------|
| **Execution Model** | Multi-process | Single-process | Single-process | WASM sandbox |
| **State Persistence** | Session files | None | Checkpointer trait | Memory systems |
| **Event System** | Message bus | None | Event broadcaster | VDFS events |
| **Tool Definition** | Manual | Macro | Macro | Macro |
| **LLM Integration** | Claude Code | GenAI lib | Multiple | GenAI lib |
| **Multi-Agent** | Yes | No | Sub-agents | Cross-extension |
| **HITL** | Auto-accept | No | Per-tool policies | Action preview |
| **Memory Types** | 1 (whiteboard) | 0 | 1 (snapshot) | 3 (T/A/W) |

**Spacedrive Advantages:**
- Tighter integration with core (VDFS-native)
- Richer memory model (three types)
- Better isolation (WASM vs processes)
- Built-in permission system

---

## Appendix C: File Locations

```
.sdlibrary/
  └── sidecars/
      └── extension/
          └── {extension_id}/
              ├── memory/
              │   ├── temporal.db          # TemporalMemory (SQLite)
              │   ├── knowledge.vss        # AssociativeMemory (Vector Store)
              │   └── working.json         # WorkingMemory (JSON)
              ├── traces/
              │   ├── 2024-10-11.log       # Daily trace logs
              │   └── current.log
              └── cache/
                  └── model_cache/          # Downloaded models
```

---

## Success Criteria

**Phase 1 Complete When:**
- [ ] Test extension can subscribe to events
- [ ] Working memory persists across restarts
- [ ] Permission violations are caught
- [ ] Trace logs are viewable in UI

**Phase 2 Complete When:**
- [ ] Photos extension stores face clusters
- [ ] Temporal queries return correct results
- [ ] Associative similarity search works
- [ ] Memory survives core restarts

**Phase 3 Complete When:**
- [ ] LLM can discover and call tools
- [ ] Tool parameters validated
- [ ] Tool results properly formatted
- [ ] Extensions can define custom tools

**Phase 4 Complete When:**
- [ ] Photos extension fully functional
- [ ] Multiple agents run concurrently
- [ ] Resource limits enforced
- [ ] Extension marketplace launched

---

**This design leverages proven patterns from production Rust agent frameworks while creating a system uniquely suited to Spacedrive's VDFS architecture, extension model, and privacy-first philosophy.**

