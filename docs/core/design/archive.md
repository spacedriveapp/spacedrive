# Archive System Integration Design

## What is Archive?

Archive is Spacedrive's data archival system that indexes external data sources beyond the filesystem. While the VDFS manages files, Archive handles everything else: emails, notes, messages, bookmarks, calendar events, contacts, and more.

**Core capabilities:**

- **Universal indexing** - Adapters ingest data from Gmail, Obsidian, Slack, Chrome, Safari, GitHub, Apple Notes, Calendar, Contacts, and other sources via a script-based protocol (stdin/stdout JSONL).

- **Hybrid search** - Every archive source combines full-text search (SQLite FTS5) with semantic vector search (LanceDB + FastEmbed) merged via Reciprocal Rank Fusion. Find content by keywords or meaning.

- **Safety screening** - Prompt Guard 2 classifies all indexed text for injection attacks before it enters the search index. Content is trust-tiered (authored, collaborative, external) and screened accordingly. Quarantined records never reach AI agents.

- **Schema-driven sources** - Each data source is a self-contained source with its own SQLite database, vector index, and TOML schema. Schemas auto-generate tables, foreign keys, and search indexes. Sources are portable (copy the folder, it works).

- **AI-ready** - Spacebot queries archived data through structured search APIs with built-in safety metadata. No raw file access, no prompt injection risk.

**Use cases:**

- Search across all your Gmail, Notes, and Slack from one interface
- Give AI agents access to your knowledge base without uploading to cloud services
- Index and preserve data from services before they shut down or change APIs
- Cross-device sync of archived metadata (not files, but emails, notes, bookmarks)
- Build custom adapters for any data source (if it reads stdin and prints JSONL, it works)

**Relationship to VDFS:**

Archive sits alongside the VDFS, not inside it. Files live in locations managed by the VDFS. Archived data lives in sources managed by the Archive system. Both are library-scoped resources. Search can span both (future unified search) or query them separately.

---

## Purpose

Fold the `./spacedrive-archive-prototype` prototype into the official `spacedrive` codebase as a new library-scoped archive system without forcing convergence between the VDFS index and the archival engine.

This keeps Spacedrive's file-native architecture intact while adding a second data plane for extracted, adapter-driven sources such as Gmail, Obsidian, Chrome History, Slack, and GitHub.

## Decision

Spacedrive will support two storage systems inside a library:

1. The VDFS library database, which remains the source of truth for files, entries, content identities, tags, spaces, sidecars, sync metadata, and file operations.
2. A source engine, which manages archived external data sources as isolated sources with their own SQLite database, vector index, schema, cursor state, and processing pipeline.

These systems are linked at the library boundary, not merged into one database.

## Why This Shape

- The VDFS already solves file indexing, sync, sidecars, jobs, and device-aware lifecycle well.
- The prototype already solves schema-driven sources, adapter ingestion, hybrid search, and isolated archival storage well.
- Forcing source records into the main library database would create a large migration with little product value.
- Keeping sources isolated preserves portability, adapter flexibility, schema evolution, and per-source lifecycle control.
- Registering sources at the library level gives us one user-facing primitive for ownership, sync, permissions, and UI.

## Goals

- Add archival sources to official Spacedrive without rewriting the VDFS.
- Reuse the existing daemon, RPC, type generation, ops, job system, and UI infrastructure.
- Keep each source self-contained on disk.
- Make sources library-scoped so they can participate in library sync and lifecycle.
- Translate the prototype's pipeline into Spacedrive's ops and jobs model.

## Non-Goals

- Do not merge source records into the VDFS entry index.
- Do not split a library into multiple primary databases yet.
- Do not ship a separate OpenAPI server for this feature.
- Do not force the source engine to use SeaORM for dynamic schema tables.
- Do not redesign the whole search stack to unify files and sources in the first slice.

## User Model

A library owns:

- its existing VDFS database and sidecars
- zero or more archival sources

A source is a managed library resource. It is visible in the library UI, syncable across devices, and controlled by library operations. Its payload stays in a separate source folder.

Examples:

- Library: `Personal`
  - VDFS index for files and locations
  - Source: `Work Gmail`
  - Source: `Obsidian Vault`
  - Source: `Chrome History`

## Storage Layout

Each library gets a sources root inside `.sdlibrary`.

```text
.sdlibrary/
  library.db
  sidecars/
  sources/
    registry.db
    <source_id>/
      data.db
      embeddings.lance/
      schema.toml
      state/
      cache/
```

### Library Database Responsibilities

The library database stores source metadata only:

- source id
- library id
- display name
- adapter id
- source path
- trust tier
- visibility
- status
- last sync timestamps
- sync policy
- pipeline policy
- device sync metadata

### Source Folder Responsibilities

Each source folder stores source-specific payload:

- generated SQLite tables from schema
- FTS tables and triggers
- vector index
- adapter cursor state
- adapter-specific caches
- local processing artifacts that are not sidecars

## Integration Boundary

The integration point is `Library`, not `CoreContext` globally and not the VDFS entry graph.

The library becomes the owner of a `SourceManager` and a `SourceRegistry` alongside its existing services.

```text
Core
  -> LibraryManager
    -> Library
      -> VDFS database and services
      -> Source subsystem
           -> source registry
           -> source manager
           -> adapter runtime
           -> source search
           -> source pipeline jobs
```

This means source operations should mostly be library actions and library queries.

## Proposed Code Shape

Build as a standalone crate in `/crates/archive/` (package: `sd-archive`) for better caching and reusability.

**Crate structure:**

```text
crates/archive/
  Cargo.toml           # Package name: sd-archive; Heavy deps: lancedb, fastembed, ort (optional)
  src/
    lib.rs             # Public API exports
    engine.rs          # Core engine (no job system)
    adapter/
      mod.rs
      script.rs
    schema/
      mod.rs
      parser.rs
      codegen.rs
      migration.rs
    db/
      mod.rs
      source_db.rs
    search/
      mod.rs
      router.rs
      fts.rs
      vector.rs
    safety.rs
    embedding.rs
    error.rs
```

**Core integration wrapper:**

```text
core/src/data/
  mod.rs               # Re-exports from sd-archive
  manager.rs           # Library-scoped wrapper
  integration.rs       # Bridges engine with KeyManager, EventBus
```

**Operations and jobs:**

```text
core/src/ops/sources/
  mod.rs
  create.rs            # CreateSourceAction
  list.rs              # ListSourcesQuery
  sync.rs              # SyncSourceAction + SourceSyncJob
  search.rs            # SearchSourcesQuery
  delete.rs            # DeleteSourceAction
```

The crate keeps `sqlx` and raw SQL internally (justified for dynamic schemas). The core wrapper integrates with Spacedrive conventions: ops, jobs, events, and typed outputs.

**Benefits:**

- Heavy dependencies (LanceDB, FastEmbed) cached separately in CI
- Pure engine reusable by other projects (Spacebot, CLI tools)
- Core integration uses v2's job system for orchestration
- Clean separation: engine logic vs job orchestration

## Why Keep `sqlx` Internals

The source engine generates tables dynamically from TOML schemas. That fits raw SQL much better than SeaORM entities.

The integration rule should be:

- use existing Spacedrive infrastructure for lifecycle, dispatch, jobs, events, sync, and UI
- keep raw SQL and schema codegen inside the source engine where it reduces friction

This avoids rewriting working prototype internals just to satisfy the ORM used by the VDFS.

## Library Registration Model

Sources should be first-class library resources.

We add a new library-scoped domain concept, likely `library_source`.

Suggested fields:

```text
id
library_id
name
adapter_id
source_root
trust_tier
visibility
status
last_synced_at
last_screened_at
last_embedded_at
sync_cursor
search_enabled
agent_enabled
created_at
updated_at
```

The exact cursor should live inside source storage if it is adapter-owned. The library record only needs summary metadata for UI and sync orchestration.

## Operations Mapping

The prototype's API should be translated into V2 ops.

### Library Actions

- `sources.create`
- `sources.update`
- `sources.delete`
- `sources.sync`
- `sources.sync_all`
- `sources.set_visibility`
- `sources.set_policy`
- `sources.release_quarantined`
- `sources.delete_record`
- `sources.adapters.install` if adapter installation remains user-facing

### Library Queries

- `sources.list`
- `sources.get`
- `sources.search`
- `sources.records.list`
- `sources.records.get`
- `sources.adapters.list`
- `sources.schemas.list`
- `sources.quarantine.list`
- `sources.status`

These should register through the existing macros in `core/src/ops/registry.rs` and flow through the current daemon transport.

## Job System Mapping

The prototype's pipeline should become library jobs.

### Core Jobs

Jobs are defined alongside their operations in `core/src/ops/sources/`:

- `SourceSyncJob` (in sync.rs)
- `SourceScreeningJob` (in screening.rs)
- `SourceEmbeddingJob` (in embedding.rs)
- `SourceClassificationJob` (in classification.rs)
- `SourceReindexJob` (in reindex.rs)
- `SourceDeleteJob` (in delete.rs) for heavy cleanup

### Sync Flow

First slice:

```text
sources.sync action
  -> enqueue SourceSyncJob
     -> adapter subprocess emits JSONL
     -> source DB upsert/delete/link
     -> enqueue or run screening stage
     -> enqueue or run embedding stage
     -> update library source status
     -> emit events
```

The job system gives us resumability, progress reporting, cancellation, and a natural home for future classification work.

## Search Model

Initial scope:

- source search is separate from file search
- the UI can offer a dedicated source search surface inside a library
- unified cross-surface search is deferred

This avoids destabilizing `core/src/ops/search/` in the first integration slice.

### Future

Later we can add a federated search query that fans out to:

- VDFS file search
- source search

and merges results at the query layer without forcing a shared storage model.

## Sync Between Devices

Sources are library-scoped resources, so library sync should distribute source metadata and availability.

Recommended phases:

### Phase 1

- sync source metadata through the library
- do not sync source payload automatically
- a second device sees that the source exists and can pull or restore it later

### Phase 2

- sync source payloads as managed library resources
- transport source bundles or deltas through the existing file sync machinery where practical
- preserve source isolation on disk

This lets us ship the feature before solving full multi-device replication.

## Adapters

Keep the script adapter model from the prototype.

Adapter shape:

- `adapter.toml`
- schema block or schema reference
- sync command
- config fields
- trust tier
- optional OAuth definition

Why keep it:

- it is simple
- it broadens the adapter ecosystem
- it does not fight the main Rust architecture
- it keeps external-source support decoupled from the VDFS

Adapters should be installed and discovered through the library feature, not as a separate server product.

## Secrets and OAuth

Do not port the prototype's secrets store. V2 already has a compatible `KeyManager` in `core/src/crypto/key_manager.rs`.

**V2 KeyManager:**

- redb database (`secrets.redb`)
- XChaCha20Poly1305 encryption
- Device key in OS keychain (with file fallback)
- Methods: `set_secret()`, `get_secret()`, `delete_secret()`

**Prototype SecretsStore:**

- redb database
- AES-256-GCM + Argon2id KDF
- Master key in OS keychain
- Adapter-scoped secrets with categories

Both use the same underlying tech. The prototype's adapter categorization can be achieved in V2 by using namespaced keys like `adapter:gmail:oauth_token`.

**Integration:**

- Use V2's existing `KeyManager` for source adapter secrets
- Source config stores secret key references, not raw values
- OAuth tokens are stored via `KeyManager::set_secret()` and retrieved during adapter sync
- Adapter runtime injects decrypted secrets into adapter subprocess environment

**This saves ~870 lines of code** that don't need to be ported.

## Safety, Classification, and Trust

These concepts map well to Spacedrive.

- trust tier belongs to the source metadata
- safety verdict and quality metadata belong to source records
- quarantine is a source view, not a VDFS concept
- classification and embedding stages should use library jobs

We should keep the prototype's pipeline ordering:

```text
adapter ingest
  -> screening
  -> classification
  -> embedding
  -> searchable
```

The first slice can ship with:

- screening
- embedding
- trust tiers
- quarantine visibility

Classification can follow as a later job-backed phase.

## UI Integration

Do not mirror the prototype desktop app structure directly.

Instead, add source features into the existing interface:

- library section for sources
- add-source flow
- source detail page
- source search page
- quarantine queue
- adapter settings and installed adapters

This keeps one product and one navigation model.

## Migration Strategy

### Phase 0: Design and Carve-Out

- add this design doc
- choose final module path under `core/src/`
- decide which prototype modules copy over largely intact
- define library metadata schema for source registration

### Phase 1: Engine Import

- port schema parser, codegen, source DB, search router, adapter runtime
- remove OpenAPI and standalone server assumptions
- wrap the imported subsystem in a library-scoped manager

### Phase 2: Library Registration

- add library models and migrations for source metadata
- create source folders under `.sdlibrary/sources/`
- expose create/list/get/delete ops

### Phase 3: Sync and Pipeline Jobs

- add sync action and `SourceSyncJob`
- port screening and embedding stages
- wire progress and status events

### Phase 4: UI Slice

- add source list, create flow, detail page, and search page
- expose quarantine state

### Phase 5: Device Sync

- propagate source metadata through library sync
- later add payload transfer strategy

### Phase 6: Unified Search

- optional federated search query across VDFS and sources

## Expected Reuse from the Prototype

Likely to port mostly intact:

- schema parser/codegen/migration
- source manager and source DB
- script adapter runtime
- vector store integration
- search router and result model
- safety model and trust policy model

Needs translation into V2 concepts:

- top-level `Engine`
- standalone CLI surface
- standalone Tauri/OpenAPI server surface
- app-owned secrets and settings flows
- desktop route structure

## Risks

### Dynamic Schema vs Existing ORM

Dynamic source tables are a poor fit for SeaORM. Forcing convergence here would slow the project down.

### Search Creep

Trying to unify file search and source search in the first pass will expand scope fast.

### Sync Scope

Full source payload sync is valuable, but not required to land the first product slice.

### Over-Refactoring

The goal is not to perfect the architecture first. The goal is to land the source engine in the official product with clean boundaries.

## Open Questions

1. Should source metadata live in `library.db` directly or in a small library-owned `sources/registry.db`?
2. Should source secrets attach to the global key manager immediately or stay engine-local for the first slice?
3. Should source search results reuse the existing search result envelope or define a source-specific output first?
4. How do we want source payload sync to package data, file sync of source folders, bundle export/import, or delta protocol?

## Recommendation

Start with the smallest honest integration:

- keep sources separate from the VDFS index
- make them library-scoped resources
- port the prototype internals with minimal rewrites
- expose everything through ops and jobs
- ship source-local search before unified search

This gets the archival product into official Spacedrive quickly without abandoning the VDFS or reopening the whole architecture.

---

## APPENDIX A: V2 Architecture Integration Patterns

### Library Structure Pattern

Libraries in v2 follow this ownership structure:

```rust
pub struct Library {
    path: PathBuf,                          // Root .sdlibrary folder
    config: Arc<RwLock<LibraryConfig>>,
    core_context: Arc<CoreContext>,
    db: Arc<Database>,                      // SeaORM
    jobs: Arc<JobManager>,
    event_bus: Arc<EventBus>,
    sync_events: Arc<SyncEventBus>,
    transaction_manager: Arc<TransactionManager>,
    sync_service: OnceCell<Arc<SyncService>>,
    file_sync_service: OnceCell<Arc<FileSyncService>>,
    // ADD:
    source_manager: OnceCell<Arc<SourceManager>>,
}
```

SourceManager initialization follows the sync_service pattern (lazy init via OnceCell):

```rust
pub fn init_source_manager(self: &Arc<Self>) -> Result<()> {
    if self.source_manager.get().is_some() {
        return Ok(());
    }
    let mgr = SourceManager::new(self.clone())?;
    self.source_manager.set(Arc::new(mgr))
        .map_err(|_| LibraryError::Other("Already initialized".into()))?;
    Ok(())
}

pub fn source_manager(&self) -> Option<&Arc<SourceManager>> {
    self.source_manager.get()
}
```

### Ops Registration Pattern

All operations MUST register via macros at end of file:

```rust
// In ops/sources/create.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSourceAction {
    input: CreateSourceInput,
}

impl LibraryAction for CreateSourceAction {
    type Input = CreateSourceInput;
    type Output = SourceInfo;

    fn from_input(input: Self::Input) -> Result<Self, String> {
        Ok(Self { input })
    }

    async fn execute(
        self,
        library: Arc<Library>,
        context: Arc<CoreContext>,
    ) -> Result<Self::Output, ActionError> {
        let mgr = library.source_manager()
            .ok_or_else(|| ActionError::Internal("Source manager not initialized".into()))?;
        mgr.create_source(self.input).await
    }

    fn action_kind(&self) -> &'static str {
        "sources.create"
    }
}

// CRITICAL: Must register at end of file
crate::register_library_action!(CreateSourceAction, "sources.create");
```

### Job System Pattern

Jobs implement `Job` + `JobHandler` traits:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceSyncJob {
    pub source_id: Uuid,
    // State persisted between resumptions:
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
}

impl Job for SourceSyncJob {
    const NAME: &'static str = "source_sync";
    const RESUMABLE: bool = true;
    const VERSION: u32 = 1;
}

#[async_trait]
impl JobHandler for SourceSyncJob {
    type Output = SyncReport;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log("Starting source sync");

        // Emit progress:
        ctx.report_progress(SyncProgress {
            current_file: "example.json".into(),
            total_items: 100,
            // ...
        }).await?;

        // Do work...

        Ok(SyncReport { /* ... */ })
    }
}
```

### Database Migration Pattern

SeaORM migrations for library.db:

```rust
// migration/mXXXXXXXXXX_create_library_sources.rs
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LibrarySource::Table)
                    .col(ColumnDef::new(LibrarySource::Id).integer().auto_increment().primary_key())
                    .col(ColumnDef::new(LibrarySource::Uuid).uuid().not_null().unique())
                    .col(ColumnDef::new(LibrarySource::Name).string().not_null())
                    // ... more columns
                    .to_owned(),
            )
            .await
    }
}
```

Register in `migration/mod.rs`:

```rust
Box::new(mXXXXXXXXXX_create_library_sources::Migration),
```

---

## APPENDIX B: Porting Catalog

### Modules to Port As-Is (~7,200 lines)

| Module             | Lines | File Path                                    | Notes                |
| ------------------ | ----- | -------------------------------------------- | -------------------- |
| Schema parser      | 250   | spacedrive-archive-prototype/core/src/schema/parser.rs    | Pure TOML logic      |
| Schema codegen     | 562   | spacedrive-archive-prototype/core/src/schema/codegen.rs   | SQL DDL generation   |
| Schema migration   | 1,101 | spacedrive-archive-prototype/core/src/schema/migration.rs | Diff and apply logic |
| SourceDb       | 1,397 | spacedrive-archive-prototype/core/src/db.rs               | SQLite operations    |
| Source manager | 299   | spacedrive-archive-prototype/core/src/source.rs       | Folder lifecycle     |
| Registry           | 516   | spacedrive-archive-prototype/core/src/registry.rs         | Metadata management  |
| Adapter trait      | 169   | spacedrive-archive-prototype/core/src/adapter/mod.rs      | Interface definition |
| Script adapter     | 1,432 | spacedrive-archive-prototype/core/src/adapter/script.rs   | Subprocess runtime   |
| Search router      | 459   | spacedrive-archive-prototype/core/src/search/router.rs    | Hybrid search        |
| FTS search         | 13    | spacedrive-archive-prototype/core/src/search/fts.rs       | Query sanitization   |
| Vector search      | 294   | spacedrive-archive-prototype/core/src/search/vector.rs    | LanceDB integration  |
| Embedding model    | 80    | spacedrive-archive-prototype/core/src/embed.rs            | FastEmbed wrapper    |
| Safety model       | 568   | spacedrive-archive-prototype/core/src/safety.rs           | Prompt Guard 2       |

### Modules Requiring Adaptation (~1,800 lines)

| Module        | Lines | Modification                                    | Reason                |
| ------------- | ----- | ----------------------------------------------- | --------------------- |
| Engine        | 1,014 | Remove bundled adapters sync, use library paths | App-level assumptions |
| OAuth manager | 339   | Integrate with v2 KeyManager                    | Storage backend diff  |

### Modules to Defer/Stub (~680 lines)

| Module      | Lines | Status | Phase                          |
| ----------- | ----- | ------ | ------------------------------ |
| hash.rs     | 36    | Stub   | Phase 2.1 (filesystem adapter) |
| job/\*      | 12    | Stub   | Phase 3.1 (job queue)          |
| fs/\*       | 21    | Stub   | As needed for adapters         |
| analysis/\* | 8     | Stub   | Future feature                 |

### **CRITICAL: Secrets Store**

**DO NOT PORT** `secrets/` module (~870 lines). Use v2's existing `KeyManager` at `core/src/crypto/key_manager.rs`.

Adapter secrets use namespaced keys:

```rust
library.core_context().key_manager()
    .set_secret(&format!("adapter:{}:{}", adapter_id, "oauth_token"), token)
    .await?;
```

---

## APPENDIX C: Conflict Resolutions

### 1. LanceDB Dependency - RESOLVED ✅

**Problem:** v2 had LanceDB commented out due to gpui conflict.

**Resolution:**

- Delete `apps/gpui-photo-grid/` prototype (not used)
- Uncomment `lancedb = "0.15"` in `core/Cargo.toml` line 119
- Port spacedrive-data's vector search as-is (uses LanceDB)

### 2. Search Type Name Conflicts

**Problem:** Both v2 and spacedrive-data have `SearchResult` and `SearchFilter` types with different structures.

**Resolution:**

- Namespace spacedrive-data types: `source::SearchResult`, `source::SearchFilter`
- Keep v2's file search types unchanged
- Create separate ops: `ops/source_search/` vs `ops/search/`

### 3. Database Architecture Mismatch

**Problem:** spacedrive-data uses raw SQLx, v2 uses SeaORM.

**Resolution:**

- Keep SourceDb internal to `core/src/data/` module
- Expose only through v2-style ops (LibraryAction/LibraryQuery)
- SourceDb uses sqlx directly (justified for dynamic schemas)
- Library metadata stored via SeaORM in `library.db`

### 4. Storage Path Resolution

**Problem:** Where do sources live? Per-library or global?

**Resolution:** **Per-library** (matches v2 design):

```
MyLib.sdlibrary/
  ├─ library.db
  ├─ sources/               ← NEW
  │   ├─ registry.db        ← Optional separate DB, or use library.db
  │   └─ {source-uuid}/
  │       ├─ data.db
  │       ├─ embeddings.lance/
  │       └─ schema.toml
  └─ thumbnails/
```

---

## APPENDIX D: Atomic Implementation Plan

### Phase 0: Adapters (Zero-Effort Copy)

**Copy adapters directory:**

```bash
cp -r ~/Projects/spacedriveapp/spacedrive-archive-prototype/adapters ~/Projects/spacedriveapp/spacedrive/adapters
```

All 11 adapters are standalone (TOML + Python scripts). They communicate via stdin/stdout JSONL protocol - no Rust dependencies. Work immediately once ScriptAdapter runtime is ported.

**Adapters included:** Gmail, Obsidian, Chrome Bookmarks, Chrome History, Safari History, Apple Notes, OpenCode, Slack, macOS Contacts, macOS Calendar, GitHub.

### Phase 1: Create Standalone Crate

**1.1: Create crate structure**

```bash
mkdir -p crates/archive/src
cd crates/archive
cargo init --lib --name sd-archive
```

**1.2: Setup Cargo.toml**

```toml
[package]
name = "sd-archive"
version = "0.1.0"
edition = "2021"

[dependencies]
sqlx = { workspace = true, features = ["sqlite", "runtime-tokio"] }
tokio = { workspace = true }
serde = { workspace = true }
uuid = { workspace = true }
blake3 = { workspace = true }
toml = { workspace = true }
indexmap = { workspace = true }
dashmap = { workspace = true }
lancedb = { version = "0.15" }
fastembed = { version = "4" }
thiserror = { workspace = true }

# Optional safety features
ort = { version = "2.0.0-rc.9", optional = true }
tokenizers = { version = "0.21", optional = true }
hf-hub = { version = "0.4", optional = true }

[features]
default = []
safety-screening = ["dep:ort", "dep:tokenizers", "dep:hf-hub"]
```

**1.3: Port core modules to crate**

- Port `error.rs`, `engine.rs`, `schema/`, `db/`, `adapter/`, `search/`, `safety.rs`, `embedding.rs`
- Remove OpenAPI derives
- Remove app-level assumptions (bundled adapters, home dir defaults)
- Keep pure engine API

**1.4: Define public API in lib.rs**

```rust
pub use engine::{Engine, EngineConfig};
pub use error::{Error, Result};
pub use schema::{DataTypeSchema, FieldType};
pub use adapter::{Adapter, AdapterInfo, SyncReport};
pub use search::{SearchResult, SearchFilter};
// ... other exports
```

**Verification:** `cargo check -p sd-archive` passes

### Phase 2: Core Integration

**2.1: Add crate dependency**

```toml
# core/Cargo.toml
[dependencies]
sd-archive = { path = "../crates/sd-archive" }
```

**2.2: Create wrapper layer**

```rust
// core/src/data/mod.rs
pub use sd_archive::{
    Engine, EngineConfig, SearchResult, SearchFilter,
    SourceInfo, AdapterInfo, SyncReport,
};

pub mod manager;
pub mod integration;
```

**2.3: Library-scoped manager**

```rust
// core/src/data/manager.rs
pub struct SourceManager {
    engine: Arc<sd_archive::Engine>,
    library: Arc<Library>,
}

impl SourceManager {
    pub async fn new(library: Arc<Library>) -> Result<Self> {
        let config = sd_archive::EngineConfig {
            data_dir: library.path().join("sources"),
        };
        let engine = sd_archive::Engine::new(config).await?;
        Ok(Self { engine: Arc::new(engine), library })
    }

    // Wrap engine methods with library context
    pub async fn create_source(&self, input: CreateSourceInput) -> Result<SourceInfo> {
        self.engine.create_source(...).await
    }
}
```

**2.4: Add to Library struct**

```rust
// core/src/library/mod.rs
pub struct Library {
    // ... existing fields
    source_manager: OnceCell<Arc<SourceManager>>,
}
```

**2.5: Database Migration**

- Create migration for `library_sources` metadata table (if tracking in library.db)
- Or let sd-archive manage its own registry.db

**Verification:** `cargo check` passes, can instantiate manager

### Phase 3: Ops & Jobs

**3.1: Create ops directory**

```bash
mkdir -p core/src/ops/sources
```

**3.2: Implement operations**

- Create files in `ops/sources/` for create, list, get, delete, sync
- Register via `register_library_action!` and `register_library_query!`

**3.3: Example: Sync operation with job**

See code example in APPENDIX A for the full pattern (SourceSyncJob in ops/sources/sync.rs).

**3.4: Register in mod.rs**

```rust
// core/src/ops/mod.rs
pub mod sources;
```

**Verification:** Can sync source, screen records, embed vectors via jobs

### Phase 4: Search & Pipeline

**4.1: Search Router**

- Port `search/router.rs` (~460 lines)
- Add to Library as `OnceCell<Arc<SearchRouter>>`
- Register `sources.search` query

**4.2: Safety Policy**

- Port `safety.rs` trust tiers and policy (~100 lines)
- Apply policies during screening

**4.3: Embedding Integration**

- Port or reuse `EmbeddingModel`
- Add to CoreContext as singleton
- Share across all libraries

**Verification:** Can search across sources with hybrid search

### Total Effort Estimate

- **Lines to port:** ~7,200 (as-is) + ~1,800 (adapted) = **9,000 lines**
- **New code:** ~2,000 lines (ops, jobs, integration)
- **Total:** **~11,000 lines**
- **Execution time:** 20 minutes if following plan atomically

---

## APPENDIX E: Critical File Paths Reference

### V2 Patterns to Follow

**Op registration:**

- `core/src/ops/libraries/create.rs` - CoreAction example
- `core/src/ops/core/status.rs` - CoreQuery example

**Job implementation:**

- `core/src/ops/files/delete/job.rs` - Job + JobHandler traits

**Migration:**

- `core/src/infra/db/migration/m20250109_*.rs` - Migration pattern

**Entity:**

- `core/src/infra/db/entities/location.rs` - Entity model

**Domain:**

- `core/src/domain/library.rs` - Domain model + Identifiable

### Spacedrive-Data Source Files

**All source files relative to:** `/Users/jamespine/Projects/spacedriveapp/spacedrive-archive-prototype/core/src/`

**Core modules:**

- `error.rs`, `engine.rs`, `db.rs`, `registry.rs`, `source.rs`

**Schema:**

- `schema/mod.rs`, `schema/parser.rs`, `schema/codegen.rs`, `schema/migration.rs`

**Adapter:**

- `adapter/mod.rs`, `adapter/script.rs`

**Search:**

- `search/mod.rs`, `search/fts.rs`, `search/router.rs`, `search/vector.rs`

**Pipeline:**

- `embed.rs`, `safety.rs`, `oauth.rs`

### Target Paths in V2

**Standalone crate:** `/Users/jamespine/Projects/spacedriveapp/spacedrive/crates/archive/`

```
crates/archive/
├── Cargo.toml              (Package: sd-archive; Heavy deps: lancedb, fastembed, ort)
└── src/
    ├── lib.rs              (Public API exports)
    ├── error.rs
    ├── engine.rs
    ├── schema/
    │   ├── mod.rs
    │   ├── parser.rs
    │   ├── codegen.rs
    │   └── migration.rs
    ├── db/
    │   ├── mod.rs
    │   └── source_db.rs
    ├── adapter/
    │   ├── mod.rs
    │   └── script.rs
    ├── search/
    │   ├── mod.rs
    │   ├── fts.rs
    │   ├── router.rs
    │   └── vector.rs
    ├── source.rs
    ├── registry.rs
    ├── safety.rs
    └── embedding.rs
```

**Core integration wrapper:** `/Users/jamespine/Projects/spacedriveapp/spacedrive/core/src/data/`

```
core/src/data/
├── mod.rs                  (Re-exports from sd-archive)
├── manager.rs              (Library-scoped wrapper)
└── integration.rs          (KeyManager, EventBus bridges)
```

**Operations and jobs:** `/Users/jamespine/Projects/spacedriveapp/spacedrive/core/src/ops/sources/`

```
core/src/ops/sources/
├── mod.rs
├── create.rs            # CreateSourceAction
├── list.rs              # ListSourcesQuery
├── get.rs               # GetSourceQuery
├── delete.rs            # DeleteSourceAction + SourceDeleteJob
├── sync.rs              # SyncSourceAction + SourceSyncJob
├── search.rs            # SearchSourcesQuery
├── screening.rs         # SourceScreeningJob
├── embedding.rs         # SourceEmbeddingJob
└── settings.rs
```

**Adapters (copy as-is):** `/Users/jamespine/Projects/spacedriveapp/spacedrive/adapters/`
