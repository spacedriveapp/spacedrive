# WASM Extension Architecture - Final Design

## The Elegant Solution

**ONE generic host function that reuses the entire existing Wire/Registry infrastructure.**

```
┌─────────────────────────────────────────────────────────────────┐
│                    Spacedrive Core                              │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │        WASM Plugin Host (Wasmer Runtime)                 │  │
│  │                                                           │  │
│  │   Finance.wasm    Vault.wasm    Photos.wasm    ...       │  │
│  │        │               │              │                   │  │
│  │        └───────────────┴──────────────┘                   │  │
│  │                        │                                   │  │
│  │                        │  All call:                        │  │
│  │                        ▼                                   │  │
│  │          spacedrive_call(method, lib_id, payload)         │  │
│  │                        │                                   │  │
│  └────────────────────────┼───────────────────────────────────┘  │
│                           │                                      │
│                           ▼                                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │   RpcServer::execute_json_operation()                    │  │
│  │   (EXISTING - used by daemon RPC!)                       │  │
│  └────────────────────────┬─────────────────────────────────┘  │
│                           │                                      │
│                           ▼                                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │   Operation Registry (inventory crate)                   │  │
│  │                                                           │  │
│  │   LIBRARY_QUERIES:                                       │  │
│  │   • "query:ai.ocr.v1" → OcrQuery::execute()             │  │
│  │   • "query:ai.classify_text.v1" → ClassifyQuery::exec() │  │
│  │   • ...                                                  │  │
│  │                                                           │  │
│  │   LIBRARY_ACTIONS:                                       │  │
│  │   • "action:vdfs.create_entry.input.v1" → Create::exec()│  │
│  │   • "action:vdfs.write_sidecar.input.v1" → Write::exec()│  │
│  │   • ...                                                  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## The Complete API

### Host Functions (Rust → WASM)

**Total: 2 functions**

```rust
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    /// Generic operation call - routes to Wire registry
    fn spacedrive_call(
        method_ptr: *const u8,      // Wire method string
        method_len: usize,
        library_id_ptr: u32,         // 0 = None, else UUID bytes
        payload_ptr: *const u8,      // JSON input
        payload_len: usize
    ) -> u32;                        // Returns JSON output ptr

    /// Logging helper
    fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
}
```

### Extension SDK (Wrapper)

```rust
// spacedrive-sdk provides ergonomic API
pub struct SpacedriveClient {
    library_id: Uuid,
}

impl SpacedriveClient {
    // Type-safe operations
    pub fn create_entry(&self, input: CreateEntryInput) -> Result<Uuid>;
    pub fn write_sidecar(&self, entry_id: Uuid, filename: &str, data: &[u8]) -> Result<()>;
    pub fn ocr(&self, data: &[u8], options: OcrOptions) -> Result<OcrOutput>;
    pub fn classify_text(&self, text: &str, prompt: &str) -> Result<serde_json::Value>;

    // Generic caller for any Wire operation
    pub fn call<I, O>(&self, method: &str, input: &I) -> Result<O>
    where I: Serialize, O: DeserializeOwned;
}
```

### Extension Code Example

```rust
use spacedrive_sdk::SpacedriveClient;

fn process_receipt(email: Vec<u8>, client: &SpacedriveClient) -> Result<Uuid> {
    // Clean, type-safe API
    let entry_id = client.create_entry(CreateEntryInput {
        name: "Receipt: Starbucks",
        path: "receipts/new.eml",
        entry_type: "FinancialDocument",
    })?;

    client.write_sidecar(entry_id, "email.json", &email)?;

    let pdf = extract_pdf(&email)?;
    let ocr = client.ocr(&pdf, OcrOptions::default())?;
    let receipt = client.classify_text(&ocr.text, "Extract receipt data")?;

    client.write_sidecar(entry_id, "receipt.json", &serde_json::to_vec(&receipt)?)?;

    Ok(entry_id)
}
```

---

## Implementation Checklist

### Core Components (~700 lines total)

**1. WASM Plugin Manager** (`core/src/infra/extension/manager.rs`)
- [ ] Load WASM modules with Wasmer
- [ ] Plugin lifecycle (init/cleanup)
- [ ] Hot-reload support
- [ ] Plugin registry
- **~300 lines**

**2. Host Functions** (`core/src/infra/extension/host_functions.rs`)
- [ ] `host_spacedrive_call()` - Generic Wire RPC
- [ ] `host_spacedrive_log()` - Logging helper
- [ ] Memory helpers (read/write WASM memory)
- [ ] Bridge to `execute_json_operation()`
- **~100 lines**

**3. Permission System** (`core/src/infra/extension/permissions.rs`)
- [ ] Load permissions from manifest
- [ ] Check method permissions
- [ ] Rate limiting
- [ ] Resource limits (via Wasmer)
- **~200 lines**

**4. Extension SDK** (`spacedrive-sdk/src/lib.rs`)
- [ ] `SpacedriveClient` wrapper
- [ ] Type-safe operation methods
- [ ] WASM memory management
- [ ] Error handling
- **~400 lines** (separate crate)

### New Operations to Register (~500 lines)

**AI Operations:**
- [ ] `OcrQuery` - Extract text from images/PDFs
- [ ] `ClassifyTextQuery` - AI text classification
- [ ] `GenerateEmbeddingQuery` - Semantic embeddings

**Credential Operations:**
- [ ] `StoreCredentialAction` - Save OAuth tokens
- [ ] `GetCredentialQuery` - Retrieve credentials (auto-refresh)

**VDFS Operations:**
- [ ] `WriteSidecarAction` - Store sidecar files
- [ ] `ReadSidecarQuery` - Read sidecar files
- [ ] `UpdateMetadataAction` - Update entry metadata

**HTTP Operations (for WASM):**
- [ ] `HttpRequestQuery` - Proxy HTTP calls for extensions

### Timeline

**Week 1-2: WASM Runtime**
- Integrate Wasmer
- Load basic .wasm module
- Call `plugin_init()`

**Week 3: Wire Bridge**
- Implement `host_spacedrive_call()`
- Connect to `execute_json_operation()`
- Test calling existing operations

**Week 4-5: Operations**
- Add AI operations
- Add credential operations
- Add VDFS sidecar operations
- Add HTTP proxy

**Week 6: SDK**
- Build `spacedrive-sdk` crate
- Type-safe wrappers
- Documentation
- Publish to crates.io

**Week 7+: Finance Extension**
- Build receipt processing logic
- Compile to WASM
- Test end-to-end
- Launch!

---

## The Key Decisions

### 1. WASM-Only (No Process-Based)

**Rationale:**
- WASM gives us: security, distribution, hot-reload, universality
- Implementation complexity is low (~700 lines)
- Timeline is reasonable (6-7 weeks)
- Gets us the platform benefits immediately

### 2. Generic `spacedrive_call()` (Not Per-Function FFI)

**Rationale:**
- Minimal API surface (2 functions vs. 15+)
- Perfect code reuse (operation registry)
- Zero maintenance (add operations without touching host)
- Type safety via Wire trait

### 3. HTTP Proxy Host Function

**Rationale:**
- WASM can't make HTTP calls directly
- Extensions need OAuth/API access
- Controlled via manifest permissions
- More secure than native network access

---

## Next Steps

1. **Review This Design** with team
2. **Prototype** `host_spacedrive_call()` bridging to `execute_json_operation()`
3. **Add First Operation** (e.g., `ai.ocr`)
4. **Test From WASM** module
5. **Build Finance Extension** in parallel with platform

---

## Questions to Resolve

1. **HTTP Proxy:** How restrictive? Allow any domain in manifest, or curated list?
2. **Async in WASM:** Use `wasm-bindgen-futures` or make host functions blocking?
3. **Error Handling:** Return errors as JSON `{error: "..."}` or throw WASM traps?
4. **Event Subscriptions:** How do WASM extensions subscribe to events?
5. **Job Execution:** Should WASM extensions define jobs, or just trigger core jobs?

**Recommendations:**
1. Allow any domain if in manifest `allowed_domains`
2. Make host functions blocking (simpler), use Tokio runtime internally
3. Return errors as JSON (more graceful)
4. Extensions export callback functions, host calls them when events fire
5. Extensions trigger core jobs via `jobs.dispatch` (don't define custom job types yet)

---

*Ready to start implementation: begin with WASM runtime integration and `host_spacedrive_call()`!*

