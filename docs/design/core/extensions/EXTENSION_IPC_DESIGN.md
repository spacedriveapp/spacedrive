<!--CREATED: 2025-10-11-->
# WASM Extension Architecture: Leveraging Existing Operation Registry

## TL;DR

**Extensions run as WASM modules with direct host function access.** The key insight: we leverage the existing **operation registry** (same handlers used by daemon RPC) but expose them via **WASM host functions** instead of Unix sockets.

**Key Insight:** WASM extensions call host functions that internally route to the same operation handlers used by CLI/GraphQL/iOS apps.

---

## Table of Contents

1. [WASM Extension Architecture](#wasm-extension-architecture)
2. [Leveraging Existing Operation Registry](#leveraging-existing-operation-registry)
3. [WASM Host Functions API](#wasm-host-functions-api)
4. [Security Model](#security-model)
5. [Extension Lifecycle](#extension-lifecycle)
6. [Implementation Plan](#implementation-plan)
7. [Complete Code Examples](#complete-code-examples)
8. [Migration Path (Optional Process-Based Prototype)](#migration-path-optional-process-based-prototype)

---

## WASM Extension Architecture

### Core Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Spacedrive Core (Rust)                     │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │        WASM Plugin Host (Wasmer)                 │  │
│  │                                                   │  │
│  │  ┌─────────────────────────────────────────┐    │  │
│  │  │  Host Functions (WASM imports)          │    │  │
│  │  │                                          │    │  │
│  │  │  • vdfs_create_entry()                  │    │  │
│  │  │  • vdfs_write_sidecar()                 │    │  │
│  │  │  • ai_ocr()                             │    │  │
│  │  │  • ai_classify()                        │    │  │
│  │  │  • jobs_dispatch()                      │    │  │
│  │  │  • credentials_store()                  │    │  │
│  │  └────────────┬────────────────────────────┘    │  │
│  │               │ (Direct call)                    │  │
│  │               ▼                                   │  │
│  │  ┌─────────────────────────────────────────┐    │  │
│  │  │  Operation Registry (REUSE!)            │    │  │
│  │  │                                          │    │  │
│  │  │  LIBRARY_QUERIES.get("ai.ocr")          │    │  │
│  │  │  LIBRARY_ACTIONS.get("vdfs.create")     │    │  │
│  │  │  ↓                                       │    │  │
│  │  │  Same handlers used by daemon RPC!      │    │  │
│  │  └─────────────────────────────────────────┘    │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │        Loaded WASM Modules                       │  │
│  │                                                   │  │
│  │  ┌────────────────┐  ┌──────────────┐           │  │
│  │  │ finance.wasm   │  │ vault.wasm   │  ...      │  │
│  │  │                │  │              │           │  │
│  │  │ Calls host     │  │ Calls host   │           │  │
│  │  │ functions ↑    │  │ functions ↑  │           │  │
│  │  └────────────────┘  └──────────────┘           │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

**Key Properties:**
- Extensions are sandboxed WASM modules (cannot access filesystem/network directly)
- Extensions call host functions exposed by Spacedrive
- Host functions route to existing operation handlers
- Same operations used by daemon RPC (code reuse!)
- Single `.wasm` file works on all platforms

### WASM vs. Process-Based

| Aspect | WASM (Recommended) | Process-Based |
|--------|-------------------|---------------|
| **Security** | ⭐⭐⭐⭐True sandbox | ⭐⭐OS isolation |
| **Distribution** | ⭐⭐⭐⭐Single .wasm | ⭐Per-platform binaries |
| **Performance** | ⭐⭐⭐In-process | ⭐⭐IPC overhead |
| **Hot Reload** | ⭐⭐⭐⭐Instant | ⭐Restart required |
| **Memory Safety** | ⭐⭐⭐⭐WASM guarantees | ⭐⭐Depends on extension |
| **Platform Support** | ⭐⭐⭐⭐Universal | ⭐⭐Need builds |
| **Debugging** | ⭐⭐WASM tools | ⭐⭐⭐⭐Native tools |

**Decision: WASM-first for production extensions**

---

## Leveraging Existing Operation Registry

### The Beautiful Part: Code Reuse

Spacedrive already has an **operation registry system** that maps method strings to handlers. The daemon RPC uses this for socket-based clients. We can **reuse the exact same registry** for WASM extensions!

**Location:** `core/src/infra/wire/registry.rs`

**How It Works:**

**Operations self-register at compile time:**

```rust
// Example: OCR operation
pub struct OcrQuery;

impl LibraryQuery for OcrQuery {
    type Input = OcrInput;
    type Output = OcrOutput;

    async fn execute(input: Self::Input, ctx: QueryContext) -> Result<Self::Output> {
        let ai_service = ctx.ai_service();
        ai_service.ocr(&input.data, input.options).await
    }
}

// Register with inventory macro
crate::register_library_query!(OcrQuery, "ai.ocr");
// Adds to global LIBRARY_QUERIES hashmap at compile time
```

**Runtime lookup:**
```rust
// core/src/infra/daemon/rpc.rs (used by daemon RPC)
pub async fn execute_json_operation(
    method: &str,
    library_id: Option<Uuid>,
    json_payload: serde_json::Value,
    core: &Arc<Core>,
) -> Result<serde_json::Value, String> {
    if let Some(handler) = LIBRARY_QUERIES.get(method) {
        return handler(core.context.clone(), session, json_payload).await;
    }
    // ... other registries
}
```

### How WASM Reuses This

**Instead of:** Socket → `execute_json_operation()` → Registry lookup

**We do:** WASM host function → `execute_json_operation()` → Registry lookup

**Same registry, different entry point!**

```rust
// WASM host function bridges to existing registry
fn host_ai_ocr(caller: &mut Caller, input_ptr: u32, input_len: u32) -> u32 {
    // 1. Read from WASM linear memory
    let memory = caller.get_export("memory").unwrap();
    let input_bytes = memory.read(input_ptr, input_len);
    let input_json: serde_json::Value = serde_json::from_slice(&input_bytes).unwrap();

    // 2. Call SAME handler used by daemon RPC!
    let result = tokio::runtime::Handle::current().block_on(async {
        execute_json_operation(
            "query:ai.ocr.v1",        // Same method string
            Some(library_id),          // From WASM context
            input_json,                // Same JSON payload
            &core                      // Same core reference
        ).await
    }).unwrap();

    // 3. Write result back to WASM memory
    let result_bytes = serde_json::to_vec(&result).unwrap();
    let result_ptr = memory.allocate(result_bytes.len());
    memory.write(result_ptr, &result_bytes);
    result_ptr
}
```

**Zero code duplication!** The operation logic is shared.

---

## WASM Host Functions API

### The Extension API Surface

Extensions interact with Spacedrive via **host functions** - Rust functions exposed to the WASM sandbox.

**Design Principle:** Keep the API small and composable. Extensions shouldn't need 100 functions; they need ~15 well-designed primitives.

### The Minimal Host API (ONE Function!)

**The genius insight:** We don't need 15 host functions. We need **ONE generic RPC function** that works exactly like the daemon RPC!

```rust
// WASM guest imports ONLY ONE function
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    /// Generic call to any registered operation
    /// method: Wire method string (e.g., "query:ai.ocr.v1")
    /// library_id: Optional library UUID (as bytes)
    /// payload: JSON input
    /// Returns: JSON output
    fn spacedrive_call(
        method_ptr: u32,
        method_len: u32,
        library_id_ptr: u32,    // 0 if None, else ptr to 16 bytes (UUID)
        payload_ptr: u32,
        payload_len: u32
    ) -> u32;

    /// Optional: Logging helper
    fn spacedrive_log(level: u32, msg_ptr: u32, msg_len: u32);
}
```

**That's the entire WASM API surface!** Everything else goes through the generic `spacedrive_call()` using Wire method strings.

### Host Function Implementation (20 lines!)

```rust
// core/src/infra/extension/host_functions.rs
use wasmer::{FunctionEnvMut, WasmPtr};

/// THE ONLY HOST FUNCTION WE NEED
fn host_spacedrive_call(
    mut env: FunctionEnvMut<PluginEnv>,
    method_ptr: WasmPtr<u8>,
    method_len: u32,
    library_id_ptr: u32,
    payload_ptr: WasmPtr<u8>,
    payload_len: u32,
) -> u32 {
    let (plugin_env, store) = env.data_and_store_mut();
    let memory = plugin_env.memory.clone();

    // 1. Read method string
    let method = memory.view(&store)
        .read_utf8_string(method_ptr, method_len as usize)
        .unwrap();

    // 2. Read library_id (if provided)
    let library_id = if library_id_ptr == 0 {
        None
    } else {
        let uuid_bytes = memory.view(&store)
            .read(library_id_ptr as u64, 16)
            .unwrap();
        Some(Uuid::from_bytes(uuid_bytes.try_into().unwrap()))
    };

    // 3. Read payload JSON
    let payload_bytes = memory.view(&store)
        .read_utf8_string(payload_ptr, payload_len as usize)
        .unwrap();
    let payload_json: serde_json::Value = serde_json::from_str(&payload_bytes).unwrap();

    // 4. Check permissions
    if !plugin_env.permissions.can_call(&method) {
        return write_error_to_wasm(&memory, &store, "Permission denied");
    }

    // 5. Call EXISTING execute_json_operation() - ZERO NEW LOGIC!
    let result = tokio::runtime::Handle::current().block_on(async {
        RpcServer::execute_json_operation(
            &method,           // Same Wire method string!
            library_id,         // Same optional library ID!
            payload_json,       // Same JSON payload!
            &plugin_env.core    // Same core reference!
        ).await
    }).unwrap();

    // 6. Write result to WASM memory
    write_json_to_wasm(&memory, &store, &result)
}
```

**That's IT!** The entire WASM bridge is ~40 lines + memory helpers.

### Why This Is Perfect

**1. Perfect Code Reuse:**
- WASM → `spacedrive_call()` → `execute_json_operation()` → Registry
- Daemon RPC → Socket → `execute_json_operation()` → Registry
- **Same path, different entry point!**

**2. Zero Maintenance:**
- Add new operation? Register it once, works everywhere
- No need to update WASM host functions
- No need to update extension SDK
- Just use the Wire method string!

**3. Type Safety:**
- Wire trait ensures method strings are correct
- JSON validation happens in operation handlers
- Compile-time registration prevents typos

**4. Developer Experience:**
- Extensions use familiar Wire method strings
- Same API as CLI/GraphQL
- Auto-generated documentation from types

---

## WASM Host Implementation

### 1. Plugin Manager (WASM Runtime)

**Purpose:** Load and manage WASM modules

**Location:** `core/src/infra/extension/manager.rs`

```rust
use wasmer::{Store, Module, Instance, imports, Function, FunctionEnv};

pub struct PluginManager {
    store: Store,
    plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
    core: Arc<Core>,
}

struct LoadedPlugin {
    id: String,
    instance: Instance,
    manifest: PluginManifest,
    memory: wasmer::Memory,
    loaded_at: DateTime<Utc>,
}

impl PluginManager {
    pub fn new(core: Arc<Core>) -> Self {
        let store = Store::default();
        Self {
            store,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            core,
        }
    }

    /// Load WASM plugin from file
    pub async fn load_plugin(&mut self, wasm_path: &Path) -> Result<()> {
        // 1. Load manifest
        let manifest = self.load_manifest_for_wasm(wasm_path)?;

        // 2. Compile WASM module
        let wasm_bytes = std::fs::read(wasm_path)?;
        let module = Module::new(&self.store, wasm_bytes)?;

        // 3. Create host function environment
        let env = FunctionEnv::new(&mut self.store, PluginEnv {
            extension_id: manifest.id.clone(),
            core: self.core.clone(),
            library_id: None, // Set by extension
        });

        // 4. Create import object with host functions
        let imports = imports! {
            "spacedrive" => {
                // ONE generic function for all operations!
                "spacedrive_call" => Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    host_spacedrive_call
                ),
                // Optional logging helper
                "spacedrive_log" => Function::new_typed_with_env(
                    &mut self.store,
                    &env,
                    host_spacedrive_log
                ),
            }
        };

        // 5. Instantiate WASM module
        let instance = Instance::new(&mut self.store, &module, &imports)?;

        // 6. Get memory export
        let memory = instance.exports.get_memory("memory")?.clone();

        // 7. Call plugin init function
        let init = instance.exports.get_function("plugin_init")?;
        init.call(&mut self.store, &[])?;

        // 8. Store loaded plugin
        self.plugins.write().await.insert(
            manifest.id.clone(),
            LoadedPlugin {
                id: manifest.id.clone(),
                instance,
                manifest,
                memory,
                loaded_at: Utc::now(),
            }
        );

        Ok(())
    }

    /// Unload plugin
    pub async fn unload_plugin(&mut self, plugin_id: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.write().await.remove(plugin_id) {
            // Call cleanup function
            let cleanup = plugin.instance.exports.get_function("plugin_cleanup")?;
            cleanup.call(&mut self.store, &[])?;
        }
        Ok(())
    }

    /// Reload plugin (hot-reload during development)
    pub async fn reload_plugin(&mut self, plugin_id: &str, wasm_path: &Path) -> Result<()> {
        self.unload_plugin(plugin_id).await?;
        self.load_plugin(wasm_path).await?;
        Ok(())
    }
}

### 2. Complete Host Function Implementation

**Location:** `core/src/infra/extension/host_functions.rs`

```rust
use wasmer::{FunctionEnvMut, WasmPtr, Memory, Store};
use crate::infra::daemon::rpc::RpcServer;

/// Environment passed to host functions
pub struct PluginEnv {
    pub extension_id: String,
    pub core: Arc<Core>,
    pub permissions: ExtensionPermissions,
    pub memory: Memory,
}

/// THE ONLY HOST FUNCTION - Generic Wire RPC
fn host_spacedrive_call(
    mut env: FunctionEnvMut<PluginEnv>,
    method_ptr: WasmPtr<u8>,
    method_len: u32,
    library_id_ptr: u32,
    payload_ptr: WasmPtr<u8>,
    payload_len: u32,
) -> u32 {
    let (plugin_env, store) = env.data_and_store_mut();
    let memory = plugin_env.memory.clone();

    // 1. Read method string from WASM memory
    let method = read_string_from_wasm(&memory, &store, method_ptr, method_len);

    // 2. Read library_id (0 = None)
    let library_id = if library_id_ptr == 0 {
        None
    } else {
        Some(read_uuid_from_wasm(&memory, &store, library_id_ptr))
    };

    // 3. Read payload JSON
    let payload_str = read_string_from_wasm(&memory, &store, payload_ptr, payload_len);
    let payload_json: serde_json::Value = serde_json::from_str(&payload_str)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to parse payload JSON: {}", e);
            serde_json::Value::Null
        });

    // 4. Permission check
    if !plugin_env.permissions.can_call(&method) {
        tracing::warn!(
            "Extension {} denied permission to call {}",
            plugin_env.extension_id,
            method
        );
        return write_error_to_wasm(&memory, &store, "Permission denied");
    }

    // 5. Call EXISTING execute_json_operation()
    // This is the EXACT same function used by daemon RPC!
    let result = tokio::runtime::Handle::current().block_on(async {
        RpcServer::execute_json_operation(
            &method,
            library_id,
            payload_json,
            &plugin_env.core
        ).await
    });

    // 6. Write result to WASM memory
    match result {
        Ok(json) => write_json_to_wasm(&memory, &store, &json),
        Err(e) => write_error_to_wasm(&memory, &store, &e),
    }
}

/// Optional logging helper
fn host_spacedrive_log(
    env: FunctionEnvMut<PluginEnv>,
    level: u32,
    msg_ptr: WasmPtr<u8>,
    msg_len: u32,
) {
    let (plugin_env, store) = env.data_and_store_mut();
    let memory = plugin_env.memory.clone();

    let message = read_string_from_wasm(&memory, &store, msg_ptr, msg_len);

    let log_level = match level {
        0 => tracing::Level::DEBUG,
        1 => tracing::Level::INFO,
        2 => tracing::Level::WARN,
        3 => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing::event!(
        log_level,
        extension = %plugin_env.extension_id,
        "{}",
        message
    );
}

// === Memory Helpers ===

fn read_string_from_wasm(
    memory: &Memory,
    store: &Store,
    ptr: WasmPtr<u8>,
    len: u32
) -> String {
    let bytes = memory.view(&store)
        .read(ptr.offset() as u64, len as usize)
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

fn read_uuid_from_wasm(memory: &Memory, store: &Store, ptr: u32) -> Uuid {
    let bytes = memory.view(&store)
        .read(ptr as u64, 16)
        .unwrap();
    Uuid::from_bytes(bytes.try_into().unwrap())
}

fn write_json_to_wasm(memory: &Memory, store: &Store, json: &serde_json::Value) -> u32 {
    let json_str = serde_json::to_string(json).unwrap();
    let bytes = json_str.as_bytes();

    // Call WASM guest's allocate function
    let alloc_fn = memory.view(&store).get_function("wasm_alloc").unwrap();
    let result = alloc_fn.call(&mut store, &[wasmer::Value::I32(bytes.len() as i32)])
        .unwrap();
    let ptr = result[0].unwrap_i32() as u32;

    // Write data
    memory.view(&store).write(ptr as u64, bytes).unwrap();

    ptr
}

fn write_error_to_wasm(memory: &Memory, store: &Store, error: &str) -> u32 {
    let error_json = json!({ "error": error });
    write_json_to_wasm(memory, store, &error_json)
}
```

**Total: ~100 lines** for the entire WASM bridge (vs. 500+ with per-function approach)!

### 3. Permission System

**Purpose:** Capability-based security for WASM extensions

**Location:** `core/src/infra/extension/permissions.rs`

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct ExtensionPermissions {
    /// Methods this extension can call (prefix matching)
    pub allowed_methods: Vec<String>,

    /// Libraries this extension can access
    pub allowed_libraries: Vec<Uuid>,  // or ["*"] for all

    /// Rate limiting
    pub max_requests_per_minute: usize,
    pub max_concurrent_jobs: usize,

    /// Resource limits (enforced by WASM runtime)
    pub max_memory_bytes: usize,
    pub max_cpu_time_ms: u64,

    /// Network access (for extensions that need external APIs)
    pub allowed_domains: Vec<String>,
}

impl ExtensionPermissions {
    /// Check if extension can call this method
    pub fn can_call(&self, method: &str) -> bool {
        self.allowed_methods.iter().any(|prefix| {
            method.starts_with(prefix)
        })
    }

    /// Check if extension can access library
    pub fn can_access_library(&self, library_id: Uuid) -> bool {
        self.allowed_libraries.iter().any(|id| {
            id == &library_id || id.to_string() == "*"
        })
    }
}

/// Load permissions from manifest
impl ExtensionPermissions {
    pub fn from_manifest(manifest: &ExtensionManifest) -> Self {
        Self {
            allowed_methods: manifest.permissions.methods.clone(),
            allowed_libraries: manifest.permissions.libraries.clone(),
            max_requests_per_minute: manifest.permissions.rate_limits
                .requests_per_minute,
            max_concurrent_jobs: manifest.permissions.rate_limits
                .concurrent_jobs,
            max_memory_bytes: manifest.permissions.max_memory_mb * 1024 * 1024,
            max_cpu_time_ms: 5000, // 5 seconds per call
            allowed_domains: manifest.permissions.network_access.clone(),
        }
    }
}
```

**Enforcement:** Permissions checked in every host function (see above).

### 4. Resource Limits (WASM Runtime)

**Wasmer provides built-in resource limiting:**

```rust
use wasmer::{Store, BaseTunables, Target};
use wasmer_middlewares::Metering;

impl PluginManager {
    fn create_store_with_limits(permissions: &ExtensionPermissions) -> Store {
        // CPU metering (prevents infinite loops)
        let metering = Arc::new(Metering::new(10_000_000, |_| 1));
        let mut tunables = BaseTunables::for_target(&Target::default());

        // Memory limits
        tunables.set_memory_style(wasmer::vm::MemoryStyle::Static {
            bound: permissions.max_memory_bytes / (64 * 1024), // Pages
            offset_guard_size: 128 * 1024,
        });

        Store::new_with_tunables(metering, tunables)
    }
}
```

**WASM Security Benefits:**
- Cannot access filesystem directly (must use host functions)
- Cannot make network calls directly (must use host functions)
- Cannot escape sandbox (WASM guarantees)
- Memory isolated (cannot read host process memory)
- CPU bounded (metering prevents DoS)
- Memory bounded (runtime enforces limits)

---

## New Operations to Register

Extensions need access to operations that may not exist yet. We add them to the **existing registry system** - no changes to WASM infrastructure needed!

### Operations to Add

#### 1. AI Service Operations (New)

```rust
// core/src/ops/ai/ocr.rs
pub struct OcrQuery;

#[derive(Serialize, Deserialize)]
pub struct OcrInput {
    pub data: Vec<u8>,  // base64-encoded PDF or image
    pub options: OcrOptions,
}

#[derive(Serialize, Deserialize)]
pub struct OcrOptions {
    pub language: String,  // "eng", "fra", etc.
    pub engine: OcrEngine,
    pub preprocessing: bool,
}

#[derive(Serialize, Deserialize)]
pub enum OcrEngine {
    Tesseract,
    EasyOcr,
}

#[derive(Serialize, Deserialize)]
pub struct OcrOutput {
    pub text: String,
    pub confidence: f32,
    pub extracted_at: DateTime<Utc>,
}

#[async_trait]
impl LibraryQuery for OcrQuery {
    type Input = OcrInput;
    type Output = OcrOutput;

    async fn execute(input: Self::Input, ctx: QueryContext) -> Result<Self::Output> {
        // Get AI service (might not exist yet - needs implementation)
        let ai_service = ctx.core_context().ai_service();
        let result = ai_service.ocr(&input.data, input.options).await?;

        Ok(OcrOutput {
            text: result.text,
            confidence: result.confidence,
            extracted_at: Utc::now(),
        })
    }
}

// Register once, available everywhere (daemon RPC + WASM host functions)
crate::register_library_query!(OcrQuery, "ai.ocr");
```

#### 2. Credential Operations (New)

```rust
// core/src/ops/credentials/store.rs
pub struct StoreCredentialAction;

#[derive(Serialize, Deserialize)]
pub struct StoreCredentialInput {
    pub credential_id: String,
    pub credential_type: CredentialType,
    pub data: CredentialData,
}

#[derive(Serialize, Deserialize)]
pub enum CredentialType {
    OAuth2 {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: DateTime<Utc>,
    },
    ApiKey { key: String },
}

#[async_trait]
impl LibraryAction for StoreCredentialAction {
    type Input = StoreCredentialInput;
    type Output = StoreCredentialOutput;

    async fn execute(input: Self::Input, ctx: ActionContext) -> Result<Self::Output> {
        // Store in encrypted credential vault
        let vault = ctx.credential_vault();
        vault.store(input.credential_id, input.credential_type, input.data).await?;

        Ok(StoreCredentialOutput { success: true })
    }
}

crate::register_library_action!(StoreCredentialAction, "credentials.store");

// Similar for credentials.get, credentials.refresh_oauth
```

#### 3. VDFS Sidecar Operations (Might Not Exist)

```rust
// core/src/ops/vdfs/sidecar.rs
pub struct WriteSidecarAction;

#[derive(Serialize, Deserialize)]
pub struct WriteSidecarInput {
    pub entry_id: Uuid,
    pub filename: String,
    pub data: Vec<u8>,  // Raw bytes or base64
}

#[async_trait]
impl LibraryAction for WriteSidecarAction {
    type Input = WriteSidecarInput;
    type Output = WriteSidecarOutput;

    async fn execute(input: Self::Input, ctx: ActionContext) -> Result<Self::Output> {
        let library = ctx.library();
        library.write_sidecar(&input.entry_id, &input.filename, &input.data).await?;

        Ok(WriteSidecarOutput { success: true })
    }
}

crate::register_library_action!(WriteSidecarAction, "vdfs.write_sidecar");
```

### Total Implementation Work

**New Operations to Add:** ~10-15 operations

**Lines of Code per Operation:** ~50-100 (simple wrappers around existing services)

**Total:** 500-1500 lines to add all extension operations

**Timeline:** 1-2 weeks for one engineer

**Key Point:** These operations are useful for ALL clients (CLI, GraphQL, extensions), not just WASM plugins!

---

## Complete Code Examples

### WASM Extension (Guest Code)

**Complete Finance extension using the single `spacedrive_call()` function:**

```rust
// spacedrive-finance/src/lib.rs (compiled to WASM)
use serde::{Serialize, Deserialize};
use serde_json::json;

// Import the ONE host function
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    fn spacedrive_call(
        method_ptr: *const u8,
        method_len: usize,
        library_id_ptr: u32,
        payload_ptr: *const u8,
        payload_len: usize
    ) -> u32;

    fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
}

/// High-level wrapper around spacedrive_call
fn call_spacedrive(
    method: &str,
    library_id: Option<Uuid>,
    payload: serde_json::Value
) -> Result<serde_json::Value> {
    // Serialize payload
    let payload_json = serde_json::to_string(&payload)?;

    // Prepare library_id (0 = None, or write UUID bytes)
    let lib_id_ptr = match library_id {
        None => 0,
        Some(uuid) => {
            let uuid_bytes = uuid.as_bytes();
            uuid_bytes.as_ptr() as u32
        }
    };

    // Call host function
    let result_ptr = unsafe {
        spacedrive_call(
            method.as_ptr(),
            method.len(),
            lib_id_ptr,
            payload_json.as_ptr(),
            payload_json.len()
        )
    };

    // Read result
    let result_str = unsafe {
        let len = *(result_ptr as *const u32);
        let data_ptr = (result_ptr + 4) as *const u8;
        let slice = std::slice::from_raw_parts(data_ptr, len as usize);
        std::str::from_utf8(slice)?
    };

    let result: serde_json::Value = serde_json::from_str(result_str)?;

    // Free memory
    unsafe { wasm_free(result_ptr) };

    Ok(result)
}

/// Process a receipt email
async fn process_receipt(
    email_data: Vec<u8>,
    library_id: Uuid
) -> Result<Uuid> {
    // 1. Create VDFS entry
    let entry_result = call_spacedrive(
        "action:vdfs.create_entry.input.v1",  // Wire method!
        Some(library_id),
        json!({
            "name": "Receipt: Unknown Vendor",
            "path": "extensions/finance/receipts/pending.eml",
            "entry_type": "FinancialDocument"
        })
    )?;

    let entry_id: Uuid = serde_json::from_value(entry_result["entry_id"].clone())?;

    // 2. Store email sidecar
    call_spacedrive(
        "action:vdfs.write_sidecar.input.v1",
        Some(library_id),
        json!({
            "entry_id": entry_id,
            "filename": "email.json",
            "data": base64::encode(&email_data)
        })
    )?;

    // 3. Run OCR on PDF attachment
    let ocr_result = call_spacedrive(
        "query:ai.ocr.v1",
        Some(library_id),
        json!({
            "data": base64::encode(&extract_pdf_attachment(&email_data)?),
            "options": {
                "language": "eng",
                "engine": "Tesseract"
            }
        })
    )?;

    let ocr_text: String = serde_json::from_value(ocr_result["text"].clone())?;

    // 4. Classify receipt with AI
    let classify_result = call_spacedrive(
        "query:ai.classify_text.v1",
        Some(library_id),
        json!({
            "text": ocr_text,
            "prompt": "Extract vendor, amount, date, category from this receipt. Return JSON.",
            "options": {
                "model": "user_default",
                "temperature": 0.1
            }
        })
    )?;

    // 5. Store analysis sidecar
    call_spacedrive(
        "action:vdfs.write_sidecar.input.v1",
        Some(library_id),
        json!({
            "entry_id": entry_id,
            "filename": "receipt_analysis.json",
            "data": serde_json::to_vec(&classify_result)?
        })
    )?;

    // 6. Update entry metadata for search
    call_spacedrive(
        "action:vdfs.update_metadata.input.v1",
        Some(library_id),
        json!({
            "entry_id": entry_id,
            "metadata": classify_result
        })
    )?;

    Ok(entry_id)
}

/// Plugin entrypoint called by Spacedrive
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    unsafe { spacedrive_log(1, b"Finance plugin initialized".as_ptr(), 29) };
    0 // Success
}

#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
    0 // Success
}

/// Memory allocation for host to write results
#[no_mangle]
pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
    let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

#[no_mangle]
pub extern "C" fn wasm_free(ptr: u32) {
    // Free memory allocated by wasm_alloc
    unsafe {
        let size = *(ptr as *const u32);
        let layout = std::alloc::Layout::from_size_align(size as usize, 1).unwrap();
        std::alloc::dealloc(ptr as *mut u8, layout);
    }
}
```

**That's a complete WASM extension!** Notice:
- Uses Wire method strings (`"query:ai.ocr.v1"`)
- Same payloads as daemon RPC
- Single `spacedrive_call()` function for everything
- ~150 lines for a working receipt processor

### WASM Extension SDK (Ergonomic Wrapper)

**We'd provide a Rust SDK to make this even easier:**

```rust
// spacedrive-sdk/src/lib.rs (published as crate)
pub struct SpacedriveClient {
    library_id: Uuid,
}

impl SpacedriveClient {
    pub fn new(library_id: Uuid) -> Self {
        Self { library_id }
    }

    /// Generic operation caller
    pub fn call<I, O>(
        &self,
        method: &str,
        input: &I
    ) -> Result<O>
    where
        I: Serialize,
        O: DeserializeOwned,
    {
        let payload = serde_json::to_value(input)?;
        let result = call_spacedrive(method, Some(self.library_id), payload)?;
        Ok(serde_json::from_value(result)?)
    }

    // Convenience methods with type safety
    pub fn create_entry(&self, input: CreateEntryInput) -> Result<Uuid> {
        let result = self.call("action:vdfs.create_entry.input.v1", &input)?;
        Ok(result)
    }

    pub fn write_sidecar(&self, entry_id: Uuid, filename: &str, data: &[u8]) -> Result<()> {
        self.call("action:vdfs.write_sidecar.input.v1", &WriteSidecarInput {
            entry_id,
            filename: filename.to_string(),
            data: data.to_vec(),
        })
    }

    pub fn ocr(&self, data: &[u8], options: OcrOptions) -> Result<OcrOutput> {
        self.call("query:ai.ocr.v1", &OcrInput { data: data.to_vec(), options })
    }

    pub fn classify_text(&self, text: &str, prompt: &str) -> Result<serde_json::Value> {
        self.call("query:ai.classify_text.v1", &ClassifyInput {
            text: text.to_string(),
            prompt: prompt.to_string(),
            options: ClassifyOptions::default(),
        })
    }
}

// Now extension code is clean:
fn process_receipt(email: Vec<u8>, client: &SpacedriveClient) -> Result<Uuid> {
    let entry_id = client.create_entry(CreateEntryInput {
        name: "Receipt".to_string(),
        path: "receipts/new.eml".to_string(),
        entry_type: "FinancialDocument".to_string(),
    })?;

    client.write_sidecar(entry_id, "email.json", &email)?;

    let pdf = extract_pdf(&email)?;
    let ocr_result = client.ocr(&pdf, OcrOptions::default())?;
    let receipt = client.classify_text(&ocr_result.text, "Extract receipt data")?;

    client.write_sidecar(entry_id, "receipt.json", &serde_json::to_vec(&receipt)?)?;

    Ok(entry_id)
}
```

**Developer experience:**
- Import `spacedrive-sdk` crate
- Use type-safe methods
- Wire methods handled internally
- Compile to WASM

---

## Security & Isolation

### Threat Model

**What we protect against:**
1. Malicious extension accessing unauthorized libraries
2. Malicious extension calling privileged operations
3. Malicious extension DoS-ing core via spam requests
4. Malicious extension reading other extension's data

**What we DON'T protect against:**
- Memory corruption (extensions are separate processes)
- Resource exhaustion (OS handles process limits)
- Local privilege escalation (OS security model)

### Security Layers

**Layer 1: OS Process Isolation**
- Extensions run as separate processes
- Cannot access each other's memory
- Cannot modify each other's files
- OS enforces resource limits

**Layer 2: Socket Permissions**
- Unix socket has file permissions (0600 = owner only)
- Only processes running as same user can connect
- Optional: per-extension sockets

**Layer 3: Permission Checking**
- Extension manifest declares required permissions
- Permission checker validates every request
- Method-level and library-level ACLs

**Layer 4: Rate Limiting**
- Per-extension request quotas
- Prevents DoS attacks
- Enforced at connection level

### Permission Manifest Example

```json
{
  "id": "finance",
  "name": "Spacedrive Finance",
  "permissions": {
    "methods": [
      "vdfs.create_entry",
      "vdfs.write_sidecar",
      "ai.ocr",
      "ai.classify_text",
      "credentials.store",
      "credentials.get",
      "jobs.dispatch"
    ],
    "libraries": ["*"],  // All libraries, or specific UUIDs
    "rate_limits": {
      "requests_per_minute": 1000,
      "concurrent_jobs": 10
    },
    "network_access": [
      "https://www.googleapis.com",
      "https://graph.microsoft.com"
    ]
  }
}
```

---

## Implementation Plan

### Phase 1: WASM Runtime Integration (Week 1-2)

**Goal:** Load and execute basic WASM modules

**Tasks:**
- [ ] Add `wasmer` dependency to `core/Cargo.toml`
- [ ] Create `core/src/infra/extension/` module
- [ ] Implement `PluginManager` (load/unload WASM)
- [ ] Implement `host_spacedrive_call()` (THE generic host function)
- [ ] Test with "hello world" WASM module

**Deliverable:** Can load .wasm file and call `plugin_init()`

**Code:**
```rust
// core/Cargo.toml
[dependencies]
wasmer = "4.2"
wasmer-middlewares = "4.2"
```

### Phase 2: Wire Integration (Week 3)

**Goal:** WASM can call existing registered operations

**Tasks:**
- [ ] Implement memory helpers (read/write JSON from WASM)
- [ ] Connect `host_spacedrive_call()` to `execute_json_operation()`
- [ ] Test calling existing operations (e.g., `query:vdfs.list_entries.v1`)
- [ ] Implement permission checking

**Deliverable:** WASM extension can query VDFS entries

### Phase 3: Extension-Specific Operations (Week 4-5)

**Goal:** Add operations needed by Finance extension

**Tasks:**
- [ ] Add `ai.ocr` operation to registry
- [ ] Add `ai.classify_text` operation
- [ ] Add `credentials.store` and `credentials.get`
- [ ] Add `vdfs.write_sidecar` (if missing)
- [ ] Test each operation from WASM

**Deliverable:** All Finance extension APIs available

### Phase 4: Extension SDK (Week 6)

**Goal:** Make WASM development ergonomic

**Tasks:**
- [ ] Create `spacedrive-sdk` Rust crate
- [ ] Implement `SpacedriveClient` wrapper
- [ ] Type-safe operation methods
- [ ] Documentation and examples
- [ ] Publish to crates.io

**Deliverable:** `cargo add spacedrive-sdk` works

### Phase 5: Finance Extension (Week 7-9)

**Goal:** Build first revenue-generating extension

**Tasks:**
- [ ] Gmail OAuth (via external HTTP - WASM limitation, see below)
- [ ] Email scanning logic
- [ ] Receipt processing pipeline
- [ ] Compile to WASM and test
- [ ] UI integration

**Deliverable:** Working Finance extension MVP

### WASM Limitation: External Network Calls

**Problem:** WASM cannot make HTTP requests directly (no socket access)

**Solutions:**

**Option 1: Proxy via Host Function**
```rust
// Add host function for HTTP
fn spacedrive_http_request(
    url_ptr: u32,
    method_ptr: u32,
    body_ptr: u32,
    headers_ptr: u32
) -> u32;

// Extension uses it
let response = call_http(
    "https://www.googleapis.com/gmail/v1/messages",
    "GET",
    headers
)?;
```

**Option 2: Native Extension Component**
- WASM handles logic (receipt processing)
- Small native binary handles OAuth/HTTP
- Communicate via JSON messages

**Recommendation:** Option 1 (simpler, more secure)

---

## Summary: The Genius of This Approach

### What Makes This Brilliant

**1. Minimal API Surface**
- ONE host function: `spacedrive_call()`
- Optional helper: `spacedrive_log()`
- Total: **2 host functions** vs. 15+ in traditional FFI

**2. Perfect Code Reuse**
```
WASM Extension:
  spacedrive_call("query:ai.ocr.v1", lib_id, payload)
    ↓
  host_spacedrive_call() [~40 lines]
    ↓
  RpcServer::execute_json_operation() [existing!]
    ↓
  LIBRARY_QUERIES.get("query:ai.ocr.v1") [existing!]
    ↓
  OcrQuery::execute() [existing!]
```

**3. Zero Maintenance Overhead**
- Add new operation? Just `register_library_query!()` - works in WASM automatically
- No need to update host functions
- No need to update SDK
- No code generation needed

**4. Type Safety**
- Wire trait ensures correct method strings
- JSON schemas validated by operation handlers
- Compile-time registration (inventory crate)

### Implementation Complexity

**Total Lines of Code:**
- WASM runtime integration: ~300 lines
- Host function (`spacedrive_call`): ~100 lines
- Permission system: ~200 lines
- Memory helpers: ~100 lines
- **Total: ~700 lines**

Compare to:
- New IPC system: ~5,000 lines
- Per-function FFI: ~2,000 lines

### Timeline

**6-7 weeks** from start to Finance extension MVP:
- Week 1-2: WASM runtime + basic loading
- Week 3: Wire integration + testing
- Week 4-5: Add extension operations
- Week 6: Extension SDK
- Week 7+: Finance extension

### What We Get

Universal platform (single .wasm works everywhere)
True sandbox security (WASM isolation)
Hot-reload during development
Perfect code reuse (operation registry)
Type-safe API (Wire trait)
Minimal maintenance burden
Extensible without touching host code

---

## Appendix: Optional Process-Based Prototype

**If you want to validate the Finance extension before building WASM platform:**

You could build a temporary process-based version using the existing `DaemonClient`. This would:
- Ship faster (no WASM work needed)
- Validate revenue model
- Learn what APIs extensions need

Then migrate to WASM once the platform is ready. See [Migration Path section](#migration-path-optional-process-based-prototype) for details.

**However:** Given the simplicity of the WASM approach (just ~700 lines), we recommend **building WASM first**. The timeline is similar and you get all the platform benefits immediately.

---

*This design leverages Spacedrive's existing Wire/Registry infrastructure through a single generic WASM host function. It's simpler, more maintainable, and more secure than traditional approaches.*

