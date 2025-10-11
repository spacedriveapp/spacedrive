# Agent Architecture Analysis from Production Rust Projects

This document analyzes three production Rust AI agent frameworks to extract patterns and best practices for Spacedrive's extension-based agent system.

## Projects Analyzed

1. **ccswarm** (v0.3.7) - Multi-agent orchestration system
2. **rust-agentai** (0.1.5) - Lightweight agent library with tool support
3. **rust-deep-agents-sdk** (0.0.1) - Deep agents with middleware and HITL

---

## Key Architectural Patterns

### 1. **Agent Core Traits**

All three projects use trait-based abstractions for agents:

#### rust-deep-agents-sdk Pattern (Most Comprehensive)
```rust
#[async_trait]
pub trait AgentHandle: Send + Sync {
    async fn describe(&self) -> AgentDescriptor;

    async fn handle_message(
        &self,
        input: AgentMessage,
        state: Arc<AgentStateSnapshot>,
    ) -> anyhow::Result<AgentMessage>;

    async fn handle_message_stream(
        &self,
        input: AgentMessage,
        state: Arc<AgentStateSnapshot>,
    ) -> anyhow::Result<AgentStream>;

    async fn current_interrupt(&self) -> anyhow::Result<Option<AgentInterrupt>>;
    async fn resume_with_approval(&self, action: HitlAction) -> anyhow::Result<AgentMessage>;
}
```

**Key Insights:**
- **Streaming support** is first-class (not an afterthought)
- **Interrupt/HITL** baked into core trait (for human-in-the-loop)
- **State is immutable Arc** - agents don't own state
- **Async all the way** - no blocking operations

#### ccswarm Pattern (Status Machine)
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Initializing,
    Available,
    Working,
    WaitingForReview,
    Error(String),
    ShuttingDown,
}
```

**Key Insight:** Explicit lifecycle states make debugging easier and enable better orchestration.

---

### 2. **State Management**

#### rust-deep-agents-sdk Approach (Best Practice)

```rust
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AgentStateSnapshot {
    pub todos: Vec<TodoItem>,
    pub files: BTreeMap<String, String>,
    pub scratchpad: BTreeMap<String, serde_json::Value>,
    pub pending_interrupts: Vec<AgentInterrupt>,
}

impl AgentStateSnapshot {
    // Smart merging with domain-specific logic
    pub fn merge(&mut self, other: AgentStateSnapshot) {
        self.files.extend(other.files);  // Dictionary merge
        if !other.todos.is_empty() {
            self.todos = other.todos;     // Replace if non-empty
        }
        self.scratchpad.extend(other.scratchpad);
    }
}
```

**Key Insights:**
- State is **immutable snapshot** (not live reference)
- **BTreeMap for deterministic ordering** (important for replays)
- **Custom merge logic** per field type
- **Scratchpad pattern**: Generic JSON storage for flexible state

**Spacedrive Application:**
```rust
// For Photos extension
pub struct PhotosMind {
    history: TemporalMemory<PhotoEvent>,       // Append-only log
    knowledge: AssociativeMemory<PhotoKnowledge>,  // Vector storage
    plan: WorkingMemory<AnalysisPlan>,         // Transactional state
}
```

---

### 3. **Persistence/Checkpointing**

#### rust-deep-agents-sdk Pattern (Multi-Backend)

```rust
#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn save_state(&self, thread_id: &ThreadId, state: &AgentStateSnapshot) -> Result<()>;
    async fn load_state(&self, thread_id: &ThreadId) -> Result<Option<AgentStateSnapshot>>;
    async fn delete_thread(&self, thread_id: &ThreadId) -> Result<()>;
    async fn list_threads(&self) -> Result<Vec<ThreadId>>;
}
```

**Implementations:**
- `InMemoryCheckpointer` - Development/testing
- `RedisCheckpointer` - Fast, ephemeral
- `PostgresCheckpointer` - Durable, queryable
- `DynamoDbCheckpointer` - AWS-native

**Key Insights:**
- **Thread-based scoping** (not global state)
- **Optional trait** (agents work without persistence)
- **Simple CRUD interface** (no complex queries)

**Spacedrive Application:**
```rust
// Store in .sdlibrary/sidecars/extension/photos/memory/
pub trait AgentMemory {
    async fn save(&self, path: &Path) -> Result<()>;
    async fn load(path: &Path) -> Result<Self>;
}
```

---

### 4. **Event System**

#### rust-deep-agents-sdk Pattern (Production-Grade)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AgentEvent {
    AgentStarted(AgentStartedEvent),
    AgentCompleted(AgentCompletedEvent),
    ToolStarted(ToolStartedEvent),
    ToolCompleted(ToolCompletedEvent),
    ToolFailed(ToolFailedEvent),
    SubAgentStarted(SubAgentStartedEvent),
    SubAgentCompleted(SubAgentCompletedEvent),
    TodosUpdated(TodosUpdatedEvent),
    StateCheckpointed(StateCheckpointedEvent),
    PlanningComplete(PlanningCompleteEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub thread_id: String,
    pub correlation_id: String,
    pub customer_id: Option<String>,
    pub timestamp: String,
}
```

**Event Broadcasting:**
```rust
#[async_trait]
pub trait EventBroadcaster: Send + Sync {
    fn id(&self) -> &str;
    async fn broadcast(&self, event: &AgentEvent) -> anyhow::Result<()>;
}

pub struct EventDispatcher {
    broadcasters: RwLock<Vec<Arc<dyn EventBroadcaster>>>,
}
```

**Key Insights:**
- **Tagged enums** for type-safe events
- **Metadata on every event** (correlation IDs crucial)
- **Multi-channel broadcasting** (console, WhatsApp, SSE, etc.)
- **PII sanitization by default** (security first)

**Spacedrive Application:**
```rust
pub enum ExtensionEvent {
    JobStarted { job_id: Uuid, job_type: String },
    TaskCompleted { task_id: Uuid, result: TaskResult },
    MemoryUpdated { agent_id: String, memory_type: MemoryType },
}
```

---

### 5. **Tool System**

> **Spacedrive Status:** ️ **Not yet implemented** - Tools system needs to be added to SDK

#### rust-deep-agents-sdk Macro Pattern (Ergonomic)

```rust
#[tool("Adds two numbers together")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// Auto-generates:
pub struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn schema(&self) -> ToolSchema { /* auto-generated */ }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult> {
        // auto-extracts parameters
        let a: i32 = args.get("a")...;
        let b: i32 = args.get("b")...;
        Ok(ToolResult::text(&ctx, add(a, b)))
    }
}
```

**Key Insights:**
- **Proc macro magic** - zero boilerplate
- **JSON Schema generation** from Rust types
- **Async support** out of the box
- **Optional parameters** via `Option<T>`

#### rust-agentai ToolBox Pattern

```rust
#[toolbox]
impl MyTools {
    async fn search(&self, query: String) -> String {
        // Implementation
    }

    async fn fetch(&self, url: String) -> String {
        // Implementation
    }
}

// Usage:
let toolbox = MyTools::new();
agent.run("gpt-4", "Search for Rust", Some(&toolbox)).await?;
```

**Key Insights:**
- **Grouped tools** in impl blocks
- **Shared state** via `&self`
- **Context access** for calling external services

**Spacedrive Application:**

> **Note:** Spacedrive currently doesn't have an AI agent "tools" system. The current SDK has:
> - **Tasks** - Units of work within durable jobs (for resumability/checkpointing)
> - **Jobs** - Long-running operations that can be paused/resumed
>
> **Tools** in the AI agent sense (LLM-callable functions with JSON schemas) need to be added to the SDK.

```rust
// FUTURE: Tools system to be added to Spacedrive SDK
// This shows the intended API after tools are implemented

// In Photos extension
#[tool("Detects faces in a photo and returns bounding boxes with embeddings")]
async fn detect_faces(ctx: &ToolContext, photo_id: Uuid) -> ToolResult<Vec<FaceDetection>> {
    let photo = ctx.vdfs().get_entry(photo_id).await?;
    let image_bytes = photo.read().await?;

    let detections = ctx.ai()
        .from_registered("face_detection:photos_v1")
        .detect_faces(&image_bytes)
        .await?;

    Ok(ToolResult::success(detections))
}

// Meanwhile, tasks remain for durable job execution:
#[task(retries = 2, timeout_ms = 30000)]
async fn analyze_photos_batch(ctx: &TaskContext, photo_ids: &[Uuid]) -> TaskResult<()> {
    // This is about resumability, not LLM tool calling
    for photo_id in photo_ids {
        // Process photo
        ctx.checkpoint().await?;  // Save progress
    }
    Ok(())
}
```

**TODO for SDK Implementation:**
- [ ] Add `Tool` trait with `schema()` method (returns JSON Schema)
- [ ] Add `#[tool]` proc macro for automatic schema generation
- [ ] Add `ToolContext` with access to VDFS, AI models, permissions
- [ ] Add `ToolResult` type for success/error responses
- [ ] Integrate with agent runtime for tool discovery and execution
- [ ] Add tool registry for listing available tools to LLM

---

### 6. **Middleware Pattern**

#### rust-deep-agents-sdk Pattern (Powerful)

```rust
#[async_trait]
pub trait AgentMiddleware: Send + Sync {
    fn id(&self) -> &'static str;

    fn tools(&self) -> Vec<ToolBox> { Vec::new() }

    async fn modify_model_request(&self, ctx: &mut MiddlewareContext<'_>) -> Result<()>;

    async fn before_tool_execution(
        &self,
        tool_name: &str,
        tool_args: &Value,
        call_id: &str,
    ) -> Result<Option<AgentInterrupt>>;
}
```

**Built-in Middleware:**
- `SummarizationMiddleware` - Context window management
- `PlanningMiddleware` - Todo list management
- `FilesystemMiddleware` - Mock filesystem
- `SubAgentMiddleware` - Task delegation
- `HitlMiddleware` - Human-in-the-loop approvals

**Key Insights:**
- **Composable layers** like HTTP middleware
- **Request/response interception**
- **Tool injection** per middleware
- **Interrupt hooks** for approval flows

**Spacedrive Application:**
```rust
pub trait ExtensionMiddleware {
    async fn on_event(&self, event: &VdfsEvent, ctx: &AgentContext) -> Result<()>;
    async fn before_action(&self, action: &Action, ctx: &AgentContext) -> Result<Option<Interrupt>>;
    async fn after_job(&self, job: &JobResult, ctx: &AgentContext) -> Result<()>;
}
```

---

### 7. **Builder Pattern**

#### rust-deep-agents-sdk Pattern (Fluent API)

```rust
let agent = ConfigurableAgentBuilder::new("You are a helpful assistant")
    .with_openai_chat(OpenAiConfig::new(api_key, "gpt-4o"))?
    .with_tool(AddTool::as_tool())
    .with_tool(SearchTool::as_tool())
    .with_subagent_config(researcher_config)
    .with_summarization(SummarizationConfig::new(10, "..."))
    .with_tool_interrupt("delete_file", HitlPolicy {
        allow_auto: false,
        note: Some("Requires approval".into()),
    })
    .with_checkpointer(Arc::new(InMemoryCheckpointer::new()))
    .with_event_broadcaster(Arc::new(ConsoleLogger))
    .with_pii_sanitization(true)
    .build()?;
```

**Key Insights:**
- **Progressive disclosure** - simple cases easy, complex possible
- **Type-safe chaining** - compiler catches errors
- **Optional components** - checkpointer, events, etc.
- **Convenience methods** - `with_openai_chat` vs manual model creation

**Spacedrive Application:**
```rust
#[extension(
    id = "com.spacedrive.photos",
    name = "Photos",
    permissions = [
        Permission::ReadEntries,
        Permission::WriteSidecars(kinds = vec!["faces"]),
        Permission::UseModel(category = "face_detection"),
    ]
)]
struct Photos {
    config: PhotosConfig,
}
```

---

### 8. **Human-in-the-Loop (HITL)**

#### rust-deep-agents-sdk Pattern (Critical Feature)

```rust
pub struct HitlPolicy {
    pub allow_auto: bool,      // Auto-execute or require approval
    pub note: Option<String>,  // Why approval needed
}

pub enum HitlAction {
    Accept,                              // Execute as-is
    Edit { tool_name: String, tool_args: Value },  // Modify then execute
    Reject { reason: Option<String> },   // Cancel execution
    Respond { message: AgentMessage },   // Custom response
}

pub struct AgentInterrupt {
    pub tool_name: String,
    pub tool_args: Value,
    pub call_id: String,
    pub policy_note: Option<String>,
}
```

**Flow:**
```rust
// Agent tries to call tool
match agent.handle_message("Delete old files", state).await {
    Err(e) if e.contains("HITL interrupt") => {
        // Show user: tool name, args, note
        let interrupt = agent.current_interrupt().await?;

        // User approves
        agent.resume_with_approval(HitlAction::Accept).await?;
    }
    Ok(response) => // Normal completion
}
```

**Key Insights:**
- **Tool-level policies** (not global)
- **Four response types** (not just yes/no)
- **Requires checkpointer** (for state persistence)
- **Security best practice** for critical operations

**Spacedrive Application:**
```rust
// In Photos extension
#[action]
async fn batch_delete_faces(ctx: &ActionContext, face_ids: Vec<Uuid>) -> ActionResult {
    // Spacedrive's Action System provides preview-before-commit
    // Similar to HITL but at action level, not tool level
}
```

---

### 9. **Agent Lifecycle Management**

#### ccswarm Pattern (Rich Status Model)

```rust
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub identity: AgentIdentity,
    pub workspace: PathBuf,
    pub personality: Option<AgentPersonality>,
    pub phronesis: PhronesisManager,  // Practical wisdom from experience
}

impl Agent {
    pub async fn initialize(&mut self) -> Result<()> {
        self.status = AgentStatus::Initializing;
        // Setup workspace, load identity, etc.
        self.status = AgentStatus::Available;
    }

    pub async fn execute_task(&mut self, task: Task) -> Result<TaskResult> {
        self.status = AgentStatus::Working;
        // Boundary checking, phronesis consultation
        let result = self.perform_work(task).await?;
        self.status = AgentStatus::WaitingForReview;
        Ok(result)
    }
}
```

**Key Insights:**
- **Identity system** - agents have consistent personas
- **Phronesis (wisdom)** - learning from past experiences
- **Boundary checking** - agents know their limits
- **Personality traits** - affect decision-making style

---

### 10. **Memory Patterns**

#### ccswarm Whiteboard Pattern

```rust
pub struct Whiteboard {
    entries: Vec<WhiteboardEntry>,
}

pub struct WhiteboardEntry {
    pub entry_type: EntryType,
    pub content: String,
    pub annotations: Vec<AnnotationMarker>,
    pub timestamp: DateTime<Utc>,
    pub agent_id: Option<String>,
}

pub enum EntryType {
    TaskDescription,
    CodeSnippet,
    DesignDecision,
    ErrorReport,
    Solution,
}
```

**Key Insight:** Shared workspace for multi-agent collaboration.

#### Spacedrive's Memory System (From SDK)

```rust
pub struct TemporalMemory<T> {
    // Append-only event log
}

pub struct AssociativeMemory<T> {
    // Vector store with semantic search
}

pub struct WorkingMemory<T> {
    // Transactional current state
}
```

**Key Difference:** Domain-specific memory types vs. generic storage.

---

## Security Best Practices

### 1. PII Sanitization (rust-deep-agents-sdk)

```rust
pub fn sanitize_tool_payload(payload: &Value, max_len: usize) -> String {
    let sanitized = sanitize_json(payload);  // Redact sensitive fields
    let text = serde_json::to_string(&sanitized).unwrap();
    let redacted = redact_pii(&text);        // Remove email, phone, CC#
    safe_preview(&redacted, max_len)         // Truncate
}

// Pattern matching for PII
const EMAIL_REGEX: &str = r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b";
const PHONE_REGEX: &str = r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b";
const CREDIT_CARD_REGEX: &str = r"\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b";
```

**Key Insight:** Enabled by default, explicit opt-out required.

### 2. Sandboxing (All Projects)

- **WASM isolation** for untrusted extensions
- **Permission systems** for file/network access
- **Resource limits** (memory, CPU, time)

---

## Performance Patterns

### 1. Zero-Cost Abstractions (ccswarm)

```rust
// Type-state pattern - compile-time validation
pub struct TaskBuilder<State> {
    task: Task,
    _phantom: PhantomData<State>,
}

impl TaskBuilder<Initial> {
    pub fn with_description(self, desc: String) -> TaskBuilder<HasDescription> { }
}

impl TaskBuilder<HasDescription> {
    pub fn build(self) -> Task { self.task }
}
```

**Key Insight:** Rust's type system prevents runtime errors.

### 2. Channel-Based Concurrency (ccswarm)

```rust
// No Arc<Mutex<T>> - use message passing
let (tx, rx) = mpsc::channel(100);

// Agent task executor
tokio::spawn(async move {
    while let Some(task) = rx.recv().await {
        process_task(task).await;
    }
});
```

**Key Insight:** Lock-free coordination for multi-agent systems.

---

## Recommended Architecture for Spacedrive

### Core Agent Trait

```rust
#[async_trait]
pub trait ExtensionAgent: Send + Sync {
    // Identity
    fn descriptor(&self) -> AgentDescriptor;

    // Lifecycle
    async fn on_startup(&self, ctx: &AgentContext<Self::Memory>) -> AgentResult<()>;
    async fn on_shutdown(&self, ctx: &AgentContext<Self::Memory>) -> AgentResult<()>;

    // Event handling
    async fn on_event(&self, event: VdfsEvent, ctx: &AgentContext<Self::Memory>) -> AgentResult<()>;

    // Scheduled tasks
    async fn on_schedule(&self, trigger: ScheduleTrigger, ctx: &AgentContext<Self::Memory>) -> AgentResult<()>;

    // Associated types
    type Memory: AgentMemory;
}
```

### Agent Context (Immutable)

```rust
pub struct AgentContext<M: AgentMemory> {
    vdfs: VdfsContext,              // Read-only VDFS access
    ai: AiContext,                  // Model inference
    jobs: JobDispatcher,            // Background jobs
    memory: MemoryHandle<M>,        // Persistent memory
    permissions: PermissionSet,     // Granted scopes
    _phantom: PhantomData<M>,
}
```

### Memory System

```rust
#[agent_memory]
struct PhotosMind {
    history: TemporalMemory<PhotoEvent>,          // Append-only events
    knowledge: AssociativeMemory<PhotoKnowledge>, // Vector search
    plan: WorkingMemory<AnalysisPlan>,           // Transactional state
}

impl Checkpointer for PhotosMind {
    async fn save(&self, path: &Path) -> Result<()> {
        // Serialize to .sdlibrary/sidecars/extension/photos/memory/
    }
}
```

### Event System

```rust
pub enum ExtensionEvent {
    AgentStarted { agent_id: String, timestamp: DateTime<Utc> },
    JobDispatched { job_id: Uuid, job_type: String },
    MemoryUpdated { agent_id: String, size_bytes: usize },
    ActionProposed { action: ActionPreview },
}

pub struct EventBroadcaster {
    channels: Vec<Arc<dyn EventChannel>>,
}

impl EventBroadcaster {
    pub async fn emit(&self, event: ExtensionEvent) -> Result<()> {
        for channel in &self.channels {
            channel.send(&event).await?;
        }
        Ok(())
    }
}
```

### Tools System (To Be Implemented)

> **Current State:** Spacedrive has **Tasks** and **Jobs** for durable execution, but not **Tools** for LLM interaction.

**Distinction:**
- **Tasks** = Work units in resumable jobs (existing)
- **Jobs** = Long-running operations with checkpoints (existing)
- **Tools** = LLM-callable functions with JSON schemas (needs implementation)

```rust
// EXISTING: Tasks for durable job execution
#[task(
    retries = 2,
    timeout_ms = 30000,
    requires_capability = "gpu_optional"
)]
async fn detect_faces_batch(ctx: &TaskContext, photo: &Entry) -> TaskResult<Vec<Face>> {
    let image = photo.read().await?;
    let model = ctx.ai().model("face_detection:photos_v1");
    model.detect(&image).await
}

#[job(name = "analyze_photos_batch")]
fn analyze_photos(ctx: &JobContext, state: &mut AnalyzeState) -> JobResult<()> {
    for photo_id in &state.photo_ids {
        ctx.run(detect_faces_batch, (photo_id,)).await?;
        ctx.checkpoint().await?;  // Resumable
    }
    Ok(())
}

// TO BE ADDED: Tools for LLM interaction
#[tool("Search for photos by person name")]
async fn search_photos_by_person(
    ctx: &ToolContext,
    person_name: String,
    max_results: Option<u32>
) -> ToolResult<Vec<PhotoMetadata>> {
    // LLM can call this function
    let photos = ctx.vdfs()
        .query_entries()
        .with_tag(&format!("#person:{}", person_name))
        .limit(max_results.unwrap_or(20))
        .collect()
        .await?;

    Ok(ToolResult::success(photos))
}

// Tools get registered with agent's planner (LLM)
let agent = AgentBuilder::new("Photo assistant")
    .with_tool(SearchPhotosByPersonTool::as_tool())  // Auto-generated
    .with_tool(AnalyzeFacesTool::as_tool())
    .build()?;
```

**Implementation Priority:**
1. **Phase 1:** Basic tool trait + manual registration
2. **Phase 2:** `#[tool]` macro for automatic schema generation
3. **Phase 3:** Tool discovery and dynamic loading
4. **Phase 4:** Tool composition and chaining

---

## Key Takeaways for Implementation

### 1. **Start Simple, Add Complexity**
- Begin with `InMemoryCheckpointer` (like rust-deep-agents-sdk)
- Add Redis/Postgres later when needed
- Event system can start with basic logging

### 2. **Leverage Proc Macros**
- `#[tool]` for zero-boilerplate tools ️ **TODO: Needs implementation**
- `#[agent]` for lifecycle registration ️ **TODO: Needs implementation**
- `#[agent_memory]` for persistence trait ️ **TODO: Needs implementation**
- `#[task]` already exists for durable jobs ✅
- `#[job]` already exists for job registration ✅

### 3. **State Management**
- Immutable snapshots (Arc<AgentStateSnapshot>)
- Custom merge logic per field
- BTreeMap for determinism

### 4. **Event-Driven Architecture**
- Tagged enums for type safety
- Multi-channel broadcasting
- PII sanitization by default

### 5. **Security First**
- WASM sandboxing
- Explicit permissions
- HITL for critical operations
- Resource limits

### 6. **Testing Strategy**
- Unit tests for memory merge logic
- Integration tests with mock VDFS
- Property tests for state reducers
- Minimal tests for speed (ccswarm: 8 essential tests)

### 7. **Documentation**
- Comprehensive examples (like rust-deep-agents-sdk)
- Migration guides
- Architecture decision records (ADRs)

---

## Implementation Phases

### Phase 1: Core Agent Runtime (2-3 weeks)
- [ ] `AgentContext` and `AgentHandle` traits
- [ ] `InMemoryCheckpointer` for state persistence
- [ ] Basic event system (console logging)
- [ ] Macro for `#[agent]` lifecycle hooks

### Phase 2: Memory System (2-3 weeks)
- [ ] `TemporalMemory` implementation (SQLite event log)
- [ ] `AssociativeMemory` implementation (vector store)
- [ ] `WorkingMemory` implementation (transactional state)
- [ ] Persistence to `.sdlibrary/sidecars/extension/`

### Phase 3: Tools System (2-3 weeks)
> **New system to add - distinct from existing Tasks/Jobs**

- [ ] `Tool` trait with `schema()` method
- [ ] `#[tool]` proc macro for automatic schema generation
- [ ] `ToolContext` providing VDFS/AI/permissions access
- [ ] `ToolResult` type for structured responses
- [ ] Tool registry for discovery
- [ ] Integration with agent planner (LLM) for tool calling
- [ ] Tool execution runtime with error handling

**Note:** Tasks/Jobs already exist for durable execution and don't need changes.

### Phase 4: Advanced Features (3-4 weeks)
- [ ] Multi-channel event broadcasting
- [ ] Middleware system for interception
- [ ] HITL-style approval for actions
- [ ] Performance optimizations

---

## References

1. **rust-deep-agents-sdk**: https://github.com/yafatek/rust-deep-agents-sdk
   - Best: State management, checkpointing, HITL, events
   - Use: Builder pattern, middleware architecture

2. **rust-agentai**: https://github.com/asm-jaime/rust-agentai
   - Best: Simple API, ToolBox pattern, MCP integration
   - Use: Macro design inspiration

3. **ccswarm**: https://github.com/nwiizo/ccswarm
   - Best: Multi-agent orchestration, status lifecycle, phronesis
   - Use: Agent identity, boundary checking, whiteboard pattern

All three are production-quality codebases with valuable patterns for Spacedrive's agent system.

---

## Appendix: Spacedrive SDK Implementation Status

### What Exists Today

**Job System:**
- `#[job]` macro for durable, long-running operations
- `JobContext` with progress reporting and checkpointing
- Job queue with pause/resume capability
- Integration with core event bus

**Task System:**
- `#[task]` macro for work units within jobs
- Task execution with retries and timeouts
- Capability-based scheduling (GPU, CPU)
- Error handling and propagation

**Extension Framework:**
- `#[extension]` macro with permissions
- WASM runtime (in design phase)
- Permission scoping to locations
- Model registration

### ️ What Needs Implementation

**Tools System (New):**
- `Tool` trait with JSON Schema generation
- `#[tool]` proc macro
- `ToolContext` for VDFS/AI access
- Tool registry for LLM discovery
- Tool execution runtime

**Agent Runtime:**
- `AgentContext` implementation (currently stubs)
- Event subscription mechanism
- Lifecycle hooks (`on_startup`, `on_event`, `scheduled`)
- Memory persistence backends

**Memory System:**
- `TemporalMemory` backend (SQLite event log)
- `AssociativeMemory` backend (vector store)
- `WorkingMemory` backend (transactional JSON)
- Query interfaces implementation

**Event System:**
- Extension event types
- Multi-channel broadcasting
- Event correlation IDs
- PII sanitization

### Quick Implementation Guide

**If implementing tools first (recommended):**

1. Study `rust-deep-agents-sdk/crates/agents-macros/src/lib.rs` - copy the `#[tool]` macro
2. Study `rust-deep-agents-sdk/crates/agents-core/src/tools.rs` - adapt the Tool trait
3. Create `crates/sdk/src/tools.rs` with Tool trait and ToolContext
4. Create `crates/sdk-macros/src/tool.rs` with proc macro
5. Add tests in `extensions/test-extension` to validate

**Key files to study:**
- Tool macro: `rust-deep-agents-sdk/crates/agents-macros/src/lib.rs` (260 lines)
- Tool trait: `rust-deep-agents-sdk/crates/agents-core/src/tools.rs` (368 lines)
- Builder: `rust-deep-agents-sdk/crates/agents-runtime/src/agent/builder.rs` (317 lines)

**Estimated effort:**
- Basic tool system: ~500 lines, 1 week
- With proc macro: ~800 lines, 2 weeks
- With registry and integration: ~1200 lines, 3 weeks

