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
5. **Enrichment is records and edges, not a blob.** Tags, albums, faces and
   places are one primitive: a definition with a stable identity, replicated
   into every source that uses it, plus membership edges local to the source
   holding the member. This is the previs's tag rule
   (*"tag definitions replicated into every source that uses them"*) applied to
   everything shaped like it. `record.type` is an open string, so `person`,
   `album` and `place` are records the way `file` and `note` are.

   `record_overlay` is for scalar assertions a person makes about one record —
   rating, favourite, a corrected title. Structured enrichment does not go in
   it; a face inside a JSON column cannot answer "which photos have this face."

6. **No shared cross-source assertion file.** `grouping` and `lib_edge` are
   deleted. Neither has a writer, a reader, or an op — `SourceManager` exposes
   `link`/`unlink`/`neighbors`/`upsert_grouping` as pass-throughs that nothing
   calls. Under decision 5 they are also the wrong shape: a person who appears
   in photos on three drives is a definition replicated three times, not a row
   in a library-level table that outlives all three. Detaching a drive should
   lose that drive's photos of them, which is the truth.

   `record_overlay` stays. The search router genuinely reads it, and it moves
   into `source.db` by dropping `source_id` from its key. Cross-source answers
   arrive with `catalog.db` in P5, designed against a real requirement.

## Open

- **`edge` cannot carry enrichment.** It is
  `(src_uuid, dst_uuid, type, ord REAL)` with `PRIMARY KEY (src_uuid, dst_uuid,
  type)`. A face is *person X at this rectangle in this photo* — the rectangle
  has nowhere to go, and the primary key allows one edge per pair per type, so
  the same person twice in one group shot is unrepresentable. Edges need a
  payload and an identity that is not the endpoint tuple. Settle this in P1;
  P2 writes the first filesystem records against it.
- **Definition identity across sources.** Decision 5 replicates a definition
  into every source that uses it, which only works if two copies are
  recognisably the same thing. Same question for tags and for faces; answer it
  once.
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

1. **Done.** Deleted `embed.rs`, `search/vector.rs` and `safety.rs`.
   `TrustTier` survives in `registry.rs` — it is declared by all eleven adapter
   manifests and is the input screening policy will key on.
2. **Done.** Removed the post-sync embedding and screening passes from
   `engine.rs` and the log lines reporting their counts.
3. **Done.** Dropped `_embedded_at`, `_safety_score`, `_safety_verdict`,
   `_safety_version` and `idx_record_verdict` from the record schema and every
   path that read them.
4. **Done.** Rewrote `docs/core/design/archive.md` and `docs/archive/README.md`
   against the code. The old versions documented a manifest format, a JSONL
   protocol and a sandbox that do not exist.
5. **Done.** Renamed `spine` → `record` across the source tree and sixteen
   docs, including the filename `2026-08-17-record-table-transplant.md`.

**Found while doing it.** The verdict gate was load-bearing in a way nothing
documented: `upsert` only refreshed the search index for records it had seen
before, because a new record was expected to enter the index later, when
screening cleared it. Screening ran off a stub, and if the model failed to load
the pass marked everything safe by a separate path — so the index was populated
by a fallback nobody had read. Removing the gate meant `upsert` had to index
unconditionally. `tests/record.rs` had a `clear_for_indexing` helper that
existed only to walk records past this, and `tests/adapters.rs` called
`mark_screened` on its probe for the same reason; both are gone. Also fixed
alongside: adapter stderr was piped and then dropped, closing the pipe under a
writing adapter — it is now drained into tracing.

### P1 — Re-charter the crate

1. **Done — the store is extracted.** `crates/store` (`sd-store`) holds the
   store and nothing else: `record`, facets, `content`, `edge`,
   `record_overlay`, schema parse/codegen/migration, `SourceDb`,
   `SourceManager`, per-source FTS, and `TrustTier` (source metadata, not
   adapter metadata). It depends on no other first-party crate. The name is
   `store` because "the source store" is already this document's vocabulary; a
   *source* is origin plus ingest plus store, and the crate is only the last of
   the three.

   `sd-archive` keeps what needs more than one source — `Engine`, `Registry`,
   `SearchRouter` — plus the adapter runtime, and sits on `sd-store`. It sheds
   `async-trait`, `futures`, `indexmap` and `dashmap` in the process.

   The rest of the split waits for P3 rather than being done speculatively.
   Once `library.db` owns the source list, `Engine`/`Registry`/`SearchRouter`
   have nothing left that is not core's and move there; what remains of
   `sd-archive` is the adapter runtime and gets renamed to match. Extracting
   the store is the part that unblocks P2, because the filesystem walker needs
   to write records without depending on an adapter engine.
2. **Give `edge` a payload** (see Open). P2 writes against it, so it lands here.
3. **Done.** Deleted `grouping` and `lib_edge` per decision 6, and with them
   the whole `library` module, `RecordKey`, and the `Engine`/`SourceManager`
   pass-throughs nothing called. `record_overlay` moved into `source.db` as
   `SourceDb::{set_overlay, get_overlay, overlays_for}`, keyed
   `(type, external_id)`. `registry.db` is now `sources` and `data_types`
   only — no durable layer, so there is no shared file left to argue about.

   **Semantic change.** Deleting a source now discards its assertions, because
   they live in the file being deleted. The old shape kept them in
   `registry.db` so re-adding a source rebound them. Re-index is the operation
   that preserves them — replace what an ingest produced, leave
   `record_overlay` alone — and it does not exist yet. Nothing calls
   `delete_source` on a source with assertions today, so this is a design
   change rather than a regression, but re-index has to land before either is
   reachable from an op.
4. **Done.** Retired the Apple Photos adapter; the reader is now
   `crates/photos` (`sd-photos`), standalone and read-only. Its API speaks
   ordinary units — unix milliseconds rather than Core Data seconds, a
   `MediaKind` rather than `ZKIND` — so every Apple-specific decode stays
   behind the boundary. It is the harvest path for decision 5's enrichment
   when P2 lands. `AdapterKind`, `register_native_adapters`, the
   `rusqlite`/`libc`/`dirs` dependencies and the `Import → Photos` entry all
   went with the adapter.
5. **Done.** Reconciled the crate docs with decision 2. `lib.rs`, `library.rs`,
   `source.rs` and `record.rs` all opened by calling the source store
   disposable; they now say it is user data and point at P1 for where the
   durable tables end up.

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
