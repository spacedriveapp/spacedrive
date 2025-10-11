# Spacedrive Official Extensions

This directory contains the extension SDK and official extensions for Spacedrive.

## Structure

```
extensions/
├── spacedrive-sdk/          # Core SDK library
├── spacedrive-sdk-macros/   # Proc macros for beautiful API
├── test-extension/          # Example extension with beautiful API
└── finance/                 # (Future) First revenue-generating extension
```

## Quick Start

### 1. Install WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

### 2. Create Extension

```bash
cargo new --lib my-extension
cd my-extension
```

**Cargo.toml:**
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
spacedrive-sdk = { path = "../spacedrive-sdk" }
serde = { version = "1.0", features = ["derive"] }
```

**src/lib.rs:**
```rust
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{extension, job};

#[extension(
    id = "my-extension",
    name = "My Extension",
    version = "0.1.0"
)]
struct MyExtension;

#[derive(Serialize, Deserialize, Default)]
pub struct MyJobState {
    pub counter: u32,
}

#[job]
fn my_job(ctx: &JobContext, state: &mut MyJobState) -> Result<()> {
    ctx.log("Job starting!");

    state.counter += 1;
    ctx.report_progress(1.0, "Done!");

    Ok(())
}
```

### 3. Build

```bash
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/my_extension.wasm .
```

### 4. Create manifest.json

```json
{
  "id": "my-extension",
  "name": "My Extension",
  "version": "0.1.0",
  "wasm_file": "my_extension.wasm",
  "permissions": {
    "methods": ["vdfs.*", "ai.*"],
    "libraries": ["*"]
  }
}
```

## The Beautiful API

### Before Macros (Manual FFI):
```rust
#[no_mangle]
pub extern "C" fn execute_my_job(
    ctx_ptr: u32, ctx_len: u32,
    state_ptr: u32, state_len: u32
) -> i32 {
    let ctx_json = unsafe { /* 30 lines of pointer manipulation */ };
    let mut state = /* 40 lines of deserialization */;
    // ... business logic buried in boilerplate ...
}
```
**180+ lines, lots of unsafe**

### After Macros (Beautiful):
```rust
#[job]
fn my_job(ctx: &JobContext, state: &mut MyJobState) -> Result<()> {
    // Just write business logic!
    ctx.log("Working...");
    state.counter += 1;
    Ok(())
}
```
**60-80 lines, zero unsafe, pure logic**

## API Reference

### Extension Container

```rust
#[extension(
    id = "finance",
    name = "Spacedrive Finance",
    version = "0.1.0"
)]
struct Finance;
```

Generates:
- `plugin_init()` export
- `plugin_cleanup()` export
- Metadata for manifest generation

### Job Definition

```rust
#[job]
fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
    // Progress reporting
    ctx.report_progress(0.5, "Half done");

    // Checkpointing
    ctx.checkpoint(state)?;

    // Interruption handling
    if ctx.check_interrupt() {
        return Err(Error::OperationFailed("Interrupted".into()));
    }

    // Metrics
    ctx.increment_items(1);
    ctx.increment_bytes(1000);

    // Warnings
    ctx.add_warning("Non-fatal issue");

    // Full SDK access
    let entry = ctx.vdfs().create_entry(...)?;
    let ocr = ctx.ai().ocr(&pdf, ...)?;

    Ok(())
}
```

### VDFS Operations

```rust
// Create entries
let entry = ctx.vdfs().create_entry(CreateEntry {
    name: "My File".into(),
    path: "path/to/file".into(),
    entry_type: "Document".into(),
    metadata: None,
})?;

// Write sidecars
ctx.vdfs().write_sidecar(entry.id, "metadata.json", data)?;

// Read sidecars
let data = ctx.vdfs().read_sidecar(entry.id, "metadata.json")?;
```

### AI Operations

```rust
// OCR
let ocr = ctx.ai().ocr(&pdf_bytes, OcrOptions::default())?;

// Classification
let result = ctx.ai().classify_text(&text, "Extract data")?;

// Embeddings
let embedding = ctx.ai().embed("query text")?;
```

### Credentials

```rust
// Store OAuth
ctx.credentials().store("gmail", Credential::oauth2(
    access_token,
    Some(refresh_token),
    3600,
    vec!["https://www.googleapis.com/auth/gmail.readonly".into()]
))?;

// Get (auto-refreshes)
let cred = ctx.credentials().get("gmail")?;
```

## Examples

See `extensions/test-extension/` for a complete working example.

## Building

All extensions:
```bash
cd extensions/test-extension
cargo build --target wasm32-unknown-unknown --release
```

## Documentation

- **[SDK API Vision](../docs/EXTENSION_SDK_API_VISION.md)** - Future API improvements
- **[Before/After Comparison](./BEFORE_AFTER_COMPARISON.md)** - See the transformation
- **[WASM Architecture](../docs/core/design/WASM_ARCHITECTURE_FINAL.md)** - Technical details
- **[Platform Revenue Model](../docs/PLATFORM_REVENUE_MODEL.md)** - Business case

---

**Extension development is now beautiful, safe, and productive. Start building!** 
