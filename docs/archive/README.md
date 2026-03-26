# Archive System

Archive is Spacedrive's data archival system for indexing external data sources beyond the filesystem. While the VDFS manages files, Archive handles everything else: emails, notes, messages, bookmarks, calendar events, contacts, and more.

## Features

- **Universal Indexing** - Adapters ingest data from Gmail, Slack, Obsidian, Chrome, Safari, GitHub, Apple Notes, Calendar, Contacts, and more via a script-based protocol
- **Hybrid Search** - Combines full-text search (SQLite FTS5) with semantic vector search (LanceDB + FastEmbed) merged via Reciprocal Rank Fusion
- **Safety Screening** - Prompt Guard 2 classifies indexed text for injection attacks before it enters the search index
- **Schema-Driven Sources** - Each data source is self-contained with its own SQLite database, vector index, and TOML schema
- **AI-Ready** - Spacebot queries archived data through structured search APIs with built-in safety metadata
- **P2P Sync** - Source metadata syncs across devices via library sync

## Quick Start

### 1. Create a Source

```typescript
// Create a Gmail source
const source = await core.sources.create({
  name: "Work Gmail",
  adapter_id: "gmail",
  trust_tier: "external",
  config: {
    email: "work@example.com",
    // OAuth flow happens automatically
  }
});
```

### 2. Sync Data

```typescript
// Trigger sync job
const jobId = await core.sources.sync({
  source_id: source.id
});

// Monitor progress
core.jobs.subscribe(jobId, (progress) => {
  console.log(`Synced ${progress.current}/${progress.total} items`);
});
```

### 3. Search

```typescript
// Hybrid search across all sources
const results = await core.sources.search({
  query: "budget proposal Q4",
  source_ids: [source.id],
  limit: 20
});

// Results include both FTS and vector matches
results.forEach(result => {
  console.log(`${result.title} (score: ${result.score})`);
  console.log(`Trust: ${result.trust_tier}, Safe: ${result.safety_verdict}`);
});
```

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────┐
│                    Spacedrive Library                    │
├─────────────────────────────────────────────────────────┤
│  VDFS (Files)              Archive (Everything Else)    │
│  ├─ Locations              ├─ Sources                   │
│  ├─ Entries                │  ├─ Gmail                  │
│  ├─ Content IDs            │  ├─ Slack                  │
│  └─ Sidecars               │  ├─ Obsidian               │
│                            │  └─ Chrome History         │
│                            │                             │
│                            ├─ Hybrid Search              │
│                            │  ├─ FTS5 (keywords)         │
│                            │  └─ LanceDB (semantic)      │
│                            │                             │
│                            └─ Safety Pipeline            │
│                               ├─ Prompt Guard 2          │
│                               ├─ Trust Tiers             │
│                               └─ Quarantine              │
└─────────────────────────────────────────────────────────┘
```

### Storage Layout

Each library contains a `sources/` directory alongside the VDFS:

```
.sdlibrary/
├─ library.db              # VDFS + source metadata
├─ sidecars/               # VDFS sidecars
└─ sources/                # Archive sources
   ├─ registry.db          # Optional separate registry
   └─ {source-uuid}/
      ├─ data.db           # Generated from TOML schema
      ├─ embeddings.lance/ # Vector index
      ├─ schema.toml       # Data type definition
      ├─ state/            # Adapter cursor state
      └─ cache/            # Adapter-specific caches
```

## Adapters

Adapters are script-based data source connectors that communicate via stdin/stdout JSONL protocol.

### Built-in Adapters

- **Gmail** - Emails, threads, labels
- **Obsidian** - Notes, links, tags
- **Slack** - Messages, threads, channels
- **Chrome Bookmarks** - Bookmarks, folders
- **Chrome History** - Browsing history
- **Safari History** - Browsing history
- **Apple Notes** - Notes, attachments
- **Apple Calendar** - Events, reminders
- **Apple Contacts** - Contacts, groups
- **GitHub** - Issues, PRs, commits
- **OpenCode** - Code snippets, projects

### Creating an Adapter

**1. Create adapter manifest (`adapters/my-adapter/adapter.toml`):**

```toml
[adapter]
id = "my-adapter"
name = "My Adapter"
version = "1.0.0"
trust_tier = "external"

[sync]
command = "python3"
args = ["sync.py"]

[schema]
inline = """
[type]
name = "MyRecord"
fields = [
  { name = "title", type = "String", indexed = true },
  { name = "content", type = "Text", indexed = true, embedded = true },
  { name = "created_at", type = "DateTime" }
]
"""
```

**2. Create sync script (`adapters/my-adapter/sync.py`):**

```python
#!/usr/bin/env python3
import json
import sys

def sync():
    # Read config from stdin
    config = json.loads(sys.stdin.readline())

    # Fetch data from source
    records = fetch_from_api(config)

    # Emit records as JSONL
    for record in records:
        print(json.dumps({
            "op": "upsert",
            "id": record["id"],
            "data": {
                "title": record["title"],
                "content": record["content"],
                "created_at": record["timestamp"]
            }
        }))
        sys.stdout.flush()

if __name__ == "__main__":
    sync()
```

**3. Install adapter:**

```bash
# Adapters are auto-discovered from adapters/ directory
# Just place your adapter folder in adapters/ and restart
```

## Operations

### Sources

```typescript
// Create
core.sources.create(input: CreateSourceInput): SourceInfo

// List
core.sources.list(): SourceInfo[]

// Get
core.sources.get(id: Uuid): SourceInfo

// Update
core.sources.update(id: Uuid, updates: SourceUpdates): SourceInfo

// Delete
core.sources.delete(id: Uuid): void

// Sync
core.sources.sync(id: Uuid): JobId

// Sync all
core.sources.sync_all(): JobId[]

// Search
core.sources.search(query: SearchInput): SearchResult[]
```

### Records

```typescript
// List records in a source
core.sources.records.list(source_id: Uuid, limit?: number): Record[]

// Get specific record
core.sources.records.get(source_id: Uuid, record_id: string): Record

// Delete record
core.sources.delete_record(source_id: Uuid, record_id: string): void
```

### Quarantine

```typescript
// List quarantined records
core.sources.quarantine.list(source_id: Uuid): QuarantinedRecord[]

// Release from quarantine
core.sources.release_quarantined(source_id: Uuid, record_id: string): void
```

### Adapters

```typescript
// List available adapters
core.sources.adapters.list(): AdapterInfo[]

// Get adapter details
core.sources.adapters.get(id: string): AdapterInfo

// List schemas
core.sources.schemas.list(): SchemaInfo[]
```

## Safety & Trust

### Trust Tiers

Sources are assigned trust tiers that determine screening strictness:

- **authored** - Content you created (Obsidian notes, drafts)
- **collaborative** - Shared workspaces (Slack channels, shared docs)
- **external** - Public or untrusted sources (Gmail, GitHub issues)

### Safety Pipeline

```
Adapter Sync
    ↓
Screening (Prompt Guard 2)
    ├─ Safe → Continue
    └─ Flagged → Quarantine
         ↓
Classification (optional)
    ↓
Embedding (FastEmbed)
    ↓
Searchable
```

### Quarantine

Flagged records are:
- Excluded from search results by default
- Visible in quarantine UI for review
- Can be manually released or deleted
- Never exposed to AI agents

## Development

### Running Tests

```bash
# Test the archive crate
cargo test -p sd-archive

# Test core integration
cargo test -p spacedrive-core -- sources::

# Test specific adapter
python3 adapters/gmail/test.py
```

### Adding a Job

Jobs live in `core/src/ops/sources/` alongside their operations:

```rust
// core/src/ops/sources/my_job.rs
use crate::infra::job::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct MyJob {
    pub source_id: Uuid,
}

impl Job for MyJob {
    const NAME: &'static str = "my_job";
    const RESUMABLE: bool = true;
}

#[async_trait]
impl JobHandler for MyJob {
    type Output = MyJobOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Get source manager
        let mgr = ctx.library.source_manager()
            .ok_or_else(|| JobError::Internal("Source manager not initialized".into()))?;

        // Do work with progress reporting
        ctx.report_progress(MyProgress { current: 10, total: 100 }).await?;

        // Return output
        Ok(MyJobOutput { ... })
    }
}
```

### Debugging

Enable verbose logging:

```bash
RUST_LOG=sd_archive=debug,spacedrive_core::data=debug cargo run
```

View source database:

```bash
sqlite3 ~/.sdlibrary/MyLibrary/sources/{source-uuid}/data.db
.schema
SELECT * FROM records LIMIT 10;
```

Inspect vector index:

```python
import lancedb
db = lancedb.connect("~/.sdlibrary/MyLibrary/sources/{source-uuid}/embeddings.lance")
table = db.open_table("embeddings")
print(table.schema)
```

## FAQ

**Q: How is this different from the VDFS?**

A: VDFS manages files on disk with content identity and cross-device awareness. Archive manages structured data from external sources (emails, notes, etc.) that aren't files.

**Q: Do adapters run in a sandbox?**

A: Adapters run as subprocess with limited privileges. They receive config via stdin and emit records via stdout. No filesystem or network access unless explicitly granted.

**Q: Can I sync the same source to multiple devices?**

A: Yes. Source metadata syncs via library sync. Each device can independently sync data from the source, or you can configure one device to sync and distribute snapshots.

**Q: What happens if an adapter crashes?**

A: The sync job tracks progress via cursor state. Resume from the last successful checkpoint. Partial syncs don't corrupt the database.

**Q: Can I search across both files and sources?**

A: Not yet. Currently file search and source search are separate. Unified federated search is planned for a future release.

**Q: How do I handle OAuth secrets?**

A: Secrets are stored encrypted in Spacedrive's KeyManager (OS keychain + redb). Adapters receive decrypted secrets as environment variables during sync.

**Q: What's the performance impact?**

A: Archive runs as background jobs. Embeddings are generated incrementally. Search is fast (FTS5 + LanceDB are both optimized for low-latency queries). Typical overhead: <5% CPU during sync, <100MB RAM per source.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for general guidelines.

For adapter contributions, see [ADAPTERS.md](../ADAPTERS.md).

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
