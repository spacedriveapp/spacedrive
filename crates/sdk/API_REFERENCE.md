# Spacedrive SDK API Reference

**Status:** Records, sidecars, models, tasks, entropy, clock and config have a host side behind `spacedrive_op`; inference answers `Error::NotAvailable`; tags, custom fields, agents and actions remain stubs.
**Purpose:** The API surface extensions compile against, and what each part does at runtime.

---

## Overview

Every method that returns data from the host goes through one import,
`spacedrive_op(name, payload)`. The host checks the manifest's `permissions`
block and answers JSON, or an error with a stable code the SDK maps onto
`Error` (`NotFound`, `PermissionDenied`, `InvalidInput`, `NotAvailable`,
`OperationFailed`).

### Renames since the stub phase

| Was | Now | Why |
|-----|-----|-----|
| `Entry`, `EntryKind` | `Record`, `RecordKind` | The entry table is gone; files are records in a source store. |
| `vdfs().get_entry(uuid)` | `vdfs().get_record(uuid)` | |
| `vdfs().query_entries()` | `vdfs().query_records()` | Filters by source, path, extension and type. `with_tag` and `where_metadata` are refused by the host. |
| new | `vdfs().read_record(uuid)`, `Record::read()` | The bytes of a record, read from its source. |
| `save_sidecar(content_uuid, kind, extension_id, data)` | `save_sidecar(content_uuid, kind, data)` | The host keys the sidecar by the calling extension. |
| `add_tag(metadata_id, tag)` | `add_tag(record_uuid, tag)` | Tags attach to records; no host side yet. |
| new | `spacedrive_sdk::clock::now()` | Wall-clock time; `SystemTime::now` panics on wasm32-unknown-unknown. |
| new | `#[model]` emits a definition the host registers | Models live in an extension-owned store under `<library>/extensions/<id>/`. |

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
pub struct Record { uuid, source, path, name, kind, extension, size, content_uuid, ... }
pub struct Tag { id, name, color, icon }
pub enum RecordKind { File, Directory, Symlink }
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
    // Record queries (read_records grant)
    fn query_records() -> RecordQuery
    async fn get_record(uuid) -> Result<Record>
    async fn read_record(uuid) -> Result<Vec<u8>>

    // Sidecars (read_sidecars / write_sidecars grants), one JSON document per kind
    fn sidecar_exists(content_uuid, kind) -> Result<bool>
    async fn read_sidecar<T>(content_uuid, kind) -> Result<T>
    async fn write_sidecar<T>(content_uuid, kind, data) -> Result<()>

    // Model operations (content-scoped)
    async fn create_model_for_content<T>(content_uuid, model) -> Result<()>
    async fn get_model_by_content<T>(content_uuid) -> Result<T>
    async fn update_model_by_content<T, F>(content_uuid, f) -> Result<()>

    // Model operations (standalone)
    async fn create_model<T>(model) -> Result<()>
    async fn get_model<T>(uuid) -> Result<T>
    fn query_models<T>() -> ModelQuery<T>

    // Tagging (no host side yet, returns Error::NotAvailable)
    async fn add_tag_to_content(content_uuid, tag) -> Result<()>
    async fn add_tag_to_model(model_uuid, tag) -> Result<()>
    async fn add_tag(record_uuid, tag) -> Result<()>

    // Custom fields (no host side yet)
    async fn update_custom_field<T>(record_uuid, field, value) -> Result<()>

    // Permissions
    fn in_granted_scope(path) -> bool
}

// Record query builder
impl RecordQuery {
    fn in_source(source_uuid) -> Self
    fn in_location(path) -> Self
    fn with_extensions(extensions) -> Self
    fn of_type<T>() -> Self
    fn with_tag(tag) -> Self          // refused by the host today
    fn where_metadata(field, predicate) -> Self  // refused by the host today
    fn limit(n) -> Self
    async fn first() -> Result<Option<Record>>
    async fn collect() -> Result<Vec<Record>>
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
    async fn ocr_document(record: &Record) -> Result<String>
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
    async fn save_sidecar<T>(content_uuid, kind, data) -> Result<()>
    async fn read_sidecar<T>(content_uuid, kind) -> Result<T>
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
    MoveRecord { record, destination },
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

### Photos Extension

`extensions/photos/src/jobs/analyze.rs` is the worked example: it reads each
photo through `get_record` and `Record::read`, runs face detection as a
`#[task]` with a retry policy, handles `Error::NotAvailable` with one warning
and a skip, and writes a `faces` sidecar when a detector answers.

```rust
#[job(name = "analyze_photos_batch")]
async fn analyze_photos(ctx: &JobContext, state: &mut AnalyzePhotosState) -> JobResult<()> {
    for photo_id in &state.photo_ids {
        let record = ctx.vdfs().get_record(*photo_id).await?;
        let Some(content_uuid) = record.content_uuid() else { continue };
        match ctx.run(detect_faces_in_photo, record).await {
            Ok(faces) => ctx.save_sidecar(content_uuid, "faces", &faces).await?,
            Err(Error::NotAvailable(_)) => { ctx.add_warning("no face detector"); break }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
```

### Test Extension

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
| `ffi.rs` | Implemented | `spacedrive_log`, `register_job`, `register_model`, `spacedrive_random`, `spacedrive_now_ms`, `spacedrive_op` |
| `job_context.rs` | Implemented | Progress, checkpoints, interrupts, sidecars, `run` for tasks |
| `types.rs` | Implemented | `Record`, `RecordKind`, `Error` with host codes |
| `vdfs.rs` | Partly | Records, sidecars and models work; tags and custom fields return `NotAvailable` |
| `clock.rs` | Implemented | `now()` over `spacedrive_now_ms` |
| `tasks.rs` | Implemented | `#[task]` with retry policy and deadline; the host logs each attempt |
| `models.rs` | Implemented | `#[model]` defines, `#[extension(models = [...])]` registers; rows in the extension's store |
| `ai.rs` | Not available | `infer` checks the `use_models` grant on the host, then returns `Error::NotAvailable` |
| `config.rs` | Implemented | `config.json` beside the manifest |
| `agent.rs` | Stubs | Memory and notifications have no host side |
| `actions.rs` | Stubs | Preview and execute have no host side |
| `query.rs` | Stubs | No host side |

---

## What Works

**Runtime:** The photos extension's `analyze_photos` runs end to end under the daemon's `wasm` feature; `core/tests/wasm_extension_test.rs` is the acceptance test.
**Randomness:** `getrandom` is backed by `spacedrive_random`, so `Uuid::new_v4()` and `rand` work in the guest (each extension's `.cargo/config.toml` sets `--cfg getrandom_backend="custom"`).

## What Doesn't Work Yet

**Inference:** The core has no provider for face detection, scene classification, embeddings or language models, so every `ai().infer` answers `NotAvailable`; photos' scenes, places and moments jobs have nothing to run on.
**Tags, custom fields, dispatching jobs:** no host side.
**Agents, actions, queries:** no host side.
**File kinds and previews:** not registered by extensions.

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

See the renames table in the overview. The original test-extension API is preserved:
- `ctx.log()` 
- `ctx.checkpoint()` 
- `ctx.check_interrupt()` 
- `ctx.report_progress()` 
- `ctx.increment_items()` 
- `ctx.increment_bytes()` 

New methods are additive only.

---

**Extensions compile and the record, sidecar, model and task paths run. Inference is the next host side to build.**

