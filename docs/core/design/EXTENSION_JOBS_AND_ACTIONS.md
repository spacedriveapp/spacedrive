# Extension-Defined Jobs and Actions

**Question:** How can WASM extensions register their own custom jobs and actions, not just call existing ones?

**Challenge:** Core uses compile-time registration (`inventory` crate + macros). WASM extensions load at runtime.

---

## Current Core Architecture

### Jobs (Compile-Time Registration)

```rust
// Core defines a job
pub struct EmailScanJob {
    pub last_uid: String,
    // ... state fields
}

impl Job for EmailScanJob {
    const NAME: &'static str = "email_scan";
    // ... trait methods
}

// Registers at compile time using inventory
register_job!(EmailScanJob);
```

**Result:** `REGISTRY` HashMap populated at startup with all job types.

### Actions (Compile-Time Registration)

```rust
pub struct FileCopyAction;

impl LibraryAction for FileCopyAction {
    type Input = FileCopyInput;
    type Output = FileCopyOutput;
    // ... implementation
}

// Registers at compile time
crate::register_library_action!(FileCopyAction, "files.copy");
```

**Result:** `LIBRARY_ACTIONS` HashMap populated at compile time.

---

## The WASM Extension Challenge

**Problem:** Extensions load at runtime, but registries are compile-time.

**Options:**

### Option 1: Extensions Define Jobs via WASM Exports (RECOMMENDED)

**Concept:** Extensions export execution functions, Core wraps them in a generic `WasmJob`.

**Architecture:**

```
Extension (WASM):
├── Exports: execute_email_scan(params_json) -> result_json
│
Core:
├── Wraps in generic WasmJob
├── Job system dispatches WasmJob
├── Executor calls WASM export
└── State serialized/resumed like normal jobs
```

**Extension Code (Beautiful API):**

```rust
use spacedrive_sdk::prelude::*;

// Extension defines job state
#[derive(Serialize, Deserialize)]
pub struct EmailScanState {
    pub last_uid: String,
    pub processed: usize,
}

// Extension exports execution function
#[no_mangle]
pub extern "C" fn execute_email_scan(params_ptr: u32, params_len: u32) -> u32 {
    let ctx = ExtensionContext::from_params(params_ptr, params_len);

    let mut state: EmailScanState = ctx.get_job_state()?;

    // Do work
    let emails = fetch_emails_since(&state.last_uid)?;

    for email in emails {
        process_email(&ctx, &email)?;
        state.processed += 1;
        state.last_uid = email.uid.clone();

        // Report progress (Core saves state automatically)
        ctx.report_progress(state.processed as f32 / emails.len() as f32, &state)?;
    }

    ctx.complete(&state)
}
```

**Core Integration:**

```rust
// core/src/infra/extension/jobs.rs
pub struct WasmJob {
    extension_id: String,
    job_name: String,  // e.g., "execute_email_scan"
    state: Vec<u8>,     // Serialized job state
}

impl Job for WasmJob {
    const NAME: &'static str = "wasm_extension_job";
    const RESUMABLE: bool = true;
}

impl JobHandler for WasmJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult<()> {
        // Get the WASM instance for this extension
        let plugin = ctx.plugin_manager().get(&self.extension_id)?;

        // Call the WASM export
        let export_fn = plugin.get_function(&self.job_name)?;
        let result_ptr = export_fn.call(&[
            Value::I32(self.state.as_ptr() as i32),
            Value::I32(self.state.len() as i32)
        ])?;

        // Read updated state from WASM memory
        self.state = read_from_wasm_memory(result_ptr)?;

        Ok(())
    }
}
```

**Extension Registers Job:**

```rust
// In plugin_init()
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    let ctx = ExtensionContext::new(library_id);

    // Register custom job
    ctx.register_job(JobRegistration {
        name: "email_scan",
        export_function: "execute_email_scan",
        resumable: true,
    })?;

    0
}
```

**Dispatching the Job (from WASM or Core):**

```rust
// Extension can dispatch its own job
let job_id = ctx.jobs().dispatch("finance:email_scan", json!({
    "provider": "gmail",
    "last_uid": "12345"
}))?;

// Or from CLI/GraphQL (once registered)
daemon_client.send(DaemonRequest::Action {
    method: "action:jobs.dispatch.input.v1",
    payload: json!({
        "job_type": "finance:email_scan",
        "params": { "provider": "gmail" }
    })
});
```

### Option 2: Runtime Registry for Extension Operations

**Concept:** Maintain separate runtime registry for extension-defined operations.

```rust
// Core maintains both registries
static CORE_OPERATIONS: Lazy<HashMap<...>> = ...;  // Compile-time
static EXTENSION_OPERATIONS: RwLock<HashMap<...>> = ...;  // Runtime

// When extension loads:
plugin_manager.register_operation(
    "finance:classify_receipt",
    WasmOperationHandler {
        extension_id: "finance",
        export_fn: "classify_receipt",
    }
);

// execute_json_operation checks both:
pub async fn execute_json_operation(method: &str, ...) -> Result<Value> {
    // Try core operations first
    if let Some(handler) = LIBRARY_QUERIES.get(method) {
        return handler(...).await;
    }

    // Try extension operations
    if let Some(handler) = EXTENSION_OPERATIONS.read().get(method) {
        return handler.call_wasm(...).await;
    }

    Err("Unknown method")
}
```

**Extension Registration:**

```rust
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    let ctx = ExtensionContext::new(library_id);

    // Register custom query
    ctx.register_query(
        "finance:classify_receipt",
        "classify_receipt",  // WASM export name
    )?;

    // Register custom action
    ctx.register_action(
        "finance:process_email",
        "process_email",
    )?;

    0
}

// Export the handler
#[no_mangle]
pub extern "C" fn classify_receipt(input_ptr: u32, input_len: u32) -> u32 {
    let input: ClassifyReceiptInput = read_from_wasm(input_ptr, input_len);

    // Extension logic
    let result = do_classification(&input);

    write_to_wasm(&result)
}
```

### Option 3: Extensions Compose Core Operations (SIMPLEST)

**Concept:** Extensions don't define new operations - they just compose existing ones.

**For Jobs:** Extensions trigger core jobs with extension-specific parameters
**For Actions:** Extensions call sequences of core actions

```rust
// Extension doesn't register new job type
// Instead, uses generic "extension_task" job

#[no_mangle]
pub extern "C" fn scan_emails() -> i32 {
    let ctx = ExtensionContext::new(library_id);

    // Dispatch a task that will call back into extension
    let job_id = ctx.jobs().dispatch("extension_task", json!({
        "extension_id": "finance",
        "task_name": "scan_emails",
        "params": { "provider": "gmail" }
    }))?;

    0
}

// Core has generic WasmTaskJob that calls extension exports
// Extension exports task handlers:
#[no_mangle]
pub extern "C" fn task_scan_emails(params_ptr: u32) -> u32 {
    let ctx = ExtensionContext::from_ptr(params_ptr);

    // Extension logic using SDK
    let emails = fetch_gmail()?;
    for email in emails {
        let entry = ctx.vdfs().create_entry(...)?;
        let ocr = ctx.ai().ocr(&email.attachment, ...)?;
        ctx.vdfs().write_sidecar(...)?;
    }

    ctx.complete()
}
```

---

## Recommendation: Hybrid Approach

**For Jobs:** Use Option 1 (WASM exports with generic WasmJob wrapper)

**For Actions/Queries:** Use Option 2 (runtime registry)

**Why:**

**Jobs:**
- Long-running, stateful, need resumability
- WASM exports work well for execution
- Core handles persistence/resume
- Clean for extension developers

**Actions/Queries:**
- Short-lived, synchronous
- Can be pure WASM functions
- Runtime registration makes sense
- Extensions can expose custom Wire methods

---

## Proposed Implementation

### 1. Add Runtime Operation Registry

```rust
// core/src/infra/extension/registry.rs
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct ExtensionOperationRegistry {
    queries: RwLock<HashMap<String, WasmQueryHandler>>,
    actions: RwLock<HashMap<String, WasmActionHandler>>,
}

struct WasmQueryHandler {
    extension_id: String,
    export_fn_name: String,
}

impl ExtensionOperationRegistry {
    pub async fn register_query(&self, method: String, handler: WasmQueryHandler) {
        self.queries.write().await.insert(method, handler);
    }

    pub async fn call_query(&self, method: &str, payload: Value, pm: &PluginManager) -> Result<Value> {
        let handler = self.queries.read().await.get(method).cloned()?;

        // Get WASM plugin
        let plugin = pm.get_plugin(&handler.extension_id).await?;

        // Call WASM export
        let export_fn = plugin.get_function(&handler.export_fn_name)?;
        let result = export_fn.call(...)?;

        Ok(result)
    }
}
```

### 2. Update execute_json_operation

```rust
// core/src/infra/daemon/rpc.rs
pub async fn execute_json_operation(...) -> Result<Value> {
    // Try core operations (compile-time registry)
    if let Some(handler) = LIBRARY_QUERIES.get(method) {
        return handler(...).await;
    }

    // Try extension operations (runtime registry)
    if let Some(result) = extension_registry.try_call(method, payload).await? {
        return Ok(result);
    }

    Err("Unknown method")
}
```

### 3. Extension SDK API

```rust
// spacedrive-sdk/src/lib.rs

impl ExtensionContext {
    /// Register a custom query operation
    pub fn register_query(&self, name: &str, handler: QueryHandler) -> Result<()> {
        // Calls host function to add to runtime registry
        ffi::register_operation(
            &format!("query:{}:{}.v1", self.extension_id(), name),
            handler.export_fn_name
        )
    }

    /// Register a custom action operation
    pub fn register_action(&self, name: &str, handler: ActionHandler) -> Result<()> {
        ffi::register_operation(
            &format!("action:{}:{}.input.v1", self.extension_id(), name),
            handler.export_fn_name
        )
    }

    /// Register a custom job type
    pub fn register_job(&self, registration: JobRegistration) -> Result<()> {
        ffi::register_job(&registration)
    }
}

pub struct QueryHandler {
    pub export_fn_name: String,
}

pub struct JobRegistration {
    pub name: String,
    pub export_fn_name: String,
    pub resumable: bool,
}
```

### 4. Extension Usage (Clean!)

```rust
use spacedrive_sdk::prelude::*;

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    let ctx = ExtensionContext::new(library_id);

    // Register custom operations
    ctx.register_query("classify_receipt", QueryHandler {
        export_fn_name: "handle_classify_receipt".into(),
    }).ok();

    ctx.register_job(JobRegistration {
        name: "email_scan".into(),
        export_fn_name: "execute_email_scan".into(),
        resumable: true,
    }).ok();

    0
}

// Implement the query handler
#[no_mangle]
pub extern "C" fn handle_classify_receipt(input_ptr: u32, input_len: u32) -> u32 {
    let ctx = ExtensionContext::from_params(input_ptr, input_len);

    // Read input
    let input: ClassifyReceiptInput = ctx.read_input()?;

    // Extension logic
    let ocr = ctx.ai().ocr(&input.pdf_data, OcrOptions::default())?;
    let analysis = parse_receipt(&ocr.text)?;

    // Return result
    ctx.write_output(&analysis)
}

// Implement the job handler
#[no_mangle]
pub extern "C" fn execute_email_scan(state_ptr: u32, state_len: u32) -> u32 {
    let ctx = ExtensionContext::from_params(state_ptr, state_len);

    // Read job state
    let mut state: EmailScanState = ctx.get_job_state()?;

    // Do work
    let emails = fetch_since(&state.last_uid)?;
    for email in emails {
        process_email(&ctx, &email)?;
        state.last_uid = email.uid;
        ctx.report_progress(state.processed as f32 / emails.len() as f32, &state)?;
    }

    ctx.complete(&state)
}
```

**Now other extensions/CLI/GraphQL can call:**

```rust
// Call extension-defined query
let result = daemon.send(DaemonRequest::Query {
    method: "query:finance:classify_receipt.v1",
    payload: json!({ "pdf_data": ... })
});

// Dispatch extension-defined job
let job_id = ctx.jobs().dispatch("finance:email_scan", json!({
    "provider": "gmail"
}));
```

---

## Implementation Plan

### Phase 1: Runtime Registry (Week 1)

```rust
// core/src/infra/extension/registry.rs

pub struct ExtensionRegistry {
    // Extension-defined operations
    operations: RwLock<HashMap<String, WasmOperation>>,
    // Extension-defined jobs
    jobs: RwLock<HashMap<String, WasmJobRegistration>>,
}

struct WasmOperation {
    extension_id: String,
    export_fn: String,
    operation_type: OperationType,
}

enum OperationType {
    Query,
    Action,
}

struct WasmJobRegistration {
    extension_id: String,
    export_fn: String,
    resumable: bool,
}

impl ExtensionRegistry {
    /// Register a WASM operation at runtime
    pub async fn register_operation(
        &self,
        method: String,
        extension_id: String,
        export_fn: String,
        op_type: OperationType,
    ) -> Result<()> {
        self.operations.write().await.insert(
            method,
            WasmOperation { extension_id, export_fn, operation_type: op_type }
        );
        Ok(())
    }

    /// Call a WASM operation
    pub async fn call_operation(
        &self,
        method: &str,
        payload: Value,
        plugin_manager: &PluginManager,
    ) -> Result<Value> {
        let op = self.operations.read().await
            .get(method)
            .cloned()
            .ok_or("Operation not found")?;

        // Get WASM plugin
        let plugin = plugin_manager.get_plugin(&op.extension_id).await?;

        // Serialize payload
        let payload_bytes = serde_json::to_vec(&payload)?;

        // Call WASM export
        let export_fn = plugin.get_export(&op.export_fn)?;
        let result_ptr = export_fn.call(&mut store, &[
            Value::I32(payload_bytes.as_ptr() as i32),
            Value::I32(payload_bytes.len() as i32),
        ])?[0].unwrap_i32() as u32;

        // Read result
        let result = read_json_from_wasm(plugin.memory(), result_ptr)?;

        Ok(result)
    }
}
```

### Phase 2: Integrate with execute_json_operation

```rust
// core/src/infra/daemon/rpc.rs
pub async fn execute_json_operation(
    method: &str,
    library_id: Option<Uuid>,
    payload: Value,
    core: &Core,
) -> Result<Value> {
    // Try core operations first (compile-time registry)
    if let Some(handler) = LIBRARY_QUERIES.get(method) {
        return handler(core.context.clone(), session, payload).await;
    }

    // Try extension operations (runtime registry)
    if let Some(result) = core.extension_registry()
        .call_operation(method, payload, core.plugin_manager())
        .await?
    {
        return Ok(result);
    }

    Err(format!("Unknown method: {}", method))
}
```

### Phase 3: SDK API

```rust
// spacedrive-sdk/src/extension.rs

impl ExtensionContext {
    /// Register a custom query that other clients can call
    pub fn register_query(&self, name: &str, export_fn: &str) -> Result<()> {
        let method = format!("query:{}:{}.v1", self.extension_id(), name);

        ffi::call_host("extension.register_operation", json!({
            "method": method,
            "export_fn": export_fn,
            "operation_type": "query"
        }))
    }

    /// Register a custom action
    pub fn register_action(&self, name: &str, export_fn: &str) -> Result<()> {
        let method = format!("action:{}:{}.input.v1", self.extension_id(), name);

        ffi::call_host("extension.register_operation", json!({
            "method": method,
            "export_fn": export_fn,
            "operation_type": "action"
        }))
    }

    /// Register a custom job type
    pub fn register_job(&self, registration: JobRegistration) -> Result<()> {
        ffi::call_host("extension.register_job", json!({
            "job_name": format!("{}:{}", self.extension_id(), registration.name),
            "export_fn": registration.export_fn,
            "resumable": registration.resumable
        }))
    }
}
```

---

## Complete Example: Finance Extension

```rust
use spacedrive_sdk::prelude::*;

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    let ctx = ExtensionContext::new(library_id);

    // Register custom operations
    ctx.register_query("classify_receipt", "classify_receipt_handler").ok();
    ctx.register_action("import_receipts", "import_receipts_handler").ok();
    ctx.register_job(JobRegistration {
        name: "email_scan",
        export_fn: "execute_email_scan",
        resumable: true,
    }).ok();

    0
}

// Custom query - callable by anyone via Wire
#[no_mangle]
pub extern "C" fn classify_receipt_handler(input_ptr: u32, input_len: u32) -> u32 {
    let ctx = ExtensionContext::from_params(input_ptr, input_len);
    let input: ClassifyInput = ctx.read_input().unwrap();

    // Use SDK to call core operations
    let ocr = ctx.ai().ocr(&input.pdf, OcrOptions::default()).unwrap();
    let analysis = ctx.ai().classify_text(&ocr.text, "Extract receipt data").unwrap();

    ctx.write_output(&analysis)
}

// Custom action - creates receipts from email
#[no_mangle]
pub extern "C" fn import_receipts_handler(input_ptr: u32, input_len: u32) -> u32 {
    let ctx = ExtensionContext::from_params(input_ptr, input_len);
    let input: ImportInput = ctx.read_input().unwrap();

    let mut imported = vec![];
    for email in input.emails {
        let entry = ctx.vdfs().create_entry(CreateEntry {
            name: format!("Receipt: {}", email.subject),
            path: format!("receipts/{}.eml", email.id),
            entry_type: "FinancialDocument".into(),
        }).unwrap();

        imported.push(entry.id);
    }

    ctx.write_output(&json!({ "imported_ids": imported }))
}

// Custom job - resumable email scanning
#[no_mangle]
pub extern "C" fn execute_email_scan(state_ptr: u32, state_len: u32) -> u32 {
    let ctx = ExtensionContext::from_job_params(state_ptr, state_len);

    let mut state: EmailScanState = ctx.get_job_state().unwrap();

    // Resumable work
    let emails = fetch_emails_since(&state.last_uid).unwrap();
    for (i, email) in emails.iter().enumerate() {
        process_email(&ctx, email).unwrap();
        state.last_uid = email.uid.clone();
        state.processed += 1;

        ctx.report_progress(i as f32 / emails.len() as f32, &state).ok();
    }

    ctx.complete(&state)
}
```

**Then from CLI:**

```bash
# Call extension-defined query
spacedrive query finance:classify_receipt --pdf receipt.pdf

# Dispatch extension-defined job
spacedrive jobs dispatch finance:email_scan --provider gmail

# Call from other extensions!
let result = ctx.call_query("finance:classify_receipt", input)?;
```

---

## Summary

**Key Insights:**

1. **Extensions CAN register custom operations** - via runtime registry
2. **Wire methods namespaced by extension** - `"finance:classify_receipt"`
3. **WASM exports are operation handlers** - clean separation
4. **Same privileges as core** - extensions are first-class

**Benefits:**

Extensions can define domain-specific operations
Operations are reusable (other extensions can call them!)
Clean SDK API hides complexity
Core handles persistence/resumability
Type-safe via JSON schemas

**Implementation:**
- Runtime registry: ~300 lines
- WASM job wrapper: ~200 lines
- SDK registration API: ~200 lines
- **Total: ~700 lines**

**Timeline:** 1-2 weeks to implement

Ready to build this?

