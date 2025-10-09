# WASM Plugin Integration - Honest Plan to Get It Working

## Current Reality Check

### ✅ What Actually Works
- Core compiles with extension module
- PluginManager code exists
- Host functions implemented (but stub some parts)
- WASM module compiles (254KB)
- Macros generate code

### ❌ What Doesn't Work Yet
- Can't load WASM module (PluginManager not integrated into Core)
- Can't call job export (no WasmJob executor)
- Host functions log but don't update real job state
- Memory allocation uses fixed offset (not proper allocator)

---

## The Blockers (In Priority Order)

### Blocker 1: PluginManager Needs Arc<Core>

**Current:**
```rust
pub fn new(core: Arc<Core>, plugin_dir: PathBuf) -> Self
```

**Problem:** Core has circular dependency - can't create PluginManager in Core::new() because PluginManager needs Core.

**Solution:** Add PluginManager to Core after initialization

```rust
// In Core struct
pub struct Core {
    // ... existing fields ...
    pub plugin_manager: Option<Arc<RwLock<PluginManager>>>,  // NEW
}

// After Core::new():
let plugin_dir = data_dir.join("extensions");
let pm = PluginManager::new(Arc::new(core.clone()), plugin_dir);  // Circular!
```

**Actually:** We need to refactor PluginManager to not need full Core:

```rust
pub fn new(
    event_bus: Arc<EventBus>,  // For logging
    plugin_dir: PathBuf
) -> Self

// Remove dependency on Core in PluginEnv
pub struct PluginEnv {
    pub extension_id: String,
    pub event_bus: Arc<EventBus>,  // Instead of Arc<Core>
    pub permissions: ExtensionPermissions,
    pub memory: Memory,
}
```

**Work:** 30 minutes to refactor

### Blocker 2: host_spacedrive_call() Can't Actually Call Operations

**Current:**
```rust
fn host_spacedrive_call(...) -> u32 {
    let result = RpcServer::execute_json_operation(...).await;  // Needs Core!
    write_json_to_memory(&result)
}
```

**Problem:** `execute_json_operation()` is a static method on RpcServer, but it needs `&Arc<Core>`.

**Solution:** Pass Core reference through PluginEnv:

```rust
pub struct PluginEnv {
    pub extension_id: String,
    pub core_ref: Arc<Core>,  // Keep this!
    pub permissions: ExtensionPermissions,
    pub memory: Memory,
}

fn host_spacedrive_call(...) -> u32 {
    // Now we have core!
    let result = RpcServer::execute_json_operation(
        &method,
        library_id,
        payload,
        &plugin_env.core_ref  // Use it here
    ).await;
    ...
}
```

**Work:** 15 minutes to fix

### Blocker 3: No Operations for SDK to Call

**Current SDK calls:**
```rust
ctx.vdfs().create_entry(...)  // Calls "action:vdfs.create_entry.input.v1" - doesn't exist
ctx.ai().ocr(...)             // Calls "query:ai.ocr.v1" - doesn't exist
```

**Solution:** Remove ALL imaginary operations from SDK. Only keep what exists:

```rust
// spacedrive-sdk - REMOVE:
- ai.rs (uses non-existent operations)
- vdfs.rs (most methods don't exist)
- credentials.rs (doesn't exist)

// spacedrive-sdk - KEEP:
- ffi.rs (low-level, works)
- job_context.rs (job functions exist!)
- types.rs (just types)
```

**Work:** 10 minutes to delete files

### Blocker 4: Job Can't Be Dispatched

**Current:** No way to dispatch a WASM job because:
1. No `WasmJob` type registered in job system
2. No way to call WASM exports from job executor

**Solution:** Create minimal WasmJob:

```rust
// core/src/infra/extension/wasm_job.rs
#[derive(Serialize, Deserialize)]
pub struct WasmJob {
    extension_id: String,
    export_fn: String,
    state_json: String,  // JSON state (simpler than binary)
}

impl Job for WasmJob {
    const NAME: &'static str = "wasm_job";
    const RESUMABLE: bool = true;
}

impl JobHandler for WasmJob {
    type Output = ();

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<()> {
        // 1. Get plugin from global registry
        let pm = ctx.core().plugin_manager()?;
        let plugin = pm.get_plugin(&self.extension_id).await?;

        // 2. Prepare context JSON
        let ctx_json = json!({
            "job_id": ctx.id().to_string(),
            "library_id": ctx.library().id().to_string(),
        });

        // 3. Call WASM export
        let export_fn = plugin.get_function(&self.export_fn)?;
        let result = export_fn.call(&mut store, &[
            /* pass ctx_json and state_json as pointers */
        ])?;

        // 4. Read updated state
        self.state_json = read_result_from_wasm(result)?;

        Ok(())
    }
}

register_job!(WasmJob);
```

**Work:** 2-3 hours

### Blocker 5: Memory Allocation Not Working

**Current:**
```rust
fn write_json_to_memory(...) -> u32 {
    let result_offset = 65536u32;  // FIXED! Won't work properly
    // ...
}
```

**Solution:** Actually call guest's `wasm_alloc`:

```rust
fn write_json_to_memory(memory: &Memory, store: &mut StoreMut, json: &Value) -> u32 {
    let json_bytes = serde_json::to_vec(json)?;

    // Get wasm_alloc export from instance
    // Need to store instance reference in PluginEnv!
    let alloc_fn = store.get_export("wasm_alloc")?;
    let ptr = alloc_fn.call(&[Value::I32(json_bytes.len() as i32)])?;

    // Write to allocated memory
    memory.write(ptr, &json_bytes)?;

    ptr
}
```

**Work:** 1-2 hours

---

## Step-by-Step Plan to Get Job Actually Running

### Phase 1: Make It Loadable (Day 1 - 2 hours)

**Goal:** Load test-extension and see "✓ Test extension initialized!" in logs

**Steps:**
1. ✅ Remove imaginary SDK operations (ai, vdfs, credentials)
2. ✅ Keep only: ffi.rs, job_context.rs, types.rs, lib.rs
3. ✅ Update test-extension to not call non-existent operations
4. ✅ Refactor PluginManager to be addable to Core
5. ✅ Add `plugin_manager: Option<Arc<RwLock<PluginManager>>>` to Core struct
6. ✅ Initialize in Core::new() after other services
7. ✅ Test: `core.plugin_manager.load_plugin("test-extension").await?`

**Deliverable:** See "✓ Test extension initialized!" in test output

### Phase 2: Make Job Callable (Day 2 - 4 hours)

**Goal:** Call the counter job export and see it log

**Steps:**
1. ✅ Create WasmJob type
2. ✅ Register with job system
3. ✅ Implement basic executor (call WASM export)
4. ✅ Fix memory allocation (call wasm_alloc properly)
5. ✅ Test: Dispatch WasmJob, see execute_test_counter() logs

**Deliverable:** Job runs, logs appear, exits with success code

### Phase 3: Hook Up Job Context (Day 3 - 4 hours)

**Goal:** Job functions actually work (progress shows up, checkpoints save)

**Steps:**
1. ✅ Create JobContext registry (job_id → JobContext map)
2. ✅ In WasmJob::run(), register JobContext before calling WASM
3. ✅ In host_job_report_progress(), look up JobContext and call real method
4. ✅ Test: See progress updates in job manager

**Deliverable:** Full job with working progress, checkpoints, metrics

---

## Minimal Test Case

```rust
// core/tests/wasm_extension_test.rs

use sd_core::Core;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_wasm_extension() {
    // 1. Initialize Core (like other tests do)
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new_with_config(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // 2. Load extension
    let pm = core.plugin_manager.as_ref().unwrap();
    pm.write().await.load_plugin("test-extension").await.unwrap();

    // 3. Verify loaded
    let loaded = pm.read().await.list_plugins().await;
    assert!(loaded.contains(&"test-extension".to_string()));

    println!("✅ Extension loaded successfully!");
}

#[tokio::test]
async fn test_dispatch_wasm_job() {
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new_with_config(temp_dir.path().to_path_buf()).await.unwrap();

    // Load extension
    core.plugin_manager.as_ref().unwrap()
        .write().await
        .load_plugin("test-extension").await.unwrap();

    // Create library
    let library = core.libraries
        .create_library("Test", None, core.context.clone())
        .await.unwrap();

    // Dispatch WASM job
    let job_id = library.jobs().dispatch_by_name(
        "wasm_job",  // Generic WasmJob type
        serde_json::json!({
            "extension_id": "test-extension",
            "export_fn": "execute_test_counter",
            "state_json": json!({"current": 0, "target": 10}).to_string()
        })
    ).await.unwrap();

    // Wait for completion
    let handle = library.jobs().get_handle(job_id).await.unwrap();
    handle.wait().await.unwrap();

    println!("✅ WASM job executed successfully!");
}
```

---

## What I'll Actually Implement

### Step 1: Debloat SDK (30 min)

Remove:
- ❌ `ai.rs` - calls non-existent operations
- ❌ `vdfs.rs` - calls non-existent operations
- ❌ `credentials.rs` - calls non-existent operations
- ❌ `jobs.rs` - dispatch doesn't work yet

Keep:
- ✅ `ffi.rs` - low-level, minimal
- ✅ `job_context.rs` - job functions exist!
- ✅ `types.rs` - just types
- ✅ `lib.rs` - minimal

### Step 2: Simplify Test Extension (15 min)

Remove all calls to non-existent operations. Job should only:
- Log messages
- Update counter
- Report progress
- Checkpoint state
- Check interruption

No VDFS, no AI, no credentials - just the job mechanics.

### Step 3: Add PluginManager to Core (1 hour)

```rust
// core/src/lib.rs
pub struct Core {
    // ... existing ...
    pub plugin_manager: Arc<RwLock<PluginManager>>,
}

impl Core {
    pub async fn new_with_config(...) -> Result<Self> {
        // ... existing initialization ...

        // Initialize plugin manager
        let plugin_dir = data_dir.join("extensions");
        std::fs::create_dir_all(&plugin_dir)?;

        let plugin_manager = Arc::new(RwLock::new(
            PluginManager::new(
                events.clone(),
                plugin_dir
            )
        ));

        Ok(Self {
            // ... existing ...
            plugin_manager,
        })
    }
}
```

### Step 4: Create WasmJob (2-3 hours)

Minimal job executor that calls WASM export.

### Step 5: Write Real Test (1 hour)

Test that loads extension and dispatches job.

---

## Total Time: 1-2 days

**Day 1 (4-5 hours):**
- Debloat SDK
- Simplify test extension
- Add PluginManager to Core
- Test loading

**Day 2 (4-5 hours):**
- Create WasmJob
- Fix memory allocation
- Test job execution
- Validate end-to-end

---

## Expected Output

```bash
$ cargo test wasm_extension_test

running 2 tests

test test_load_wasm_extension ...
  INFO Loading plugin: test-extension
  INFO Compiled WASM module
  INFO ✓ Test extension initialized!
  INFO Plugin test-extension loaded successfully
✅ Extension loaded successfully!
ok

test test_dispatch_wasm_job ...
  INFO Dispatching job: wasm_job
  INFO Starting counter (current: 0, target: 10)
  INFO Counted 1/10 (10% complete)
  INFO Counted 2/10 (20% complete)
  ...
  INFO ✓ Completed! Processed 10 items
✅ WASM job executed successfully!
ok
```

---

**Ready to do this for real? I'll focus on getting ONE thing actually working instead of designing perfect APIs.**

