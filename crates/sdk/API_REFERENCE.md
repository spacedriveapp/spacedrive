# Spacedrive SDK API Reference

**Status:** Stubs for type-checking (implementations are `todo!()`)
**Purpose:** Full API surface from `docs/sdk/sdk.md` - allows extensions to compile

---

## Overview

The SDK now includes **all APIs** documented in the specification as type-checked stubs.

### Modules

```
spacedrive-sdk/
├── actions.rs       - Action preview/execute system
├── agent.rs         - Agent context and memory
├── ai.rs            - AI models and inference
├── ffi.rs           - Low-level WASM imports (existing)
├── job_context.rs   - Job execution context (expanded)
├── models.rs        - Model registration
├── query.rs         - Query context
├── tasks.rs         - Task execution context
├── types.rs         - Common types (expanded)
└── vdfs.rs          - VDFS queries and operations
```

---

## Core Types (types.rs)

```rust
// Results
pub type Result<T> = std::result::Result<T, Error>;
pub type AgentResult<T> = std::result::Result<T, Error>;
pub type JobResult<T> = std::result::Result<T, Error>;
pub type QueryResult<T> = std::result::Result<T, Error>;
pub type TaskResult<T> = std::result::Result<T, Error>;

// Core entities
pub struct Entry { id, uuid, name, kind, ... }
pub struct Tag { id, name, color, icon }
pub enum EntryKind { File, Directory, Symlink, Virtual }
pub enum Priority { Low, Normal, High }
pub enum Capability { GPU, CPU }
pub enum Progress { Indeterminate, Simple, Complete }

// Type markers
pub struct Image;  // For .of_type::<Image>()
pub struct Pdf;
```

---

## VDFS Operations (vdfs.rs)

```rust
impl VdfsContext {
    // Entry queries
    fn query_entries() -> EntryQuery
    async fn get_entry(uuid) -> Result<Entry>

    // Model operations (content-scoped)
    async fn create_model_for_content<T>(content_uuid, model) -> Result<()>
    async fn get_model_by_content<T>(content_uuid) -> Result<T>
    async fn update_model_by_content<T, F>(content_uuid, f) -> Result<()>

    // Model operations (standalone)
    async fn create_model<T>(model) -> Result<()>
    async fn get_model<T>(uuid) -> Result<T>
    fn query_models<T>() -> ModelQuery<T>

    // Tagging
    async fn add_tag_to_content(content_uuid, tag) -> Result<()>
    async fn add_tag_to_model(model_uuid, tag) -> Result<()>
    async fn add_tag(metadata_id, tag) -> Result<()>

    // Custom fields
    async fn update_custom_field<T>(entry_uuid, field, value) -> Result<()>

    // Permissions
    fn in_granted_scope(path) -> bool
}

// Entry query builder
impl EntryQuery {
    fn in_location(path) -> Self
    fn of_type<T>() -> Self
    fn where_content_id(content_uuid) -> Self
    fn on_this_device() -> Self
    fn with_tag(tag) -> Self
    fn with_sidecar(kind) -> Self
    async fn first() -> Result<Option<Entry>>
    async fn collect() -> Result<Vec<Entry>>
    fn map<F, T>(f) -> MappedQuery<T>
}

// Model query builder
impl ModelQuery<T> {
    fn where_field(field, predicate) -> Self
    fn where_json_field(path, predicate) -> Self
    fn search_semantic(field, query) -> Self
    async fn first() -> Result<Option<T>>
    async fn collect() -> Result<Vec<T>>
}

// Predicates
fn equals<T>(value) -> FieldPredicate
fn contains(value) -> FieldPredicate
fn is_not_null() -> FieldPredicate
fn similar_to(query) -> SemanticQuery
```

---

## AI Operations (ai.rs)

```rust
impl AiContext {
    fn from_registered(model_id: &str) -> ModelHandle
    fn with_model(preference: &str) -> ModelHandle
}

impl ModelHandle {
    fn prompt_template(template_name: &str) -> PromptBuilder
    async fn detect_faces(image_data: &[u8]) -> Result<Vec<FaceDetection>>
    async fn classify(image_data: &[u8]) -> Result<Vec<SceneTag>>
    async fn ocr_document(entry: &Entry) -> Result<String>
    async fn embed_text(text: &str) -> Result<Vec<f32>>
}

impl PromptBuilder {
    fn render_with<T: Serialize>(context: &T) -> Result<RenderedPrompt>
}

impl RenderedPrompt {
    async fn generate_text() -> Result<String>
    async fn generate_json<T>() -> Result<T>
}

// AI types
pub struct FaceDetection { bbox, confidence, embedding, identified_as }
pub struct BoundingBox { x, y, width, height }
pub struct SceneTag { label, confidence }
```

---

## Agent System (agent.rs)

```rust
impl AgentContext<M> {
    fn vdfs() -> VdfsContext
    fn ai() -> AiContext
    fn models() -> ModelContext
    fn jobs() -> JobDispatcher
    fn memory() -> MemoryHandle<M>
    fn trace(message)
    fn in_granted_scope(path) -> bool
    fn config<C>() -> &C
    fn notify() -> NotificationBuilder
}

impl JobDispatcher {
    fn dispatch<J, A>(job, args) -> JobDispatchBuilder
}

impl JobDispatchBuilder {
    fn priority(priority) -> Self
    fn when_idle() -> Self
    fn on_device_with_capability(cap) -> Self
    async fn await() -> Result<()>
}

impl NotificationBuilder {
    fn message(msg) -> Self
    fn on_active_device() -> Self
    fn with_title(title) -> Self
    async fn send() -> Result<()>
}

// Memory types
pub struct TemporalMemory<T> {
    async fn append(event: T) -> Result<()>
    fn query() -> TemporalQuery<T>
}

pub struct AssociativeMemory<T> {
    async fn add(knowledge: T) -> Result<()>
    fn query() -> AssociativeQuery<T>
    fn query_similar(query: &str) -> AssociativeQuery<T>
}

pub struct WorkingMemory<T> {
    async fn read() -> T
    async fn update<F>(f: F) -> Result<()>
}

// Query builders
impl TemporalQuery<T> {
    fn where_variant<V>(variant) -> Self
    fn since(duration) -> Self
    fn where_field(field, predicate) -> Self
    fn where_semantic(field, query) -> Self
    fn sort_by<F>(f) -> Self
    fn limit(n) -> Self
    async fn collect() -> Result<Vec<T>>
}

impl AssociativeQuery<T> {
    fn where_variant<V>(variant) -> Self
    fn where_field(field, predicate) -> Self
    fn min_similarity(threshold) -> Self
    fn top_k(k) -> Self
    fn within_context<U>(context: &[U]) -> Self
    fn and_related_concepts(depth) -> Self
    async fn collect() -> Result<Vec<T>>
}

pub trait AgentMemory: Send + Sync {}
```

---

## Job Context (job_context.rs)

```rust
impl JobContext {
    // Existing (working in test-extension)
    fn report_progress(progress: f32, message: &str)
    fn checkpoint<S: Serialize>(state: &S) -> Result<()>
    fn check_interrupt() -> bool
    fn add_warning(message: &str)
    fn increment_bytes(bytes: u64)
    fn increment_items(count: u64)
    fn log(message: &str)
    fn log_error(message: &str)

    // NEW
    fn vdfs() -> VdfsContext
    fn ai() -> AiContext
    fn models() -> ModelContext
    async fn run<F, A, R>(task: F, args: A) -> Result<R>
    fn progress(progress: Progress)
    async fn check_interrupt() -> Result<()>  // Async version
    fn sidecar_exists(content_uuid, kind) -> Result<bool>
    async fn save_sidecar<T>(content_uuid, kind, extension_id, data) -> Result<()>
    fn memory() -> MemoryHandle<()>
    fn config<C>() -> &C
    fn notify() -> NotificationBuilder
}
```

---

## Task Context (tasks.rs)

```rust
impl TaskContext {
    fn vdfs() -> VdfsContext
    fn ai() -> AiContext
    fn config<C>() -> &C
    async fn read_sidecar<T>(content_uuid, kind) -> Result<T>
}
```

---

## Action Context (actions.rs)

```rust
impl ActionContext {
    fn vdfs() -> VdfsContext
}

pub struct ActionPreview {
    pub title: String,
    pub description: String,
    pub changes: Vec<Change>,
    pub reversible: bool,
}

pub enum Change {
    CreateModel { model_type, data },
    UpdateModel { model_id, field, operation, value },
    UpdateCustomField { entry_id, field, value },
    AddTag { target, tag },
    CreateDirectory { name, parent },
    MoveEntry { entry, destination },
}

pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
}
```

---

## Query Context (query.rs)

```rust
impl QueryContext<M> {
    fn vdfs() -> VdfsContext
    fn memory() -> MemoryHandle<M>
}
```

---

## Model Registration (models.rs)

```rust
impl ModelContext {
    async fn register(category, name, source) -> Result<ModelId>
    fn is_registered(model_id: &str) -> bool
}

pub enum ModelSource {
    Bundled(Vec<u8>),
    Download { url, sha256 },
    Ollama(String),
}

pub struct ModelId {
    pub category: String,
    pub name: String,
}

pub trait ExtensionModel: Serialize + DeserializeOwned + Send + Sync {
    const MODEL_TYPE: &'static str;
    fn uuid(&self) -> Uuid;
}
```

---

## Macros (spacedrive-sdk-macros)

All macros are currently pass-through stubs:

```rust
#[extension(id = "...", permissions = [...])]  // Generates plugin_init, metadata
#[model(version = "...", scope = "...")]       // Generates ExtensionModel impl
#[agent]                                        // Generates agent registration
#[agent_memory]                                 // Generates AgentMemory impl
#[job(parallelism = 4)]                        // Generates FFI exports (working)
#[task(retries = 3)]                           // Generates task wrapper
#[action]                                       // Generates action exports
#[query("pattern")]                             // Generates query exports
```

---

## Usage Examples

### Photos Extension (Now Type-Checks!)

```rust
#[model(scope = "content")]
struct PhotoAnalysis {
    detected_faces: Vec<FaceDetection>,
    // ... compiles!
}

#[job]
async fn analyze_photos(ctx: &JobContext, content_uuids: Vec<Uuid>) -> JobResult<()> {
    for content_uuid in content_uuids {
        let entry = ctx.vdfs()  // Type-checks
            .query_entries()
            .where_content_id(content_uuid)
            .first()
            .await?;

        let faces = ctx.ai()  // Type-checks
            .from_registered("face_detection")
            .detect_faces(&image_data)
            .await?;

        ctx.vdfs().create_model_for_content(content_uuid, analysis).await?;  // Type-checks
    }
    Ok(())
}
```

### Test Extension (Still Works!)

```rust
#[job(name = "counter")]
fn test_counter(ctx: &JobContext, state: &mut CounterState) -> Result<()> {
    ctx.log("Working...");  // Still works
    ctx.checkpoint(state)?;  // Still works
    Ok(())
}
```

---

## Implementation Status

| Module | Status | Notes |
|--------|--------|-------|
| `ffi.rs` | Implemented | Low-level WASM imports |
| `job_context.rs` | Expanded | New methods added, existing preserved |
| `types.rs` | Expanded | All common types added |
| `vdfs.rs` | Stubs | Type-checks, `todo!()` for host calls |
| `ai.rs` | Stubs | Type-checks, `todo!()` for inference |
| `agent.rs` | Stubs | Type-checks, memory system defined |
| `models.rs` | Stubs | Type-checks, registration stubs |
| `actions.rs` | Stubs | Type-checks, preview/execute defined |
| `tasks.rs` | Stubs | Type-checks, task context defined |
| `query.rs` | Stubs | Type-checks, query context defined |

**Macros:** All defined as pass-through (no codegen yet)

---

## What Works

**Type-checking:** Photos extension compiles and type-checks
**Test extension:** Existing test-extension still works
**API surface:** Complete API from specification available
**Documentation:** IntelliSense/rust-analyzer autocomplete works

## What Doesn't Work Yet

**Runtime:** All new methods are `todo!()` - will panic if called
**Host functions:** WASM imports not implemented in Core
**Macro codegen:** Macros don't generate code yet
**Memory persistence:** No storage backend

---

## Next Steps for Implementation

### Phase 1: Core Host Functions

Implement in `core/src/infra/extension/host_functions.rs`:
```rust
#[no_mangle]
pub extern "C" fn vdfs_query_entries(...) -> u32;
#[no_mangle]
pub extern "C" fn model_create(...) -> u32;
#[no_mangle]
pub extern "C" fn model_get_by_content(...) -> u32;
#[no_mangle]
pub extern "C" fn add_tag_to_content(...) -> u32;
#[no_mangle]
pub extern "C" fn model_register(...) -> u32;
#[no_mangle]
pub extern "C" fn ai_infer(...) -> u32;
```

### Phase 2: SDK Implementation

Replace `todo!()` with actual WASM host calls:
```rust
pub async fn get_model_by_content<T>(content_uuid: Uuid) -> Result<T> {
    // Serialize request
    let request = ModelRequest { content_uuid, model_type: T::MODEL_TYPE };
    let req_bytes = serde_json::to_vec(&request)?;

    // Call host function
    let result_ptr = unsafe {
        model_get_by_content(req_bytes.as_ptr(), req_bytes.len())
    };

    // Deserialize response
    let response_bytes = unsafe { read_host_memory(result_ptr) };
    let model: T = serde_json::from_slice(&response_bytes)?;

    Ok(model)
}
```

### Phase 3: Macro Code Generation

Implement real macros:
```rust
#[model(scope = "content")]
struct PhotoAnalysis { ... }

// Generates:
impl ExtensionModel for PhotoAnalysis {
    const MODEL_TYPE: &'static str = "PhotoAnalysis";
    fn uuid(&self) -> Uuid { self.id }
}

impl PhotoAnalysis {
    pub async fn save_for_content(ctx: &VdfsContext, content_uuid: Uuid, self) -> Result<()> {
        ctx.create_model_for_content(content_uuid, self).await
    }
}
```

---

## Testing

### Compile Test

```bash
cd crates/sdk
cargo check
# Should compile (all stubs)
```

### Extension Test

```bash
cd extensions/photos
cargo check --target wasm32-unknown-unknown
# Should type-check (won't run, but compiles)
```

### Runtime Test

```bash
cd extensions/test-extension
cargo build --target wasm32-unknown-unknown --release
# Should build and run (uses only implemented methods)
```

---

## Breaking Changes

**None!** The existing test-extension API is preserved:
- `ctx.log()` 
- `ctx.checkpoint()` 
- `ctx.check_interrupt()` 
- `ctx.report_progress()` 
- `ctx.increment_items()` 
- `ctx.increment_bytes()` 

New methods are additive only.

---

**The SDK is now complete for type-checking. Extensions can be written and will compile. Runtime implementation is the next phase.** 

