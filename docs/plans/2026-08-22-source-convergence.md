# Source Convergence

> Coherence pass before the entries teardown continues. The teardown
> (`2026-08-20-entries-teardown-execution.md`) is the register for deleting the
> old world; this is the register for making the new one one thing instead of
> three.
>
> **Related.** `2026-08-20-architecture-previs.md` is the destination.
> `2026-08-21-filesystem-source-store.md` designs the filesystem writer.
> `docs/core/design/file-backed-sources.md` sets the rule that adapters enrich
> filesystem records rather than shadowing them.

## The rule

**Spacedrive is a set of sources. A source has an origin, an ingest, and a
store. Filesystem and adapter sources differ only in ingest.**

Everything below follows from holding that line. Where the tree disagrees with
it today, the tree is wrong.

## The problem

There are four models of "a thing in Spacedrive," and no two of them agree on
what is authoritative.

| Model | Where | Written by | Read by | Persisted as |
|---|---|---|---|---|
| `entry`, `entry_closure`, `location`, `directory_paths`, `content_identity` | `core/src/infra/db/entities/` | `ops/indexing/handlers/persistent.rs`, `database_storage.rs` | the old ops | `library.db` — durable, scheduled for deletion |
| `FileNode` + `PackedMetadata` | `ops/indexing/ephemeral/types.rs` | walker → `MemoryAdapter` → arena | `files/query/*`, `search`, `tags/apply`, `volumes`, `mounts` — everything the UI actually shows | postcard snapshot v2, deletes itself on version mismatch |
| record table + facets | `crates/archive` | `ScriptAdapter` only | `ops/sources/*` only | `<library>/archive/sources/<id>/data.db` |
| `source.db` | `infra/source_dirs.rs:84` | nothing | nothing | zero callers |

So the authoritative filesystem schema today is `PackedMetadata` — sixteen
bytes, state/type/size bit-packed into a `u64` — and its only durable form is a
file whose documented policy is delete-on-decode-failure. The SQL entry tables
are still live and still written. The record table, which exists to replace
both, has never seen a file.

Two source registries also exist, both calling their rows sources:
`ops/indexing/ephemeral/sources.rs` (JSON, filesystem) and
`sd_archive::Registry` (`registry.db`, adapters). `ops/sources/*` serves only
the second. Filesystem sources surface under `ops/indexing`, `ops/locations`
and `ops/volumes` instead.

## Disposability was an inversion

`crates/archive/src/library.rs` opens by stating that source indexes are
disposable and only the shared `registry.db` is durable. The teardown register
already ruled the other way — *"a source store is never rebuilt; it is user data,
not a cache"* — so the crate is arguing a case that has been decided against it.

The reasoning, written down so it stops getting lost: **rebuildable is a
property of one source on a good day, not a property of a store.** A detached
drive, a revoked token, a closed account, a renamed file — in each of those the
store is the only copy that exists. And separately, nothing a person or an agent
added ever came from the source at all. "You can rescan it" described the happy
path of one ingest and got promoted to an architectural invariant.

## What the crate accumulated

Not style, mechanics:

- `embed.rs` returns zero vectors. `search/vector.rs` returns empty results.
  `safety.rs` marks everything safe. Three stubs.
- `engine.rs` runs a post-sync screening pass and a post-sync embedding pass
  over those stubs, then logs `"generated embeddings after sync"` with a count.
  It reports work that did not happen.
- Every record carries `_embedded_at`, `_safety_score`, `_safety_verdict` and
  `_safety_version` for pipelines that do not exist. A verdict column that
  always reads `safe` is worse than no column, because it reads as a guarantee.
- `adapter/apple_photos.rs` is a native adapter compiled into the crate while
  eleven script adapters live in `adapters/`. `file-backed-sources.md` states
  that its model — minting a parallel record universe over bytes a filesystem
  source will also index — is the shape contract 1 forbids.
- `docs/core/design/archive.md` and `docs/archive/README.md` both describe
  archive as a second data plane beside the VDFS, which is the opposite of this
  direction.

## Target shape

- **The store crate** owns the record table, facets, content, edges,
  assertions, schema codegen and migrations. No adapter runtime, no search
  backends, no bundled models. This is what core depends on, and depending on
  it becomes obviously correct rather than incidental.
- **Adapters consume the store**, peer to the filesystem walker. `ScriptAdapter`
  and the walker are two writers over one set of tables — already the decision
  in `2026-08-21-filesystem-source-store.md`, not yet reflected in where the
  code sits.
- **Apple Photos leaves.** Under `file-backed-sources.md` it is enrichment
  joined onto filesystem records, not an adapter that owns records.
- **One registry, in `library.db`.** One row per source whether it is a drive or
  a Gmail account. `registry.db` and `<library>/archive/` both go; `SourceDirs`
  becomes the only layout and `data.db` becomes `source.db`.
- **Stubs are deleted, not kept.** Safety screening stays a design position
  worth writing down — quarantine before an agent reads it — and its columns
  return with the implementation.

## Decisions

1. **"Spine" is retired.** The module is `record`, the constant is
   `RECORD_SCHEMA`, and the prose says "the record table" and "records and
   facets." Applied to the source tree on 2026-08-22; the docs still carry the
   old word.
2. **A source store is durable.** Restated here because the crate's own docs
   still contradict it. Rebuild is a recovery path, not a design assumption.
3. **One store shape, two writers.** Carried forward from
   `2026-08-21-filesystem-source-store.md`. Cross-source search, cross-source
   edges and `catalog.db` all join on one shape; two shapes means two of
   everything downstream, permanently.
4. **One source registry.** Filesystem and adapter sources are the same row
   with different ingest. This is `T6.1` pulled forward, because leaving two
   registries alive through the filesystem-store work is what makes that work
   ambiguous.

## Open

- **`lib_edge` has no home.** Previs decision 1 moves durable tables into each
  `source.db`, but cross-source edges are cross-source by definition and
  `catalog.db` is phase 5. Either edges live in a shared file — partially
  conceding decision 1 — or cross-source assertions cannot be written until the
  catalog lands. Rule on this before P2 rather than discovering it inside P2.
- **Bundled adapter installation.** `core/src/data/manager.rs` resolves bundled
  adapters through `env!("CARGO_MANIFEST_DIR")` at compile time. That works from
  a cargo tree and nowhere else. Needs to read from app resources before
  anything ships.
- **T2.0b, volume/source boundary**, unchanged from the teardown register and
  still blocking T6.1 — which P3 now depends on.

## Phases

### P0 — Truth pass

No behaviour changes. Everything here is deleting or correcting claims, so the
later phases are not read against fiction.

1. Delete `embed.rs`, `search/vector.rs` and the `safety.rs` classifier stub.
2. Delete the post-sync embedding and screening passes in `engine.rs`, and the
   log lines reporting their counts.
3. Drop `_embedded_at`, `_safety_score`, `_safety_verdict`, `_safety_version`
   from the record schema and every read path that projects them.
4. Rewrite `docs/core/design/archive.md` and `docs/archive/README.md` to
   describe sources rather than a second data plane beside the VDFS.
5. Rename `spine` → `record`. Source tree done 2026-08-22, `cargo check -p
   sd-archive --all-targets` clean. Sixteen docs still carry the old word,
   including the filename `2026-08-17-record-spine-transplant.md`.

### P1 — Re-charter the crate

1. Store in, adapter runtime out, `apple_photos` out.
2. Settle `lib_edge` (see Open).
3. Reconcile the crate docs with decision 2 — `library.rs`'s opening paragraph
   is the specific thing that is wrong.

### P2 — The filesystem writes the store

Teardown `T2.1`, `T2.2`, `T2.5`, unchanged in substance: `facet_file`, a batched
writer, `upsert` reimplemented as a batch of one, and the arena demoted to a
read cache over a durable store. This is where the filesystem schema gets an
authoritative home for the first time.

### P3 — One registry

Teardown `T6.1`, pulled ahead of `T4`. `library.db` owns sources; `registry.db`
and `<library>/archive/` are removed; `ops/sources/*` serves both kinds. The
`ephemeral` qualifier retires here rather than at `T6.7`, because after this
phase there is nothing for it to distinguish.

### P4 — Delete entries

The remaining `T6.x`, now with one target instead of two.

### P5 — Catalog

Teardown `P7`, unchanged.

## Against the existing register

`P2`, `P4` and `P5` are `T2.x`, `T6.x` and `P7` as already written. `P0` and
`P1` are new. `P3` is `T6.1` moved earlier. Nothing in the teardown register is
cancelled; the ordering changes and two phases are inserted ahead of it.
