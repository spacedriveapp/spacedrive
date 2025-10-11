# Spacedrive Extension SDK

**Beautiful, type-safe API for building Spacedrive WASM extensions.**

## Installation

Add to your extension's `Cargo.toml`:

```toml
[dependencies]
spacedrive-sdk = { path = "../spacedrive-sdk" }
```

## Quick Start

```rust
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::ExtensionContext;

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    spacedrive_sdk::ffi::log_info("Extension started!");
    0 // Success
}

fn process_receipt(ctx: &ExtensionContext, pdf_data: &[u8]) -> Result<Uuid> {
    // Create entry
    let entry = ctx.vdfs().create_entry(CreateEntry {
        name: "Receipt: Starbucks".into(),
        path: "receipts/1.pdf".into(),
        entry_type: "FinancialDocument".into(),
        metadata: None,
    })?;

    // Run OCR
    let ocr_result = ctx.ai().ocr(pdf_data, OcrOptions::default())?;

    // Store result
    ctx.vdfs().write_sidecar(entry.id, "ocr.txt", ocr_result.text.as_bytes())?;

    // Classify with AI
    let receipt = ctx.ai().classify_text(
        &ocr_result.text,
        "Extract vendor, amount, date from this receipt. Return JSON."
    )?;

    ctx.vdfs().write_sidecar(entry.id, "receipt.json",
        serde_json::to_vec(&receipt)?.as_slice()
    )?;

    Ok(entry.id)
}
```

## No Unsafe, No FFI, Just Clean Rust

**Before (raw C bindings):**
```rust
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

// Then manually:
// - Serialize to JSON
// - Get pointer to string
// - Call unsafe function
// - Read result from returned pointer
// - Deserialize JSON
// - Handle errors
```

**After (with SDK):**
```rust
let entry = ctx.vdfs().create_entry(CreateEntry { ... })?;
let ocr = ctx.ai().ocr(&pdf_data, OcrOptions::default())?;
```

**That's it!** Clean, type-safe, ergonomic.

## API Reference

### VDFS Operations

```rust
// Create entries
let entry = ctx.vdfs().create_entry(CreateEntry {
    name: "My File".into(),
    path: "path/to/file".into(),
    entry_type: "Document".into(),
    metadata: None,
})?;

// Write sidecars (store metadata/analysis)
ctx.vdfs().write_sidecar(entry.id, "metadata.json", data)?;

// Read sidecars
let data = ctx.vdfs().read_sidecar(entry.id, "metadata.json")?;

// Update metadata
ctx.vdfs().update_metadata(entry.id, json!({ "category": "Important" }))?;
```

### AI Operations

```rust
// OCR
let ocr_result = ctx.ai().ocr(&pdf_bytes, OcrOptions {
    language: "eng".into(),
    engine: OcrEngine::Tesseract,
    preprocessing: true,
})?;

// Text classification
let result = ctx.ai().classify_text(
    &text,
    "Extract structured data from this receipt"
)?;

// Embeddings
let embedding = ctx.ai().embed("search query text")?;
```

### Credential Management

```rust
// Store OAuth token
ctx.credentials().store("gmail_oauth", Credential::oauth2(
    access_token,
    Some(refresh_token),
    3600, // expires_in_seconds
    vec!["https://www.googleapis.com/auth/gmail.readonly".into()]
))?;

// Get credential (auto-refreshes if OAuth)
let cred = ctx.credentials().get("gmail_oauth")?;

// Delete credential
ctx.credentials().delete("old_credential")?;
```

### Job System

```rust
// Dispatch background job
let job_id = ctx.jobs().dispatch("email_scan", json!({
    "provider": "gmail"
}))?;

// Check status
match ctx.jobs().get_status(job_id)? {
    JobStatus::Running { progress } => {
        ctx.log(&format!("Job {}% complete", progress * 100.0));
    }
    JobStatus::Completed => {
        ctx.log("Job done!");
    }
    _ => {}
}

// Cancel job
ctx.jobs().cancel(job_id)?;
```

## Building Your Extension

```bash
# Build for release (optimized for size)
cargo build --target wasm32-unknown-unknown --release

# Output: target/wasm32-unknown-unknown/release/your_extension.wasm
```

## Required Exports

Your extension must export these functions:

```rust
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    spacedrive_sdk::ffi::log_info("Extension starting!");
    0 // Return 0 for success
}

#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
    spacedrive_sdk::ffi::log_info("Extension cleanup");
    0 // Return 0 for success
}
```

The SDK automatically provides `wasm_alloc` and `wasm_free` - you don't need to implement them!

## manifest.json

```json
{
  "id": "my-extension",
  "name": "My Extension",
  "version": "0.1.0",
  "description": "What my extension does",
  "author": "Your Name",
  "wasm_file": "my_extension.wasm",
  "permissions": {
    "methods": ["vdfs.", "ai.ocr", "credentials."],
    "libraries": ["*"],
    "rate_limits": {
      "requests_per_minute": 1000
    },
    "max_memory_mb": 512
  }
}
```

## Error Handling

All operations return `Result<T, Error>`:

```rust
use spacedrive_sdk::prelude::*;

fn my_operation(ctx: &ExtensionContext) -> Result<()> {
    let entry = ctx.vdfs().create_entry(...)?;  // ? operator works!
    ctx.vdfs().write_sidecar(entry.id, "data.json", data)?;
    Ok(())
}
```

## Logging

```rust
ctx.log("Info message");
ctx.log_error("Error message");

// Or directly:
spacedrive_sdk::ffi::log_info("Message");
spacedrive_sdk::ffi::log_debug("Debug");
spacedrive_sdk::ffi::log_warn("Warning");
spacedrive_sdk::ffi::log_error("Error");
```

## Examples

See `extensions/test-extension/` for a complete working example.

---

**Beautiful API. Zero unsafe code. Just Rust. **

