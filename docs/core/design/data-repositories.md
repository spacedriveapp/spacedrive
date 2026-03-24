# Data Repositories Integration Design

## Purpose

Fold the `spacedrive-data` prototype into the official `spacedrive` codebase as a new library-scoped feature without forcing convergence between the VDFS index and the archival repository engine.

This keeps Spacedrive's file-native architecture intact while adding a second data plane for extracted, adapter-driven repositories such as Gmail, Obsidian, Chrome History, Slack, and GitHub.

## Decision

Spacedrive will support two storage systems inside a library:

1. The VDFS library database, which remains the source of truth for files, entries, content identities, tags, spaces, sidecars, sync metadata, and file operations.
2. A repository engine, which manages archived external data sources as isolated repositories with their own SQLite database, vector index, schema, cursor state, and processing pipeline.

These systems are linked at the library boundary, not merged into one database.

## Why This Shape

- The VDFS already solves file indexing, sync, sidecars, jobs, and device-aware lifecycle well.
- The prototype already solves schema-driven repositories, adapter ingestion, hybrid search, and isolated archival storage well.
- Forcing repository records into the main library database would create a large migration with little product value.
- Keeping repositories isolated preserves portability, adapter flexibility, schema evolution, and per-source lifecycle control.
- Registering repositories at the library level gives us one user-facing primitive for ownership, sync, permissions, and UI.

## Goals

- Add archival repositories to official Spacedrive without rewriting the VDFS.
- Reuse the existing daemon, RPC, type generation, ops, job system, and UI infrastructure.
- Keep each repository self-contained on disk.
- Make repositories library-scoped so they can participate in library sync and lifecycle.
- Translate the prototype's pipeline into Spacedrive's ops and jobs model.

## Non-Goals

- Do not merge repository records into the VDFS entry index.
- Do not split a library into multiple primary databases yet.
- Do not ship a separate OpenAPI server for this feature.
- Do not force the repository engine to use SeaORM for dynamic schema tables.
- Do not redesign the whole search stack to unify files and repositories in the first slice.

## User Model

A library owns:

- its existing VDFS database and sidecars
- zero or more archival repositories

A repository is a managed library resource. It is visible in the library UI, syncable across devices, and controlled by library operations. Its payload stays in a separate repository folder.

Examples:

- Library: `Personal`
  - VDFS index for files and locations
  - Repository: `Work Gmail`
  - Repository: `Obsidian Vault`
  - Repository: `Chrome History`

## Storage Layout

Each library gets a repository root inside `.sdlibrary`.

```text
.sdlibrary/
  library.db
  sidecars/
  repositories/
    registry.db
    <repository_id>/
      data.db
      embeddings.lance/
      schema.toml
      state/
      cache/
```

### Library Database Responsibilities

The library database stores repository metadata only:

- repository id
- library id
- display name
- adapter id
- repository path
- trust tier
- visibility
- status
- last sync timestamps
- sync policy
- pipeline policy
- device sync metadata

### Repository Folder Responsibilities

Each repository folder stores source-specific payload:

- generated SQLite tables from schema
- FTS tables and triggers
- vector index
- adapter cursor state
- adapter-specific caches
- local processing artifacts that are not sidecars

## Integration Boundary

The integration point is `Library`, not `CoreContext` globally and not the VDFS entry graph.

The library becomes the owner of a `RepositoryManager` and a `RepositoryRegistry` alongside its existing services.

```text
Core
  -> LibraryManager
    -> Library
      -> VDFS database and services
      -> Repository subsystem
           -> repository registry
           -> repository manager
           -> adapter runtime
           -> repository search
           -> repository pipeline jobs
```

This means repository operations should mostly be library actions and library queries.

## Proposed Code Shape

Add a new subsystem under `core/src/repository/` or `core/src/data/`.

Recommended shape:

```text
core/src/data/
  mod.rs
  manager.rs           # library-scoped repository manager
  registry.rs          # metadata persisted in library db or library-owned registry db
  repository.rs        # open/create/delete repository folders
  engine.rs            # orchestration facade used by ops/jobs
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
    repository_db.rs
  search/
    mod.rs
    router.rs
    fts.rs
    vector.rs
  secrets/
    mod.rs
  safety/
    mod.rs
  classify/
    mod.rs
```

This subsystem can keep `sqlx` and raw SQL internally where that makes dynamic schemas practical. The outer integration surface should still follow Spacedrive conventions: ops, jobs, events, and typed outputs.

## Why Keep `sqlx` Internals

The repository engine generates tables dynamically from TOML schemas. That fits raw SQL much better than SeaORM entities.

The integration rule should be:

- use existing Spacedrive infrastructure for lifecycle, dispatch, jobs, events, sync, and UI
- keep raw SQL and schema codegen inside the repository engine where it reduces friction

This avoids rewriting working prototype internals just to satisfy the ORM used by the VDFS.

## Library Registration Model

Repositories should be first-class library resources.

We add a new library-scoped domain concept, likely `library_repository`.

Suggested fields:

```text
id
library_id
name
adapter_id
repository_root
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

The exact cursor should live inside repository storage if it is adapter-owned. The library record only needs summary metadata for UI and sync orchestration.

## Operations Mapping

The prototype's API should be translated into V2 ops.

### Library Actions

- `repositories.create`
- `repositories.update`
- `repositories.delete`
- `repositories.sync`
- `repositories.sync_all`
- `repositories.set_visibility`
- `repositories.set_policy`
- `repositories.release_quarantined`
- `repositories.delete_record`
- `repositories.adapters.install` if adapter installation remains user-facing

### Library Queries

- `repositories.list`
- `repositories.get`
- `repositories.search`
- `repositories.records.list`
- `repositories.records.get`
- `repositories.adapters.list`
- `repositories.schemas.list`
- `repositories.quarantine.list`
- `repositories.status`

These should register through the existing macros in `core/src/ops/registry.rs` and flow through the current daemon transport.

## Job System Mapping

The prototype's pipeline should become library jobs.

### Core Jobs

- `RepositorySyncJob`
- `RepositoryScreeningJob`
- `RepositoryEmbeddingJob`
- `RepositoryClassificationJob`
- `RepositoryReindexJob`
- `RepositoryDeleteJob` for heavy cleanup

### Sync Flow

First slice:

```text
repositories.sync action
  -> enqueue RepositorySyncJob
     -> adapter subprocess emits JSONL
     -> repository DB upsert/delete/link
     -> enqueue or run screening stage
     -> enqueue or run embedding stage
     -> update library repository status
     -> emit events
```

The job system gives us resumability, progress reporting, cancellation, and a natural home for future classification work.

## Search Model

Initial scope:

- repository search is separate from file search
- the UI can offer a dedicated repository search surface inside a library
- unified cross-surface search is deferred

This avoids destabilizing `core/src/ops/search/` in the first integration slice.

### Future

Later we can add a federated search query that fans out to:

- VDFS file search
- repository search

and merges results at the query layer without forcing a shared storage model.

## Sync Between Devices

Repositories are library-scoped resources, so library sync should distribute repository metadata and availability.

Recommended phases:

### Phase 1

- sync repository metadata through the library
- do not sync repository payload automatically
- a second device sees that the repository exists and can pull or restore it later

### Phase 2

- sync repository payloads as managed library resources
- transport repository bundles or deltas through the existing file sync machinery where practical
- preserve repository isolation on disk

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

Do not keep the prototype's app-level silo if the library already has stronger primitives available.

Recommended direction:

- secrets remain managed by Spacedrive's existing secure storage model where possible
- repository config stores secret references, not raw values
- OAuth tokens are attached to the library-owned repository resource

If the prototype secrets store is easier to land initially, it can be embedded behind the repository engine and migrated later. The UI and ops surface should not expose that implementation detail.

## Safety, Classification, and Trust

These concepts map well to Spacedrive.

- trust tier belongs to the repository metadata
- safety verdict and quality metadata belong to repository records
- quarantine is a repository view, not a VDFS concept
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

Instead, add repository features into the existing interface:

- library section for repositories
- add-source flow
- repository detail page
- repository search page
- quarantine queue
- adapter settings and installed adapters

This keeps one product and one navigation model.

## Migration Strategy

### Phase 0: Design and Carve-Out

- add this design doc
- choose final module path under `core/src/`
- decide which prototype modules copy over largely intact
- define library metadata schema for repository registration

### Phase 1: Engine Import

- port schema parser, codegen, repository DB, search router, adapter runtime
- remove OpenAPI and standalone server assumptions
- wrap the imported subsystem in a library-scoped manager

### Phase 2: Library Registration

- add library models and migrations for repository metadata
- create repository folders under `.sdlibrary/repositories/`
- expose create/list/get/delete ops

### Phase 3: Sync and Pipeline Jobs

- add sync action and `RepositorySyncJob`
- port screening and embedding stages
- wire progress and status events

### Phase 4: UI Slice

- add repository list, create flow, detail page, and search page
- expose quarantine state

### Phase 5: Device Sync

- propagate repository metadata through library sync
- later add payload transfer strategy

### Phase 6: Unified Search

- optional federated search query across VDFS and repositories

## Expected Reuse from the Prototype

Likely to port mostly intact:

- schema parser/codegen/migration
- repository manager and repository DB
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

Dynamic repository tables are a poor fit for SeaORM. Forcing convergence here would slow the project down.

### Search Creep

Trying to unify file search and repository search in the first pass will expand scope fast.

### Sync Scope

Full repository payload sync is valuable, but not required to land the first product slice.

### Over-Refactoring

The goal is not to perfect the architecture first. The goal is to land the repository engine in the official product with clean boundaries.

## Open Questions

1. Should repository metadata live in `library.db` directly or in a small library-owned `repositories/registry.db`?
2. Should repository secrets attach to the global key manager immediately or stay engine-local for the first slice?
3. Should repository search results reuse the existing search result envelope or define a repository-specific output first?
4. How do we want repository payload sync to package data, file sync of repository folders, bundle export/import, or delta protocol?

## Recommendation

Start with the smallest honest integration:

- keep repositories separate from the VDFS index
- make them library-scoped resources
- port the prototype internals with minimal rewrites
- expose everything through ops and jobs
- ship repository-local search before unified search

This gets the archival product into official Spacedrive quickly without abandoning the VDFS or reopening the whole architecture.
