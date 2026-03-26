# sd-archive

Archive engine for Spacedrive - indexes external data sources beyond the filesystem.

## Overview

This crate provides the core archival engine that powers Spacedrive's data source integration. It is designed to be used as a standalone library or integrated into Spacedrive's core.

**Key features:**

- Schema-driven SQLite databases generated from TOML schemas
- Script-based adapter runtime (stdin/stdout JSONL protocol)
- Hybrid search (FTS5 + LanceDB vector search + RRF merging)
- Safety screening (Prompt Guard 2 for injection detection)
- Portable sources (copy folder, it works)

## Usage

### Standalone

```rust
use sd_archive::{Engine, EngineConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize engine
    let config = EngineConfig {
        data_dir: PathBuf::from("./data"),
    };
    let engine = Engine::new(config).await?;

    // Create source from adapter
    let source_id = engine.create_source(
        "my-gmail",
        "gmail",
        serde_json::json!({
            "email": "user@example.com"
        })
    ).await?;

    // Sync data
    let report = engine.sync_source(&source_id, |progress| {
        println!("Progress: {}/{}", progress.current, progress.total);
    }).await?;

    // Search
    let results = engine.search(&source_id, "budget proposal", 10).await?;
    for result in results {
        println!("{}: {} (score: {})", result.id, result.title, result.score);
    }

    Ok(())
}
```

### Integrated with Spacedrive

See `core/src/data/` for the Spacedrive integration wrapper that adds:
- Library-scoped lifecycle
- Job system integration
- Event bus integration
- KeyManager for secrets
- Operation/query registration

## Architecture

### Components

- **Engine** - Top-level coordinator
- **Schema** - TOML parser, SQL codegen, migrations
- **SourceDb** - SQLite database per source
- **Registry** - Source metadata management
- **Adapter** - Script subprocess runtime
- **Search** - Hybrid search router (FTS + vector)
- **Safety** - Prompt Guard 2 screening
- **Embedding** - FastEmbed vector generation

### Data Flow

```
Adapter (script)
    ↓ JSONL
ScriptAdapter
    ↓ Records
SourceDb (upsert/delete)
    ↓
Safety Screening
    ↓
Embedding Generation
    ↓
Search Index (FTS5 + LanceDB)
```

## Features

### Default Features

None. The crate compiles with minimal dependencies by default.

### Optional Features

- **`safety-screening`** - Enable Prompt Guard 2 safety classifier
  - Adds: `ort`, `tokenizers`, `hf-hub`
  - Enables: `safety::PromptGuard` module
  - Use when: Building with AI safety features

## Schema Format

Sources are defined by TOML schemas:

```toml
[type]
name = "Email"
fields = [
  { name = "subject", type = "String", indexed = true },
  { name = "body", type = "Text", indexed = true, embedded = true },
  { name = "from", type = "String" },
  { name = "to", type = "String" },
  { name = "received_at", type = "DateTime" },
]

[type]
name = "Attachment"
fields = [
  { name = "filename", type = "String" },
  { name = "size", type = "Integer" },
  { name = "email_id", type = "ForeignKey", references = "Email" }
]
```

**Field types:**
- `String` - Short text (up to 1KB)
- `Text` - Long text (unlimited)
- `Integer` - i64
- `Float` - f64
- `Boolean` - bool
- `DateTime` - ISO 8601 timestamp
- `Json` - Arbitrary JSON
- `ForeignKey` - Reference to another type

**Field flags:**
- `indexed: true` - Create FTS5 index for full-text search
- `embedded: true` - Generate vector embeddings for semantic search
- `unique: true` - Enforce uniqueness constraint
- `nullable: false` - Require non-null values

## Adapter Protocol

Adapters communicate via stdin/stdout using line-delimited JSON.

### Input (stdin)

Config object sent once at startup:

```json
{"email": "user@example.com", "cursor": "abc123"}
```

### Output (stdout)

Stream of operation objects:

```json
{"op": "upsert", "id": "msg-1", "data": {"subject": "Hello", "body": "..."}}
{"op": "upsert", "id": "msg-2", "data": {"subject": "Re: Hello", "body": "..."}}
{"op": "delete", "id": "msg-3"}
```

**Operations:**
- `upsert` - Insert or update record
- `delete` - Delete record
- `link` - Create relationship between records

### Cursor State

Adapters maintain cursor state for incremental sync:

```json
{"op": "cursor", "value": "next-page-token-xyz"}
```

The engine persists cursor state and provides it on next sync.

## Dependencies

**Core:**
- `sqlx` - SQLite database operations
- `toml` - Schema parsing
- `serde` / `serde_json` - Serialization
- `tokio` - Async runtime
- `uuid` - Source IDs
- `blake3` - Content hashing

**Search:**
- `lancedb` - Vector database
- `fastembed` - Embedding model

**Safety (optional):**
- `ort` - ONNX Runtime for Prompt Guard 2
- `tokenizers` - Text tokenization
- `hf-hub` - Model downloads

## Performance

**Benchmarks** (M2 Max, 10k emails):

- Schema parsing: ~1ms
- Schema migration: ~50ms (first time), ~5ms (no-op)
- Adapter sync: ~2000 records/sec (I/O bound)
- FTS5 search: ~5ms (p95)
- Vector search: ~20ms (p95)
- Hybrid search (RRF): ~30ms (p95)
- Embedding generation: ~100 records/sec (CPU bound)

**Memory:**
- Engine overhead: ~10MB
- Per-source overhead: ~5MB
- LanceDB cache: ~50MB
- FastEmbed model: ~100MB (shared across sources)

## Testing

```bash
# Run all tests
cargo test -p sd-archive

# Run with safety features
cargo test -p sd-archive --features safety-screening

# Run specific test
cargo test -p sd-archive schema::tests::parse_simple_schema

# Benchmark
cargo bench -p sd-archive
```

## License

FSL-1.1-ALv2 - See [../../LICENSE](../../LICENSE) for details.
