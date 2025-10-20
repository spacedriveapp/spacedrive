# WASM Extension System

**Status:** Basic structure integrated, compiling successfully

This module provides Spacedrive's WebAssembly-based extension system, enabling secure, sandboxed plugins.

## What's Implemented

### Core Infrastructure

- **`manager.rs`** - PluginManager for loading/unloading WASM modules (Wasmer integration)
- **`host_functions.rs`** - Skeleton for `host_spacedrive_call()` and `host_spacedrive_log()`
- **`permissions.rs`** - Capability-based security with rate limiting
- **`types.rs`** - Extension manifest format and types

### Dependencies Added

```toml
wasmer = "4.2"
wasmer-middlewares = "4.2"
```

## The Design

**Key Insight:** ONE generic host function reuses the entire Wire/Registry infrastructure.

```rust
// WASM extensions import:
extern "C" {
    fn spacedrive_call(method, library_id, payload) -> result;
}

// Host function routes to existing registry:
host_spacedrive_call()
  ↓
RpcServer::execute_json_operation()  // EXISTING!
  ↓
LIBRARY_QUERIES/ACTIONS.get()        // EXISTING!
  ↓
Operation::execute()                  // EXISTING!
```

**Result:** Zero code duplication. WASM extensions use same operations as CLI/GraphQL/daemon clients.

## What's NOT Implemented Yet

### Pending Work

**1. WASM Memory Interaction** (`host_functions.rs`)

- Read/write strings from WASM linear memory
- Read/write JSON payloads
- UUID handling
- Guest allocator integration

**2. Full Wire Bridge** (`host_functions.rs`)

- Call `RpcServer::execute_json_operation()`
- Permission checking before operation
- Error handling and propagation

**3. Extension Operations** (`core/src/ops/`)

- `ai.ocr` - OCR operation
- `ai.classify_text` - AI classification
- `credentials.store/get` - Credential management
- `vdfs.write_sidecar` - Sidecar file operations

**4. Test WASM Module**

- Simple "hello world" .wasm file
- Calls `spacedrive_call()` to test integration
- Validates permission system

**5. Extension SDK** (separate crate)

- `spacedrive-sdk` Rust crate
- Type-safe wrapper around `spacedrive_call()`
- Ergonomic API for extension developers

## Next Steps

### Immediate (This Week)

1. **Implement WASM Memory Helpers**
   - Study Wasmer 4.2 API documentation
   - Implement `read_string_from_wasm()`
   - Implement `write_json_to_wasm()`
   - Test with simple WASM module

2. **Complete `host_spacedrive_call()`**
   - Bridge to `execute_json_operation()`
   - Add permission checking
   - Error handling

3. **Create Test WASM Module**
   - Rust project that compiles to WASM
   - Calls `spacedrive_call()` with test payload
   - Validates round-trip works

### Week 2-3

4. **Add Extension Operations**
   - Implement `ai.ocr` (Tesseract integration)
   - Implement `credentials.store/get`
   - Implement `vdfs.write_sidecar`

5. **Build Extension SDK**
   - Create `spacedrive-sdk` crate
   - Type-safe wrappers
   - Documentation

### Week 4+

6. **Finance Extension**
   - Email scanning
   - Receipt processing
   - Full end-to-end test

## Architecture Documents

- **[WASM_ARCHITECTURE_FINAL.md](../../docs/core/design/WASM_ARCHITECTURE_FINAL.md)** - Quick reference
- **[EXTENSION_IPC_DESIGN.md](../../docs/core/design/EXTENSION_IPC_DESIGN.md)** - Detailed design
- **[EMAIL_INGESTION_EXTENSION_DESIGN.md](../../docs/core/design/EMAIL_INGESTION_EXTENSION_DESIGN.md)** - Finance extension
- **[PLATFORM_REVENUE_MODEL.md](../../docs/PLATFORM_REVENUE_MODEL.md)** - Business model

## Example Usage (Future)

```rust
// In Spacedrive Core
let mut plugin_manager = PluginManager::new(core.clone(), plugins_dir);
plugin_manager.load_plugin("finance").await?;

// Extension (WASM) calls:
let result = spacedrive_call(
    "query:ai.ocr",
    library_id,
    json!({ "data": pdf_bytes, "options": { "language": "eng" } })
);
```

## Testing

```bash
# Check compilation
cd core && cargo check

# Run tests (once implemented)
cd core && cargo test extension

# Load test plugin (once implemented)
cargo run --bin spacedrive extension load ./plugins/test-plugin
```

## Notes

- **Memory Management:** WASM modules must export `wasm_alloc(size: i32) -> *mut u8`
- **Error Handling:** Errors returned as JSON `{ "error": "message" }`
- **Permissions:** Checked on every `spacedrive_call()`
- **Rate Limiting:** 1000 requests/minute default

---

_Last Updated: October 2025 - Initial integration_
