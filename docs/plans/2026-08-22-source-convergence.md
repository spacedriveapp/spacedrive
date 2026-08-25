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
> `docs/core/design/source-durability.md` settles what a store owes when its
> origin stops answering, and what the assertion tables have to reserve so sync
> is a transport problem later instead of a schema migration.

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

`crates/archive/src/library.rs` opened by stating that source indexes are
disposable and only the shared `registry.db` is durable. The teardown register
had already ruled the other way (*"a source store is never rebuilt; it is user
data, not a cache"*), so the crate was arguing a case that had been decided
against it.

Both claims were half right, which is why the argument kept coming back. **A
store has two halves.** The generation is rebuildable for exactly as long as its
origin still answers. The assertion layer never is, on any day, for any source,
because nothing a person or an agent added ever came from the origin at all.

And whether the origin answers varies per source, varies over time, and changes
without an event. An archived email account is rebuildable right up until the
day it closes, and that day is not on the calendar. Since the code cannot tell
which regime it is in, it has to behave as though the store is the only copy.
"You can rescan it" described the happy path of one ingest and got promoted to
an architectural invariant.

`docs/core/design/source-durability.md` carries this out to origin availability
as a registry column, the reindex and evict operations the generation and
assertion split makes expressible, and what sync needs reserved now.

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
2. **No code path may assume a store can be rebuilt.** The invariant is a rule
   about code, not a claim about stores: the generation is rebuildable while its
   origin answers, the assertion layer never is, and the transition between
   those is silent. Rebuild is an operation a person asks for when the origin is
   known to be answering. `docs/core/design/source-durability.md` has the rest,
   including the three things the schema has to reserve for sync.
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
- **Can sources nest.** Two live documents answer this in opposite directions.
  `2026-08-18-storage-consolidation.md` contract 1: one physical filesystem owns
  one source id and one record namespace, and *"the source registry rejects a
  root nested under an existing source's root."*
  `2026-08-20-architecture-previs.md:74`: *"One volume hosts any number of
  sources (nested roots are ordinary)."* If nesting is allowed, one file gets
  two records with two uuids in two stores, and a tag applied through one root
  is invisible through the other, which is the second-namespace problem contract
  1 exists to forbid, reappearing between two filesystem sources. Settle before
  P3, since the registry is where the rejection rule would live.
- **Two devices indexing one origin.** A NAS mounted on a laptop and a desktop
  is ordinary, and each machine mints its own record uuids for the same files.
  The rebind procedure resolves on path, inode, size and mtime, of which only
  path crosses a machine boundary. P1.7 gives hashed files an answer through the
  convergent content id. Unhashed files have none, and this is the first thing
  sync will hit.

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
   disposable. They now carry the two-halves framing and point at
   `docs/core/design/source-durability.md`.
6. **Done.** Re-keyed the assertion layer. `record_overlay` keyed on
   `(type, external_id)`, which is portable across devices and does not survive
   a rename, since the filesystem rebind procedure rewrites `external_id` when a
   file moves. It now keys on `record_uuid` and carries
   `(type, external_id, content_uuid)` as rebind evidence plus `hlc` and
   `device_uuid`, matching `tag_assertion` in
   `docs/core/design/tags-and-assertions.md`. It has no foreign key to `record`
   on purpose: a cascade would delete the one half of a store nothing can
   rebuild.

   **Found while doing it.** The old key was doing real work that the new key
   has to replace explicitly. Deleting a record and re-ingesting it mints a
   fresh uuid, and under the path key the assertions rebound for free. So the
   evidence columns needed a reader: `SourceDb::rebind_overlays` walks rows
   whose record uuid no longer resolves and rehomes them, content uuid first
   and the source key second, declining a target that already carries
   assertions of its own. That is the procedure a rebuilt generation and a
   store copied from another device both need, and without it the evidence
   would have been three columns nothing consulted.
7. **Done.** Convergent content ids, absorbing teardown T2.7. `content` gained
   a `uuid` column derived as `uuid_for(hash) = v5(CONTENT_NAMESPACE, hash)`
   (`crates/store/src/content.rs`), `sampled_hash` is `UNIQUE`, and
   `set_content_identity` is an upsert, so two copies of one file inside a
   source share a content row instead of minting one each.

   Two things the design settled themselves once written down. The uuid is
   derived from the strongest hash present, so it changes when the integrity
   tier lands, which is why `record.content_id` stays the local rowid: it has to
   be stable across the ladder while the name of the bytes is not. And a later
   write carrying only the cheap hash must not walk a confirmed id back down to
   a guess, so the upsert keeps the existing uuid unless the incoming row brings
   an integrity hash. `ContentId::{Candidate, Confirmed}` carries the tier in
   the type, per the previs.

   The namespace is `v5(DNS, "content.spacedrive.app")` rather than the RFC 4122
   DNS namespace the entries world reused, and it is pinned by a test, since
   changing it is the same as declaring that no two installs have ever seen the
   same file.
8. **Done.** `record.uuid` is a BLOB: 16 bytes against 36, on the primary key,
   in `idx_record_parent`, on both sides of every edge, and on every facet
   row's `record_uuid`. `2026-07-29-per-source-databases.md` called this one and
   named the window. The crate's public surface speaks `Uuid` rather than
   `String` throughout; `SearchResult.id` stays a string because it is a wire
   type.

   One thing to know before writing SQL against this: `json_object` refuses
   BLOBs, so `list_records_full` selects the uuid beside the JSON and merges it
   in rather than inside it.

9. **The filesystem is a data type.** `filesystem_schema()` in
   `crates/store/src/file.rs` declares one model, `file`, in Rust rather than
   TOML. The facet DDL, the `_schema` row and the migration diff then come from
   the machinery every adapter already uses, so `facet_file` needs no
   declaration of its own. The search contract is empty, which `diff_schemas`
   tolerates.

### P2 — The filesystem writes the store

Teardown `T2.2` and `T2.5`: `facet_file`, a batched writer, and the arena
demoted to a read cache over a durable store. This is where the filesystem
schema gets an authoritative home for the first time.

1. **Done.** The store side. `crates/store/src/file.rs` carries the ingest:
   `Observation` (what the walk saw), `Ledger` (every binding, in memory),
   `Resolution` (what the ledger made of it), and `SourceDb::apply_files`, which
   writes a batch, its removals and its watermark in one transaction. Killing
   the process partway through leaves neither a half-written batch nor a
   watermark claiming work that did not land.

   The two writers share one statement rather than one code path, which is what
   "one file format, two writers" turns out to mean in practice.
   `INSERT_RECORD` conflicts on the uuid rather than on `(type, external_id)`,
   because the walker resolves identity before it writes and a moved file is the
   same record at a new key. `UNIQUE (type, external_id)` still stands, so two
   records claiming one path fails loudly. `upsert` is not "a batch of one": the
   adapter path resolves relations and writes arbitrary facet columns out of
   JSON, the walker writes six typed columns, and the record table is the part
   they genuinely have in common.

   **No index on `facet_file.inode`,** against what
   `2026-08-21-filesystem-source-store.md` specifies. Resolution runs in memory
   off `Ledger::load`, whose access pattern is one sequential scan at attach, so
   the index would cost a write on every file in the source to serve a query
   nobody makes. It comes back the day something queries inodes directly.

2. **Done.** The core side. `core/src/ops/indexing/ephemeral/store.rs` is the
   seam: one writer task per source owns the `Ledger` and the `SourceDb`, and
   `SourceSlot` opens it on first write against `SourceDirs`, so a source's
   store sits in the same directory as its snapshot and its thumbnails. The
   queue applies backpressure rather than dropping — a store that quietly
   skipped observations would claim a completeness it does not have.

   **`MemoryAdapter` was not the fan-out point.** The plan said the walk and
   the watcher converge there; only the watcher does. The ephemeral walk
   batches straight into the arena from
   `IndexerJob::run_ephemeral_processing_static`, and `MemoryAdapter`'s
   `IndexPersistence` half has no caller at all. So there are two fan-out
   sites, not one: the watcher through `MemoryAdapter`'s `ChangeHandler`
   methods, and the walk through the job's batch loop. Collapsing them is `P4`
   work, not a precondition for this.

   External ids are `/`-separated on every platform, so a drive indexed on one
   reads on another. `apply_files` orders each batch parents-first, because
   `record.parent_uuid` is a foreign key and the watcher reports what the
   kernel coalesced in whatever order it coalesced it.

   **No watermark from this path.** `apply_files` takes one and the filesystem
   passes `None`. A resume point means "the walk got this far", and the walk
   has no ordering guarantee to hang that on yet. The batch is still the unit
   of durability; what is missing is the ability to skip work already done.

   **The sweep opens only for a walk that could see everything.**
   `IndexerJobConfig::enumerates_whole_source` is the gate: volume indexing,
   recursive, no depth limit, no rules. A sweep reads absence as deletion,
   which is sound only when the walk would have seen the file had it been
   there — so it rules out a browse, which stops at one directory, and every
   walk that applies rules, which hide files on purpose. Today that leaves one
   path: archiving a removable drive, which is also the one case where the
   origin goes in a drawer and absence is the only signal there will ever be.

   Deletions on internal volumes and browsed directories reach the store only
   through the watcher. A file removed while the daemon was down leaves a ghost
   there until something re-archives the source. Accepted: those origins are
   present and re-walkable on demand, so absence is never the only evidence
   available.

   **A locked folder costs its subtree, not the walk.** `discovery.rs` treats
   an unreadable directory as a non-critical error, so a walk finishes
   successfully with that subtree missing entirely. `Ledger::finish_sweep`
   takes the paths the walk failed to open and spares everything under them,
   keeping their bindings intact so the next walk that can read them resolves
   them as unchanged.

   Interrupt is safe without special handling: `check_interrupt` returns before
   the sweep closes, so a killed walk leaves it open for the resume rather than
   closing it over half an enumeration. `IndexerState.sweep_open` carries that
   across serialization, and a walk resumed in a *new* process finds a ledger
   with no sweep running and closes nothing.

3. **Next.** `entry_uuids` leaves `ephemeral.snapshot`, which is what makes the
   snapshot purely rebuildable and all five of its discard paths harmless.

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
