# Extension System Implementation Status

**Date:** October 9, 2025
**Status:** 🟢 Foundation Integrated - Compiling Successfully

---

## What We Built Today

### ✅ Completed: WASM Foundation

**1. Dependencies Integrated**
```toml
# core/Cargo.toml
wasmer = "4.2"
wasmer-middlewares = "4.2"
```
✅ Compiles successfully

**2. Module Structure Created**
```
core/src/infra/extension/
├── mod.rs              ✅ Module exports
├── types.rs            ✅ ExtensionManifest, permissions types
├── permissions.rs      ✅ Permission checking + rate limiting
├── host_functions.rs   ✅ host_spacedrive_call() skeleton
├── manager.rs          ✅ PluginManager (load/unload WASM)
└── README.md           ✅ Documentation
```

**3. Core Components**

**PluginManager** (`manager.rs`)
- Loads WASM modules from `plugins/` directory
- Compiles .wasm files with Wasmer
- Creates host function imports
- Manages plugin lifecycle (load/unload/reload)
- **Lines:** ~200

**Host Functions** (`host_functions.rs`)
- `host_spacedrive_call()` - THE generic Wire RPC function
- `host_spacedrive_log()` - Logging helper
- Skeleton implementation (pending memory management)
- **Lines:** ~50

**Permission System** (`permissions.rs`)
- Manifest-based permissions
- Method-level authorization (prefix matching)
- Library-level access control
- Rate limiting (1000 req/min default)
- **Lines:** ~200

---

## The Architecture

### The Genius Insight

**We don't need 15 host functions. We need ONE generic function that routes to the existing Wire registry:**

```
WASM Extension:
  spacedrive_call("query:ai.ocr.v1", lib_id, payload)
    ↓
  host_spacedrive_call() [reads from WASM memory]
    ↓
  RpcServer::execute_json_operation() [EXISTING - used by daemon!]
    ↓
  LIBRARY_QUERIES.get("query:ai.ocr.v1") [EXISTING registry!]
    ↓
  OcrQuery::execute() [EXISTING or NEW operation!]
```

**Result:**
- ✅ Zero protocol development (reuse Wire/Registry)
- ✅ Minimal host code (~100 lines when complete)
- ✅ Same operations work in WASM + daemon RPC + CLI + GraphQL
- ✅ Add new operations without touching host functions

### Manifest Format

```json
{
  "id": "finance",
  "name": "Spacedrive Finance",
  "version": "0.1.0",
  "wasm_file": "finance.wasm",
  "permissions": {
    "methods": [
      "vdfs.",           // Can call any vdfs.* operation
      "ai.ocr",           // Can call ai.ocr specifically
      "credentials."      // Can call any credentials.* operation
    ],
    "libraries": ["*"],   // All libraries, or specific UUIDs
    "rate_limits": {
      "requests_per_minute": 1000,
      "concurrent_jobs": 10
    },
    "max_memory_mb": 512
  }
}
```

---

## What's Next

### Phase 1: Complete WASM Memory Integration (Week 1)

**Tasks:**
- [ ] Study Wasmer 4.2 Memory API
- [ ] Implement `read_string_from_wasm()`
- [ ] Implement `write_json_to_wasm()`
- [ ] Implement guest allocator integration
- [ ] Complete `host_spacedrive_call()` with full Wire routing

**Blockers:** None - just learning Wasmer API

**Deliverable:** Working `host_spacedrive_call()` that calls `execute_json_operation()`

### Phase 2: Test WASM Module (Week 2)

**Tasks:**
- [ ] Create `test-plugin/` Rust project
- [ ] Implement `plugin_init()` export
- [ ] Call `spacedrive_call()` with test payload
- [ ] Compile to WASM (`wasm32-unknown-unknown`)
- [ ] Test loading with PluginManager

**Deliverable:** End-to-end test proving WASM → Wire → Operation works

### Phase 3: Extension Operations (Week 2-3)

**Tasks:**
- [ ] Implement `OcrQuery` (`core/src/ops/ai/ocr.rs`)
- [ ] Implement `ClassifyTextQuery` (`core/src/ops/ai/classify.rs`)
- [ ] Implement `StoreCredentialAction` (`core/src/ops/credentials/store.rs`)
- [ ] Implement `GetCredentialQuery` (`core/src/ops/credentials/get.rs`)
- [ ] Implement `WriteSidecarAction` (`core/src/ops/vdfs/sidecar.rs`)
- [ ] Register all with `register_library_query!()` / `register_library_action!()`

**Deliverable:** All operations needed by Finance extension available

### Phase 4: Extension SDK (Week 4)

**Tasks:**
- [ ] Create `spacedrive-sdk` crate
- [ ] Implement `SpacedriveClient` wrapper
- [ ] Type-safe operation methods
- [ ] Documentation
- [ ] Publish to crates.io (or local registry)

**Deliverable:** `cargo add spacedrive-sdk` works

### Phase 5: Finance Extension (Week 5-7)

**Tasks:**
- [ ] Gmail OAuth flow (via HTTP proxy host function)
- [ ] Email scanning logic
- [ ] Receipt detection heuristics
- [ ] OCR + AI classification pipeline
- [ ] Compile to WASM and test
- [ ] UI integration

**Deliverable:** Revenue-generating Finance extension MVP

---

## Technical Decisions Made

### 1. WASM-First (Not Process-Based)

**Rationale:**
- Better security (true sandbox)
- Better distribution (single .wasm file)
- Hot-reload capability
- Timeline is reasonable (~7 weeks total)

### 2. Generic `spacedrive_call()` (Not Per-Function FFI)

**Rationale:**
- Minimal API surface (2 functions vs. 15+)
- Perfect code reuse (Wire registry)
- Zero maintenance overhead
- Extensible without changing host

### 3. Reuse Wire/Registry Infrastructure

**Rationale:**
- Already exists and works
- Battle-tested by daemon RPC
- Type-safe via inventory crate
- Consistent across all clients

---

## Key Files

### Core Implementation
- `core/src/infra/extension/mod.rs` - Module exports
- `core/src/infra/extension/manager.rs` - Plugin lifecycle
- `core/src/infra/extension/host_functions.rs` - WASM host functions
- `core/src/infra/extension/permissions.rs` - Security model
- `core/src/infra/extension/types.rs` - Shared types

### Documentation
- `docs/core/design/WASM_ARCHITECTURE_FINAL.md` - Architecture overview
- `docs/core/design/EXTENSION_IPC_DESIGN.md` - Detailed design
- `docs/core/design/EMAIL_INGESTION_EXTENSION_DESIGN.md` - Finance extension spec
- `docs/PLATFORM_REVENUE_MODEL.md` - Business model

### Tasks
- `.tasks/PLUG-000-wasm-plugin-system.md` - Epic
- `.tasks/PLUG-001-integrate-wasm-runtime.md` - ✅ IN PROGRESS
- `.tasks/PLUG-002-define-vdfs-plugin-api.md` - Next
- `.tasks/PLUG-003-develop-twitter-agent-poc.md` - Future

---

## Current Limitations

### Not Yet Implemented

**1. WASM Memory Management**
- Reading strings/JSON from WASM memory
- Writing results back to WASM memory
- Guest allocator integration (`wasm_alloc` export)

**2. Full Wire Integration**
- Actual call to `execute_json_operation()`
- Permission enforcement in host function
- Error propagation to WASM

**3. Extension Operations**
- No AI operations exist yet (`ai.ocr`, `ai.classify_text`)
- No credential operations
- No VDFS sidecar operations

**4. HTTP Proxy**
- Extensions can't make external HTTP calls yet
- Need `spacedrive_http()` host function
- OAuth flows require this

### Workarounds

**For Testing:** Can test plugin loading without actual operation calls

**For Development:** Can use stub operations that return mock data

---

## How to Test (Once Memory is Implemented)

### 1. Create Test Plugin

```bash
# Create WASM project
cargo new --lib test-plugin
cd test-plugin

# Add to Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
serde = "1.0"
serde_json = "1.0"
```

```rust
// src/lib.rs
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    fn spacedrive_call(
        method_ptr: *const u8,
        method_len: usize,
        library_id_ptr: u32,
        payload_ptr: *const u8,
        payload_len: usize
    ) -> u32;
}

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    // Call a simple operation to test
    0
}
```

### 2. Compile to WASM

```bash
cargo build --target wasm32-unknown-unknown --release
```

### 3. Load in Spacedrive

```rust
let mut pm = PluginManager::new(core, PathBuf::from("./plugins"));
pm.load_plugin("test-plugin").await?;
```

---

## Performance Characteristics

### Expected Performance

**Plugin Loading:**
- WASM compilation: ~50-200ms (one-time)
- Instance creation: ~5-10ms
- Total startup: <250ms

**Operation Calls:**
- WASM → Host transition: ~1-5μs
- Wire registry lookup: ~100ns (HashMap)
- Operation execution: Varies by operation
- Total overhead: <10μs per call

**Memory:**
- WASM linear memory: Configurable (default 512MB max)
- Runtime overhead: ~5-10MB per loaded plugin
- Reasonable for 10-20 plugins loaded simultaneously

---

## Security Model

### WASM Sandbox Guarantees

✅ Cannot access filesystem directly
✅ Cannot make network calls directly
✅ Cannot access host process memory
✅ Cannot escape sandbox
✅ CPU usage bounded (Wasmer metering)
✅ Memory usage bounded (runtime limits)

### Permission Layers

1. **Manifest Permissions** - Declared capabilities
2. **Runtime Checks** - Enforced on every `spacedrive_call()`
3. **Rate Limiting** - Prevents DoS
4. **Resource Limits** - CPU/memory bounded by Wasmer

### Permission Example

```json
{
  "permissions": {
    "methods": ["vdfs.", "ai.ocr"],
    "libraries": ["550e8400-e29b-41d4-a716-446655440000"],
    "rate_limits": { "requests_per_minute": 1000 }
  }
}
```

Results in:
- ✅ Can call `vdfs.create_entry`
- ✅ Can call `ai.ocr`
- ❌ Cannot call `credentials.delete` (not in list)
- ❌ Cannot access other libraries

---

## Team Communication

### What to Tell Engineers

"We've integrated the foundation for WASM extensions. The system compiles and the architecture is sound. Next step is implementing memory interaction and creating a test module."

### What to Tell Product/Business

"WASM extension foundation is in place. Timeline to first revenue-generating extension (Finance) is 6-7 weeks. Architecture allows infinite extensions without touching core code."

### What to Tell Investors

"Platform foundation integrated. Single generic API (`spacedrive_call`) reuses existing infrastructure, minimizing maintenance burden while enabling unlimited extensions."

---

## Questions & Answers

**Q: Why WASM instead of native plugins?**
A: Security (true sandbox), distribution (single .wasm file), hot-reload, memory safety.

**Q: Can extensions make HTTP calls?**
A: Not directly (WASM sandbox). We'll add `spacedrive_http()` host function as controlled proxy.

**Q: How do extensions access OAuth tokens?**
A: Via `credentials.get()` operation - tokens stored encrypted in Spacedrive vault.

**Q: What if an extension crashes?**
A: WASM sandbox prevents corrupting core. Extension just stops, can be reloaded.

**Q: Can we support JavaScript extensions?**
A: Yes! Compile JS → WASM via AssemblyScript or similar. Rust recommended for now.

---

*Status: Foundation complete ✅ - Ready for memory implementation phase*

