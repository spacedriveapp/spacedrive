# WASM Extension System - Current Actual Status

**Updated:** October 9, 2025
**Reality Check:** What actually works vs. what needs to be done

---

## ✅ What's ACTUALLY Working Right Now

### 1. Code Compiles
- ✅ Core with extension module
- ✅ Debloated SDK (only ffi, job_context, types)
- ✅ Test extension (97KB WASM)
- ✅ Macros generate code correctly

### 2. WASM Module Built
```bash
$ ls -lh extensions/test-extension/test_extension.wasm
-rwxr-xr-x  97K test_extension.wasm
```

### 3. Host Functions Implemented
```rust
// In core/src/infra/extension/host_functions.rs
✅ spacedrive_call()         // Implemented (stubs some parts)
✅ spacedrive_log()           // Fully working
✅ job_report_progress()      // Implemented (logs only)
✅ job_checkpoint()           // Implemented (logs only)
✅ job_check_interrupt()      // Implemented (always returns "continue")
✅ job_add_warning()          // Implemented (logs only)
✅ job_increment_bytes()      // Implemented (logs only)
✅ job_increment_items()      // Implemented (logs only)
```

### 4. Test Structure Created
- ✅ `core/tests/wasm_extension_test.rs` exists
- ✅ Compiles successfully
- ✅ Currently stubbed (waiting for PluginManager integration)

---

## ❌ What's NOT Working (The Honest List)

### 1. PluginManager Not in Core

**Current:** PluginManager exists but isn't initialized in Core

**Missing:**
```rust
// core/src/lib.rs
pub struct Core {
    // ... existing fields ...
    pub plugin_manager: Arc<RwLock<PluginManager>>,  // ← ADD THIS
}
```

**Impact:** Can't call `core.plugin_manager.load_plugin()`

**Fix Time:** 1 hour

### 2. Can't Load Extension Yet

Because PluginManager isn't in Core, this doesn't work:
```rust
let pm = core.plugin_manager; // Field doesn't exist!
pm.load_plugin("test-extension").await?;
```

**Fix Time:** Part of #1

### 3. Can't Dispatch WASM Job

**Missing:** WasmJob executor that calls WASM exports

**Needed:**
```rust
// core/src/infra/extension/wasm_job.rs
pub struct WasmJob {
    extension_id: String,
    export_fn: String,
    state_json: String,
}

impl JobHandler for WasmJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult<()> {
        // Call WASM export with context and state
    }
}

register_job!(WasmJob);
```

**Fix Time:** 2-3 hours

### 4. Job Functions Are Stubs

**Current:** Job host functions log but don't update real state

**Example:**
```rust
fn host_job_report_progress(...) {
    tracing::info!("Progress: {}", progress);
    // TODO: Forward to actual JobContext
}
```

**Needed:** JobContext registry to map job_id → actual JobContext

**Fix Time:** 2-3 hours

### 5. Memory Allocation Simplified

**Current:** Uses fixed offset (65536)

**Should:** Call guest's `wasm_alloc()` function

**Fix Time:** 1-2 hours

---

## The Path Forward (Honest Timeline)

### Tomorrow (Day 1): Get Loading Working

**Goal:** See "✓ Test extension initialized!" when running test

**Tasks:**
1. Add `plugin_manager: Arc<RwLock<PluginManager>>` to Core struct
2. Initialize in `Core::new_with_config()`
3. Run test, see extension load

**Time:** 2-3 hours

**Deliverable:**
```bash
$ cargo test wasm_extension_test
...
INFO Loading plugin: test-extension
INFO ✓ Test extension initialized!
test test_load_wasm_extension ... ok
```

### Day 2-3: Get Job Execution Working

**Goal:** Dispatch a job and see it run

**Tasks:**
1. Create WasmJob executor
2. Register with job system
3. Test dispatching
4. See job logs

**Time:** 4-6 hours

**Deliverable:**
```bash
$ cargo test test_dispatch_wasm_job
...
INFO Starting counter (current: 0, target: 10)
INFO Counted 1/10
...
INFO ✓ Completed!
test test_dispatch_wasm_job ... ok
```

### Day 4: Hook Up Job Context

**Goal:** Progress/checkpoints actually work

**Tasks:**
1. JobContext registry
2. Connect host functions to real JobContext
3. Test progress updates

**Time:** 3-4 hours

**Deliverable:** See actual progress in job manager UI

---

## What I'm Committing To

**By end of tomorrow:**
- ✅ Extension loads
- ✅ plugin_init() calls
- ✅ Appears in plugin list

**By end of this week:**
- ✅ Job dispatches
- ✅ execute_test_counter() runs
- ✅ Progress/checkpoints work
- ✅ Full end-to-end test passes

---

## Current Blocker: PluginManager Not in Core

**This is the ONLY thing stopping us from testing loading right now.**

Ready to add it?

