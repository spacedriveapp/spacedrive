<!--CREATED: 2025-10-11-->
# Extension Job System Parity

**Question:** Can extensions do everything core jobs can? (Progress, checkpoints, child jobs, metrics, etc.)

**Answer:** YES - by exposing JobContext capabilities through host functions.

---

## What Core Jobs Can Do

Based on `JobContext` in `core/src/infra/job/context.rs`:

| Capability | Core Job API | Purpose |
|------------|-------------|---------|
| **Progress** | `ctx.progress(Progress::percent(0.5))` | Report 0-100% progress |
| **Checkpoints** | `ctx.checkpoint()` | Save state for resumability |
| **State Persistence** | `ctx.save_state(&state)` | Store job state |
| **State Loading** | `ctx.load_state::<State>()` | Resume from saved state |
| **Interruption Check** | `ctx.check_interrupt()` | Handle pause/cancel |
| **Metrics** | `ctx.increment_bytes(1000)` | Track bytes/items processed |
| **Warnings** | `ctx.add_warning("message")` | Non-fatal issues |
| **Errors** | `ctx.add_non_critical_error(err)` | Recoverable errors |
| **Logging** | `ctx.log("message")` | Structured logging |
| **Child Jobs** | `ctx.spawn_child(job)` | Spawn sub-jobs |
| **Library Access** | `ctx.library()` | Get library database |
| **Networking** | `ctx.networking_service()` | P2P operations |

**Extensions MUST have these same capabilities to be first-class.**

---

## How Extensions Get Full Parity

### Option 1: JobContext Host Functions (RECOMMENDED)

**Concept:** Expose JobContext operations as additional host functions.

```rust
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    // Generic operation call (existing)
    fn spacedrive_call(...) -> u32;

    // === Job-Specific Functions (NEW) ===

    /// Report job progress (0.0 to 1.0)
    fn job_report_progress(job_id_ptr: u32, progress: f32, message_ptr: u32, message_len: u32);

    /// Save checkpoint with current state
    fn job_checkpoint(job_id_ptr: u32, state_ptr: u32, state_len: u32) -> i32;

    /// Load saved state
    fn job_load_state(job_id_ptr: u32) -> u32;  // Returns ptr to state bytes

    /// Check if job should pause/cancel
    fn job_check_interrupt(job_id_ptr: u32) -> i32;  // 0=continue, 1=pause, 2=cancel

    /// Add warning message
    fn job_add_warning(job_id_ptr: u32, message_ptr: u32, message_len: u32);

    /// Track metrics
    fn job_increment_bytes(job_id_ptr: u32, bytes: u64);
    fn job_increment_items(job_id_ptr: u32, count: u64);

    /// Spawn child job
    fn job_spawn_child(job_id_ptr: u32, child_type_ptr: u32, child_type_len: u32, params_ptr: u32, params_len: u32) -> u32;
}
```

**Total: 10 additional host functions** (but all simple wrappers)

### Option 2: Pass JobContext as Params (SIMPLER)

**Concept:** When Core calls WASM job export, pass serialized JobContext info.

```rust
// Core calls WASM job export with context
let context_json = json!({
    "job_id": job_id.to_string(),
    "library_id": library.id(),
    "capabilities": ["progress", "checkpoint", "spawn_child"]
});

let context_bytes = serde_json::to_vec(&context_json)?;

// Call WASM export
export_fn.call(&[
    Value::I32(context_bytes.as_ptr() as i32),
    Value::I32(context_bytes.len() as i32),
    Value::I32(state_bytes.as_ptr() as i32),
    Value::I32(state_bytes.len() as i32)
])?;
```

**Then WASM uses job ID to call back:**

```rust
// Extension calls host function with job ID
fn job_report_progress(job_id: Uuid, progress: f32, message: &str);
```

---

## Recommendation: Hybrid (Best of Both)

**Job Execution Pattern:**

```
1. Core dispatches WasmJob
2. Core serializes JobContext info (job_id, library_id, etc.)
3. Core calls WASM export: execute_job(job_ctx_json, job_state_bytes)
4. WASM deserializes context + state
5. WASM calls host functions for job operations (using job_id)
6. Core routes based on job_id to actual JobContext
7. WASM returns updated state
8. Core saves state to database
```

**Implementation:**

```rust
// core/src/infra/extension/host_functions.rs

/// Report job progress (job-specific host function)
fn host_job_report_progress(
    env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
    progress: f32,
    message_ptr: WasmPtr<u8>,
    message_len: u32,
) {
    let (plugin_env, store) = env.data_and_store_mut();

    // Read job ID
    let job_id = read_uuid_from_wasm(&store, job_id_ptr);
    let message = read_string_from_wasm(&store, message_ptr, message_len);

    // Get the JobContext for this job_id (stored in Core)
    let job_ctx = plugin_env.core.get_job_context(&job_id)?;

    // Call the actual context method
    job_ctx.progress(Progress::percent(progress, message));
}

/// Save checkpoint
fn host_job_checkpoint(
    env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
    state_ptr: WasmPtr<u8>,
    state_len: u32,
) -> i32 {
    let (plugin_env, store) = env.data_and_store_mut();

    let job_id = read_uuid_from_wasm(&store, job_id_ptr);
    let state_bytes = read_bytes_from_wasm(&store, state_ptr, state_len);

    // Get JobContext
    let job_ctx = plugin_env.core.get_job_context(&job_id)?;

    // Save checkpoint
    tokio::runtime::Handle::current().block_on(async {
        job_ctx.checkpoint_with_state(&state_bytes).await
    }).map(|_| 0).unwrap_or(1)
}

/// Check for interruption
fn host_job_check_interrupt(
    env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
) -> i32 {
    let (plugin_env, store) = env.data_and_store_mut();

    let job_id = read_uuid_from_wasm(&store, job_id_ptr);
    let job_ctx = plugin_env.core.get_job_context(&job_id)?;

    // Check interrupt
    tokio::runtime::Handle::current().block_on(async {
        job_ctx.check_interrupt().await
    }).map(|_| 0).unwrap_or(1) // 0 = continue, 1 = interrupted
}
```

---

## Beautiful SDK API for Extensions

```rust
// spacedrive-sdk/src/jobs.rs

pub struct JobContext {
    job_id: Uuid,
    library_id: Uuid,
}

impl JobContext {
    /// Report progress (0.0 to 1.0)
    pub fn report_progress(&self, progress: f32, message: &str) -> Result<()> {
        unsafe {
            job_report_progress(
                self.job_id.as_bytes().as_ptr() as u32,
                progress,
                message.as_ptr() as u32,
                message.len() as u32
            );
        }
        Ok(())
    }

    /// Save checkpoint with current state
    pub fn checkpoint<S: Serialize>(&self, state: &S) -> Result<()> {
        let state_bytes = serde_json::to_vec(state)?;
        unsafe {
            job_checkpoint(
                self.job_id.as_bytes().as_ptr() as u32,
                state_bytes.as_ptr() as u32,
                state_bytes.len() as u32
            );
        }
        Ok(())
    }

    /// Load saved state
    pub fn load_state<S: DeserializeOwned>(&self) -> Result<Option<S>> {
        let state_ptr = unsafe {
            job_load_state(self.job_id.as_bytes().as_ptr() as u32)
        };

        if state_ptr == 0 {
            return Ok(None);
        }

        // Read state from WASM memory
        let state_bytes = read_from_wasm_ptr(state_ptr);
        Ok(Some(serde_json::from_slice(&state_bytes)?))
    }

    /// Check if job should stop (returns true if interrupted)
    pub fn check_interrupt(&self) -> Result<bool> {
        let result = unsafe {
            job_check_interrupt(self.job_id.as_bytes().as_ptr() as u32)
        };
        Ok(result != 0)
    }

    /// Add warning (non-fatal issue)
    pub fn add_warning(&self, message: &str) {
        unsafe {
            job_add_warning(
                self.job_id.as_bytes().as_ptr() as u32,
                message.as_ptr() as u32,
                message.len() as u32
            );
        }
    }

    /// Track bytes processed
    pub fn increment_bytes(&self, bytes: u64) {
        unsafe {
            job_increment_bytes(self.job_id.as_bytes().as_ptr() as u32, bytes);
        }
    }

    /// Track items processed
    pub fn increment_items(&self, count: u64) {
        unsafe {
            job_increment_items(self.job_id.as_bytes().as_ptr() as u32, count);
        }
    }

    /// Get VDFS client
    pub fn vdfs(&self) -> VdfsClient {
        // Uses library_id from context
        VdfsClient::new_with_library(self.library_id)
    }

    /// Get AI client
    pub fn ai(&self) -> AiClient {
        AiClient::new_with_library(self.library_id)
    }
}
```

---

## Extension Job Example (Full Parity!)

```rust
use spacedrive_sdk::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct EmailScanState {
    last_uid: String,
    processed: usize,
    total: usize,
}

/// WASM job export - called by Core's WasmJobExecutor
#[no_mangle]
pub extern "C" fn execute_email_scan(
    job_ctx_ptr: u32,
    job_ctx_len: u32,
    state_ptr: u32,
    state_len: u32
) -> u32 {
    // Parse job context (from Core)
    let ctx = JobContext::from_params(job_ctx_ptr, job_ctx_len);

    // Load or initialize state
    let mut state: EmailScanState = if state_len > 0 {
        ctx.deserialize_state(state_ptr, state_len).unwrap()
    } else {
        // First run
        EmailScanState {
            last_uid: String::new(),
            processed: 0,
            total: 0,
        }
    };

    ctx.log(&format!("Resuming email scan from UID: {}", state.last_uid));

    // Fetch emails
    let emails = fetch_emails_since(&state.last_uid).unwrap();
    state.total = emails.len();

    for (i, email) in emails.iter().enumerate() {
        // Check if we should pause/cancel
        if ctx.check_interrupt().unwrap() {
            ctx.log("Received interrupt, saving checkpoint...");
            ctx.checkpoint(&state).unwrap();
            return ctx.return_interrupted(&state);
        }

        // Process email using SDK
        let entry = ctx.vdfs().create_entry(CreateEntry {
            name: format!("Receipt: {}", email.subject),
            path: format!("receipts/{}.eml", email.id),
            entry_type: "FinancialDocument".into(),
        }).unwrap();

        // Run OCR
        if let Some(pdf) = &email.pdf_attachment {
            match ctx.ai().ocr(pdf, OcrOptions::default()) {
                Ok(ocr_result) => {
                    ctx.vdfs().write_sidecar(entry.id, "ocr.txt", ocr_result.text.as_bytes()).unwrap();
                    ctx.increment_bytes(pdf.len() as u64);
                }
                Err(e) => {
                    ctx.add_warning(&format!("OCR failed for {}: {}", email.id, e));
                }
            }
        }

        // Update state
        state.last_uid = email.uid.clone();
        state.processed += 1;

        // Report progress
        let progress = state.processed as f32 / state.total as f32;
        ctx.report_progress(
            progress,
            &format!("Processed {}/{} emails", state.processed, state.total)
        ).unwrap();

        // Checkpoint every 10 emails
        if state.processed % 10 == 0 {
            ctx.checkpoint(&state).unwrap();
        }

        ctx.increment_items(1);
    }

    ctx.log("Email scan completed!");
    ctx.return_completed(&state)
}
```

**That's a complete resumable job with full parity to core jobs!**

---

## Implementation: Job-Specific Host Functions

### Additional Host Functions Needed

```rust
// core/src/infra/extension/host_functions.rs

// Add to imports:
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    // Existing
    fn spacedrive_call(...);
    fn spacedrive_log(...);

    // === NEW: Job Operations ===

    /// Report progress for a job
    fn job_report_progress(
        job_id_ptr: u32,
        progress: f32,
        message_ptr: u32,
        message_len: u32
    ) -> i32;

    /// Save checkpoint
    fn job_checkpoint(
        job_id_ptr: u32,
        state_ptr: u32,
        state_len: u32
    ) -> i32;

    /// Load saved state
    fn job_load_state(job_id_ptr: u32) -> u32;  // Returns ptr to state

    /// Check for pause/cancel
    fn job_check_interrupt(job_id_ptr: u32) -> i32;  // 0=continue, 1=interrupted

    /// Add warning
    fn job_add_warning(
        job_id_ptr: u32,
        message_ptr: u32,
        message_len: u32
    );

    /// Track bytes processed
    fn job_increment_bytes(job_id_ptr: u32, bytes: u64);

    /// Track items processed
    fn job_increment_items(job_id_ptr: u32, count: u64);

    /// Spawn child job
    fn job_spawn_child(
        job_id_ptr: u32,
        child_type_ptr: u32,
        child_type_len: u32,
        params_ptr: u32,
        params_len: u32
    ) -> u32;  // Returns child job_id
}
```

### Host Function Implementation (~30 lines each)

```rust
// core/src/infra/extension/host_functions.rs

fn host_job_report_progress(
    mut env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
    progress: f32,
    message_ptr: WasmPtr<u8>,
    message_len: u32,
) -> i32 {
    let (plugin_env, mut store) = env.data_and_store_mut();
    let memory = &plugin_env.memory;
    let memory_view = memory.view(&store);

    // Read job ID and message
    let job_id = match read_uuid_from_wasm(&memory_view, job_id_ptr) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to read job ID: {}", e);
            return 1; // Error
        }
    };

    let message = match read_string_from_wasm(&memory_view, message_ptr, message_len) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!("Failed to read message: {}", e);
            return 1;
        }
    };

    // Get the JobContext for this job_id from Core
    // Core maintains a map: job_id -> JobContext
    let job_ctx = match plugin_env.core.get_active_job_context(&job_id) {
        Some(ctx) => ctx,
        None => {
            tracing::error!("No active job context for {}", job_id);
            return 1;
        }
    };

    // Call the actual JobContext method
    job_ctx.progress(Progress::percent(progress, message));

    0 // Success
}

fn host_job_checkpoint(
    mut env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
    state_ptr: WasmPtr<u8>,
    state_len: u32,
) -> i32 {
    let (plugin_env, mut store) = env.data_and_store_mut();
    let memory = &plugin_env.memory;
    let memory_view = memory.view(&store);

    let job_id = read_uuid_from_wasm(&memory_view, job_id_ptr).unwrap();
    let state_bytes = read_bytes_from_wasm(&memory_view, state_ptr, state_len).unwrap();

    let job_ctx = plugin_env.core.get_active_job_context(&job_id)?;

    // Save checkpoint
    tokio::runtime::Handle::current().block_on(async {
        job_ctx.checkpoint_with_state(&state_bytes).await
    }).map(|_| 0).unwrap_or(1)
}

fn host_job_check_interrupt(
    mut env: FunctionEnvMut<PluginEnv>,
    job_id_ptr: WasmPtr<u8>,
) -> i32 {
    let (plugin_env, mut store) = env.data_and_store_mut();
    let memory = &plugin_env.memory;
    let memory_view = memory.view(&store);

    let job_id = read_uuid_from_wasm(&memory_view, job_id_ptr).unwrap();
    let job_ctx = plugin_env.core.get_active_job_context(&job_id)?;

    // Check if interrupted
    tokio::runtime::Handle::current().block_on(async {
        job_ctx.check_interrupt().await
    }).map(|_| 0).unwrap_or(1) // 0 = not interrupted, 1 = interrupted
}

// Similar for other functions (increment_bytes, add_warning, etc.)
```

### Core: Job Context Registry

```rust
// core/src/infra/extension/job_contexts.rs

use std::collections::HashMap;
use tokio::sync::RwLock;

/// Registry of active job contexts
/// Allows WASM jobs to access their JobContext via job_id
pub struct JobContextRegistry {
    contexts: RwLock<HashMap<Uuid, Arc<JobContext>>>,
}

impl JobContextRegistry {
    pub async fn register(&self, job_id: Uuid, ctx: Arc<JobContext>) {
        self.contexts.write().await.insert(job_id, ctx);
    }

    pub async fn get(&self, job_id: &Uuid) -> Option<Arc<JobContext>> {
        self.contexts.read().await.get(job_id).cloned()
    }

    pub async fn remove(&self, job_id: &Uuid) {
        self.contexts.write().await.remove(job_id);
    }
}

// Add to Core
impl Core {
    pub fn job_context_registry(&self) -> &JobContextRegistry {
        &self.job_context_registry
    }
}
```

### WasmJob Executor

```rust
// core/src/infra/extension/wasm_job.rs

pub struct WasmJob {
    extension_id: String,
    export_fn: String,
    state: Vec<u8>,  // Serialized job state
}

impl Job for WasmJob {
    const NAME: &'static str = "wasm_extension_job";
    const RESUMABLE: bool = true;
}

impl JobHandler for WasmJob {
    type Output = ();

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<()> {
        // 1. Register JobContext so WASM can access it
        ctx.core().job_context_registry().register(ctx.id(), Arc::new(ctx)).await;

        // 2. Prepare job context info for WASM
        let job_ctx_json = json!({
            "job_id": ctx.id().to_string(),
            "library_id": ctx.library().id().to_string(),
        });
        let ctx_bytes = serde_json::to_vec(&job_ctx_json)?;

        // 3. Get WASM plugin
        let plugin = ctx.core().plugin_manager().get(&self.extension_id).await?;

        // 4. Call WASM export
        let export_fn = plugin.get_function(&self.export_fn)?;
        let result_ptr = export_fn.call(&mut store, &[
            Value::I32(ctx_bytes.as_ptr() as i32),
            Value::I32(ctx_bytes.len() as i32),
            Value::I32(self.state.as_ptr() as i32),
            Value::I32(self.state.len() as i32),
        ])?[0].unwrap_i32() as u32;

        // 5. Read updated state from WASM memory
        self.state = read_from_wasm_memory(plugin.memory(), result_ptr)?;

        // 6. Cleanup context registry
        ctx.core().job_context_registry().remove(&ctx.id()).await;

        Ok(())
    }
}
```

---

## Complete Extension Job Example

```rust
use spacedrive_sdk::jobs::JobContext;

#[derive(Serialize, Deserialize)]
pub struct EmailScanState {
    last_uid: String,
    processed: usize,
    errors: Vec<String>,
}

#[no_mangle]
pub extern "C" fn execute_email_scan(
    ctx_ptr: u32,
    ctx_len: u32,
    state_ptr: u32,
    state_len: u32
) -> u32 {
    // 1. Parse job context
    let job_ctx = JobContext::from_params(ctx_ptr, ctx_len).unwrap();

    // 2. Load or initialize state
    let mut state: EmailScanState = if state_len > 0 {
        JobContext::deserialize_state(state_ptr, state_len).unwrap()
    } else {
        // Load from checkpoint if resuming
        job_ctx.load_state().unwrap().unwrap_or(EmailScanState {
            last_uid: String::new(),
            processed: 0,
            errors: Vec::new(),
        })
    };

    job_ctx.log(&format!("Starting email scan from UID: {}", state.last_uid));

    // 3. Do work with full job capabilities
    let emails = fetch_emails(&state.last_uid).unwrap();

    for (i, email) in emails.iter().enumerate() {
        // Check interruption every email
        if job_ctx.check_interrupt().unwrap() {
            job_ctx.log("Job interrupted, saving state...");
            job_ctx.checkpoint(&state).unwrap();
            return job_ctx.return_interrupted(&state);
        }

        // Process email
        match process_email(&job_ctx, email) {
            Ok(entry_id) => {
                job_ctx.increment_items(1);
                if let Some(pdf) = &email.pdf_attachment {
                    job_ctx.increment_bytes(pdf.len() as u64);
                }
            }
            Err(e) => {
                // Non-critical error
                job_ctx.add_warning(&format!("Failed to process {}: {}", email.id, e));
                state.errors.push(email.id.clone());
            }
        }

        state.last_uid = email.uid.clone();
        state.processed += 1;

        // Report progress
        let progress = (i + 1) as f32 / emails.len() as f32;
        job_ctx.report_progress(
            progress,
            &format!("Processed {}/{} emails", i + 1, emails.len())
        ).unwrap();

        // Checkpoint every 10 emails
        if state.processed % 10 == 0 {
            job_ctx.checkpoint(&state).unwrap();
        }
    }

    // 4. Complete
    job_ctx.log(&format!("✓ Completed! Processed {} emails, {} errors", state.processed, state.errors.len()));
    job_ctx.return_completed(&state)
}
```

**Extension jobs now have:**
- Progress reporting
- Checkpointing (auto-resume)
- Interruption handling (pause/cancel)
- Metrics tracking
- Warning/error reporting
- Full VDFS/AI access
- Same UX as core jobs

---

## Summary

### Can Extensions Do Everything Core Jobs Can?

**YES!** By adding ~10 job-specific host functions:

| Core Job Capability | Extension Equivalent | Implementation |
|-------------------|---------------------|----------------|
| Progress reporting | `job_ctx.report_progress()` | host_job_report_progress() |
| Checkpointing | `job_ctx.checkpoint(&state)` | host_job_checkpoint() |
| State loading | `job_ctx.load_state()` | host_job_load_state() |
| Interruption check | `job_ctx.check_interrupt()` | host_job_check_interrupt() |
| Warnings | `job_ctx.add_warning()` | host_job_add_warning() |
| Metrics | `job_ctx.increment_bytes()` | host_job_increment_bytes() |
| Logging | `job_ctx.log()` | host_job_log() |
| Child jobs | `job_ctx.spawn_child()` | host_job_spawn_child() |

### Total Host Functions

**Core:**
- `spacedrive_call()` - Generic Wire RPC
- `spacedrive_log()` - General logging

**Job-Specific (8 functions):**
- `job_report_progress()`
- `job_checkpoint()`
- `job_load_state()`
- `job_check_interrupt()`
- `job_add_warning()`
- `job_increment_bytes()`
- `job_increment_items()`
- `job_spawn_child()`

**Total: 10 host functions**

### Implementation Cost

- Host functions: ~250 lines (8 functions × 30 lines)
- JobContext registry: ~100 lines
- WasmJob wrapper: ~200 lines
- SDK JobContext API: ~200 lines
- **Total: ~750 lines**

**Timeline: 1 week**

### Result

Extensions get **100% parity** with core jobs:
- Same progress UX
- Same resumability
- Same metrics
- Same logging
- Same child job support
- Same everything!

Ready to implement this?

