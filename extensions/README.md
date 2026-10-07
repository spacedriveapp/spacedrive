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
    "read_records": { "glob": "**/*.{jpg,png}" },
    "read_sidecars": ["exif"],
    "write_sidecars": ["digest"],
    "use_models": [{ "category": "face_detection", "preference": "local" }]
  }
}
```

The host rejects a manifest with a field it does not know, so a typo in a
grant fails at load. `read_records` is the only way to see files; its optional
`glob` is matched against the record's path. `write_sidecars` names the kinds
the extension may write; `use_models` the inference categories it may ask for.
`write_tags`, `write_custom_fields` and `dispatch_jobs` are declared but have
no host side yet.

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
    let record = ctx.vdfs().get_record(uuid).await?;
    let bytes = record.read().await?;

    Ok(())
}
```

### VDFS Operations

```rust
// Records of the library's sources, filtered by extension (read_records grant)
let photos = ctx.vdfs()
    .query_records()
    .with_extensions(["jpg", "png"])
    .collect()
    .await?;

// One record and its bytes
let record = ctx.vdfs().get_record(uuid).await?;
let bytes = record.read().await?;

// Sidecars are one JSON document per kind, keyed by content uuid
let content_uuid = record.content_uuid().expect("hashed");
if !ctx.sidecar_exists(content_uuid, "digest")? {
    ctx.save_sidecar(content_uuid, "digest", &digest).await?;
}
let digest: Digest = ctx.read_sidecar(content_uuid, "digest").await?;

// Models declared with #[model] live in the extension's own store
ctx.vdfs().create_model(entry).await?;
let all: Vec<CatalogEntry> = ctx.vdfs().query_models().collect().await?;
```

Tags and custom fields have no host side yet and return `Error::NotAvailable`.

### AI Operations

Inference goes through `ctx.ai().infer(...)` behind the `use_models` grant.
The core has no provider yet, so every request answers `Error::NotAvailable`;
handle it the way `extensions/photos/src/jobs/analyze.rs` does, with one
warning and a skip, so the job completes and a later run with a provider
picks the files up.

```rust
// Written against the API; answers NotAvailable until a provider exists
let faces = ctx.ai().from_registered("face_detection").detect_faces(&bytes).await?;
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
