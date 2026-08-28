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
- **Can sources nest.** Settled in P2.7: yes, once a partition is keyed by the
  drive rather than by a registration. Both live documents were right about
  their own half. `2026-08-18-storage-consolidation.md` contract 1 is right that
  one filesystem owns one record namespace, and that namespace is the arena.
  `2026-08-20-architecture-previs.md:74` is right that nested roots are
  ordinary, because a source is a scope over that namespace rather than a second
  copy of it. The rejection rule the registry would have needed is what P2.7
  removes the need for.
- **How a filesystem record is addressed.** Settled in P2.6: full paths on
  directories, sibling keys on files. Left here as the question that produced
  it, since the same trade recurs for any source with a tree.
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

   That split has already cost one bug. A move updated the arena and the store
   correctly and emitted nothing to clients, because `emit_change_event` is
   handed the entry as it was *before* the move and looked up a path the file
   had left. Fixed by emitting from `move_entry`, which is the only place
   holding both ends. `P4` carries the structural answer: one fan-out, with the
   store write and the client event both derived from the arena mutation.

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

3. **Done, and it was worse than this line said.** The arena and the store were
   minting *separate* uuids for the same file: `MemoryAdapter` generated one
   inline for the arena while `Ledger::resolve` minted its own for the record.
   Tags hang off the first and `record_overlay` assertions off the second, and
   nothing joined them. Indexing anything at scale before fixing that would
   have baked the split into every row.

   The ledger now assigns identity and the arena writes what it is told:
   `SourceStore::identify` resolves a batch and answers with record uuids,
   which is also where the batch gets taken in, so there is no second call that
   could disagree. Resolution is a hash lookup and the reply lands before
   anything commits, so a walk pays one queue round trip per batch rather than
   a transaction. Volume indexing no longer defers uuids, because the ledger
   has one whether or not anything asks.

   `entry_uuids` is still in `ephemeral.snapshot` and it no longer matters: it
   now holds the same uuid the ledger does, so it is a cache rather than the
   only copy, and discarding it costs a re-identify rather than an identity.
   Dropping the field is cleanup that can follow the arena growing a read path
   to the ledger.

### P2.5 — What a record carries

Measured against the last pre-sources library, on the same machine: 2,123,029
records in
`~/.spacedrive/sources/01a03cc9649e77428b43199715ecdef6/data.db` against
23,157 entries in the backed-up `library.db`. The new store holds a hundred
times the files and less about each of them, and three separate causes are
tangled together in that sentence.

| | old, 23,157 entries | new, 2,123,029 records |
|---|---|---|
| `created_at` | 100% | 8 rows |
| parent link | 99.96% | 1.0% |
| content identity | 87% | 0 rows |
| transitive ancestry | 84,106 closure rows | no table |
| image / video metadata | 1,949 rows | no facet |
| sidecars | 7,358 rows | not in the store |
| `permissions` / `mode` | 0% | 0% |
| `accessed_at` | 0% | no column |

The last two rows are worth stating plainly: atime and permissions were empty
in the old library too, so those are an equal gap rather than a regression.
Everything above them is a real loss.

1. **The walk discards metadata it already has.** `DirEntry`
   (`ops/indexing/state.rs:79`) carries path, kind, size, mtime and inode, and
   `run_ephemeral_processing_static` fills the rest with `accessed: None,
   created: None, permissions: None`. The `fs::Metadata` behind each entry has
   `created()`, `accessed()` and `mode()` on it already, so the stat is paid
   for and the fields are dropped on the floor. Birth time is the expensive one:
   it is the timestamp a photo library sorts by, it is unrecoverable once the
   file is copied, and every day of indexing without it loses more of them.
   `EntryMetadata` and `facet_file.mode` both have the columns waiting.

2. **The bulk walk does not build the tree.** Parent linkage is
   `parent_of(external_id).and_then(|p| ledger.uuid_of(p))`, so a child is
   linked only if the ledger has already resolved its directory. The incremental
   path satisfies that and the walk does not:

   ```
   06:37-06:39   2,102,811 records from the bulk walk       1,349 parented
   10:48-10:49      19,856 records from browse and watcher  19,769 parented
   ```

   240,122 directories exist as records and 949 of them know their parent. The
   fix is ordering rather than schema: resolve a directory before the batch
   carrying its children, or resolve parents within a batch first, which
   `parents_first` already does one layer down in `apply_files` for exactly this
   reason.

3. **No media facets.** `filesystem_schema()` declares one model with six
   fields. The old library had `image_media_data` (EXIF, GPS, camera, lens,
   orientation, blurhash) and `video_media_data` (codec, fps, colour primaries,
   duration, audio stream). The facet mechanism is schema-driven and already
   generates `facet_file` from a declaration, so `facet_image` and `facet_video`
   are a schema entry rather than new machinery. Their writer is enrichment,
   which is parked, but declaring them costs nothing and stops the shape being
   invented twice.

4. **Sidecars belong in the source store.** A sidecar is keyed by content uuid
   and `content` now lives in the source store, so leaving the row in
   `library.db` makes it a foreign key into a different database file with
   nothing enforcing it. That is the fingerprint join this plan removed,
   reappearing between content and its artifacts: re-index a source, mint new
   content rows, and the sidecar rows point at nothing with no way to notice.
   The old schema could cascade because both tables shared a file.

   The archival case agrees. Previews for a drive in a drawer should travel with
   the drive rather than living in a library on a machine that may not be the
   one that plugs it in next, which is only possible if the row and the file are
   both source-scoped.

   The old design's two tables split cleanly on the new boundary, and they are
   different kinds of fact:

   - `sidecar` is *this artifact was derived from these bytes*. Content-scoped,
     device-independent, true wherever the drive is plugged in. It becomes
     `content_sidecar` in the source store, keyed
     `(content_uuid, kind, variant)`, sitting beside `content` rather than
     hanging off a record. Not a facet: facets are one row per record, and one
     content has many artifacts across kinds and variants.
   - `sidecar_availability` is *device D holds kind K variant V for content C*.
     That is a claim about a device and a source store has no device table, so
     it stays in the library, and it is the half that syncs.

   `sidecar.source_entry_id` does not come across. It records which copy
   happened to get rendered first, and it stops being true the moment that copy
   is deleted while three others remain.

   Layout follows the same move: `sources/<id>/sidecars/{h0}/{h1}/{content_uuid}/`,
   with `SidecarPathBuilder` taking a source directory where it takes a library
   path today. `sd_sidecar_path` already owns the sharding, so the change is the
   root it is given. `thumbs.pvcache` is already per-source and already the hot
   tier, so this is the durable half catching up to where the cache went.

**Order.** (1) and (2) are bugs against data the walk already holds and should
land first; every walk before then produces records that need re-walking. (2)
also has to land with P2.6, which makes the tree load-bearing rather than merely
missing. (3) is a schema declaration. (4) is behind the content identity job,
since a sidecar has nothing to key to while `content` has 0 rows.

### P2.6 — Address a file by its parent, not by its path

`external_id` is the full relative path on every record, averaging 108 bytes,
and `UNIQUE (type, external_id)` holds a second copy. On the 2.1M-record store
that is most of the file. The old library did not work this way: it stored a
path once per directory in `directory_paths` (2,980 rows for 23,157 entries)
and derived everything else from `parent_id` and `name`. That was the right
call and dropping it was a mistake.

Measured, not projected. Both databases vacuumed, the same 2,123,008 records,
the alternative built by replaying the live store through the proposed schema:

| | today | full paths on directories only |
|---|---|---|
| `record` | 401 MB | 201 MB |
| path uniqueness index | 255 MB `(type, external_id)` | 92 MB `(parent_uuid, name)` |
| primary key | 50 MB | 50 MB |
| `idx_record_type` | 27 MB | 27 MB |
| `idx_record_parent` | 18 MB | folded into the sibling index |
| `directory_path` + its indexes | — | 53 MB |
| **record side** | **751 MB** | **423 MB** |
| whole database, `facet_file` included | 896 MB | 568 MB |

443 bytes per record becomes 281. The saving is not the headline, though: the
255 MB index it removes mostly serves path lookup, while the 92 MB index that
replaces it answers *list this directory's children*, which is the query the UI
actually makes.

**Lookup stays two probes, and that is the whole point of keeping paths on
directories.** Resolving `a/b/c/d.png` is one probe into `directory_path` for
`a/b/c`, then one into `(parent_uuid, name)`. Producing a file's full path is
the same in reverse: its `parent_uuid` gives a `directory_path` row, join on the
name. No recursive walk in either direction, which is what a tree without
`directory_paths` would have cost.

**Renames get cheaper by an order of magnitude.** Renaming a directory today
rewrites `external_id` on every descendant record. Afterwards it rewrites
`directory_path` for descendant *directories* only, which is 240,129 rows
against 2,123,008 in this store, and far fewer in the ordinary case. The
watcher is the main caller and it does this on a drag in Finder.

**The tree stops being optional.** P2.5 (2) is currently a fidelity bug. Here it
becomes load-bearing, since `parent_uuid` is how a file is addressed at all. The
two land together or neither does.

Worth knowing before that lands: replaying the live store found **0 unresolved
parents**. Every file's parent directory already exists as a record, so the
existing 2.1M-record store can be backfilled in place from what is on disk. No
re-walk, which matters because a re-walk is where the surviving `created_at`
values would go.

**Shape.**

- `external_id` becomes nullable. A filesystem directory keeps its path, a
  filesystem file gets `NULL`, an adapter record is untouched, and SQLite
  permits many NULLs under a unique index so `UNIQUE (type, external_id)`
  stands as-is. Adapters address by opaque id because a Notion page has no
  tree; this is a filesystem optimisation and should read as one.
- `UNIQUE (parent_uuid, name)` is added, which is the constraint that was always
  true and never stated: two files cannot share a name in a directory.
- `directory_path (record_uuid, path)` returns, one row per directory.
- `record_overlay` keeps addressing by full path. It is the one table where a
  durable human-meaningful key earns its bytes, since an assertion has to
  survive the record it describes, and it is sparse enough that the cost does
  not signify.
- `Ledger::load` builds its in-memory path index from a directory pass followed
  by a file pass rather than from one column. Same sequential scan, same
  result.

**The cost, stated plainly.** Two addressing modes in one table, chosen by data
type. That is the price of a store that serves both a filesystem and an API with
one schema, and it is worth paying here because the filesystem is the one that
has to hold a hundred million rows.

### P2.7 — The arena is a volume index, a source is a scope over it

The requirement did not change when the library broke up. A machine needs full
local search over every file on it and a size figure the analyser can trust,
and neither of those is the same question as *what did someone choose to keep*.
The v2 design had two answers for that, volume indexing and locations. The
sources migration collapsed them into one and lost the distinction.

**What is true today.** `EphemeralIndexCache` keys an arena partition by source
id, and `SourceRegistry::resolve` routes a path to the source whose root is the
longest prefix. So a source nested inside another does not narrow anything, it
*forks*:

- paths under the inner root resolve to the inner partition
- the outer partition keeps its now-unreachable copy of them
- two snapshots, two `data.db` files, two ledgers
- **two uuids for one file**, because each ledger mints its own

A tag applied through one root is invisible through the other. That is the
second-namespace problem contract 1 of `2026-08-18-storage-consolidation.md`
exists to forbid, arriving between two filesystem sources instead of between
two data types. It is also why the Open register asks whether sources can nest
and finds two live documents answering in opposite directions.

**The line.** A partition is a property of the *drive*, not of a
registration. One drive, one arena, one snapshot, whatever is persisted from
it. A source says *persist records for paths under this root* and owns a
`data.db`. Nesting is then ordinary, because the two things being nested are no
longer the same thing.

Concretely:

| | keyed by | holds | rebuilt from |
|---|---|---|---|
| volume index | drive | every file the walk could see | a walk |
| snapshot | drive | the arena's durable copy | the arena |
| `thumbs.pvcache` | drive | decoded tiles | the sidecar or the original |
| `data.db` | source | records for one subtree | a walk, plus assertions that outlive it |

`VolumeKey` is the volume uuid where there is one, and the root itself where
there is not. Every mount the volume manager can see gets a record, network
shares included, so the second case is a fallback rather than a second way of
doing this.

**On the name.** This was called a *medium* for one afternoon, which named the
abstraction after its own exception. It is a volume index: the index of a
volume. `volumes/<id>/` on disk, beside `sources/<id>/`.

**Identity is the part that has to be right first.** Today the ledger mints
uuids and the arena is told what they are, which is what forks identity when
two ledgers cover one file. Afterwards the arena mints, once, per drive, and a
source *adopts* the identity already sitting there rather than resolving its
own. A file gets one uuid whether it is persisted by no source, one source, or
a source inside a source. Creating a source over already-mapped files becomes a
write of rows that already have their identities, which is also what makes it
fast.

A path under no source still has a uuid and still cannot carry an assertion,
because assertions live in `record_overlay` inside a source store. That falls
out correctly rather than needing a rule: choosing to keep something is what
gives it somewhere to keep things.

**What moves.**

1. `VolumeIndex` keys on `VolumeKey`. `resolve` answers *which drive*, and the
   longest-prefix walk over roots serves `store_for` instead, which is where it
   was always the right question.
2. `SourceDirs` grows a `volumes/` directory. `ephemeral.snapshot` and
   `thumbs.pvcache` move into it; `data.db` and `blocks/` stay with the source.
3. Per-source counts come from the store (`SELECT COUNT(*) FROM record`) rather
   than from an arena partition, since an index no longer belongs to one
   source. `sources.record_count` keeps its meaning and changes its writer.
4. Whole-drive figures for the analyser come from the arena, which is the only
   thing that has them.
5. A volume stops being a source. Tracking a drive maps it and snapshots it;
   it does not create a registration. The sources list becomes what a person
   chose to keep, which is what it reads as.

**Completeness, and what it costs.** Full search and honest size reporting both
need the walk to see everything, and today an internal volume walks with
`RuleToggles::default()`: `/Library`, `/Applications`, `~/Library`,
`node_modules`, `target`, caches, and anything `.gitignore`d never enter the
map. Measured on a 1 TB drive with 89 GB free, that is 262 GB indexed against
roughly 900 GB used.

Rules become what the external-drive path already calls them, view-time lenses,
and the walk records what it can read. The cost is real and worth stating: 2.1M
entries is 1.0 GB resident, so a complete map of that machine is plausibly 5 to
8M entries and 2.5 to 4 GB. `EphemeralIndex` already carries `collection_flags`,
a per-entry bitfield, so a filtered bit lets listings hide what the rules
exclude while sizes and search stay complete. That is the shape; the memory is
the decision.

**These are two changes and they land in that order.** Nesting is a correctness
bug about identity and blocks anything that persists a subtree. Rules as lenses
is a memory tradeoff and can wait for a number someone is happy with.

The second one moved to `2026-08-27-storage-map.md`, which measured it rather
than estimating: 11.1M entries and 5.0 GB for a complete map against 54 MB for a
seven-ring structure, so the answer is level of detail rather than a choice
between all and nothing.

### P3 — One registry

Teardown `T6.1`, pulled ahead of `T4`. `library.db` owns sources; `registry.db`
and `<library>/archive/` are removed; `ops/sources/*` serves both kinds. The
`ephemeral` qualifier retires here rather than at `T6.7`, because after this
phase there is nothing for it to distinguish.

**Partly done.** `sources` is a table (`entities::source`,
`m20260825_000001_create_sources`), one row per source whatever its ingest,
forked by `data_type` to match `_schema.data_type_id` in the source's own store.
`sources.json` is deleted rather than migrated. `T2.0b` is ruled: a source
carries a nullable `volume_uuid` and stores its root *relative to that volume*,
so a remount re-derives the absolute path instead of being a case anything
handles. Fingerprint matching is gone from the registry, since it duplicated an
identity the volume manager already maintains.

Still open here: folding `registry.db` in so adapter sources use the same table,
and a deliberate "track this drive" flow so a source stops appearing only as a
side effect of a browse.

Two seams this left visible on purpose. `EphemeralIndexCache` is machine-scoped
and a registration is library metadata, so the cache follows the open library
(`attach_library` / `detach_library`); that is where sources becoming reachable
from wherever they are mutated resolves it. And a volume that remounts *while
the daemon runs* does not yet move its sources: roots resolve at attach, which
covers the restart case, and `SourceRegistry::remount` is written and tested
waiting on a `VolumeEvent::VolumeMountChanged` subscriber, which nothing in the
tree has.

### P4 — Delete entries

The remaining `T6.x`, now with one target instead of two.

**Collapse `MemoryAdapter`'s two halves, and give the arena a change stream.**
`IndexPersistence` has no production caller: the ephemeral walk batches straight
into the arena from `IndexerJob::run_ephemeral_processing_static`, and only the
watcher goes through `ChangeHandler`. So the trait dies with the entries world
that needed it, and the walk and the watcher can finally share one path rather
than two that happen to call the same methods.

The shape to land while doing it: **an arena mutation produces a change record,
and the store write and the client event are both derived from it.** Today each
`ChangeHandler` method updates three destinations by hand — arena, store,
event bus — with nothing forcing them to agree, and they have already drifted
once. A move updated the arena and the store correctly and emitted nothing,
because creates emit inline while moves delegated to a generic hook that was
handed the path the file had just left. Fixed in place, but the next such bug is
a matter of time while three writes are maintained by hand at every call site.

One fan-out makes the failure unrepresentable: the store cannot know about a
move the client does not, because neither is written independently. It also
answers what a change record has to carry, which the move bug already showed —
both the destination and the vacated directory, since a client renders directory
listings and a move invalidates two of them.

Worth doing here rather than sooner. `MemoryAdapter` is being rewritten in this
phase anyway, and doing it earlier means building the seam twice.

### P5 — Catalog

Teardown `P7`, unchanged.

## Against the existing register

`P2`, `P4` and `P5` are `T2.x`, `T6.x` and `P7` as already written. `P0` and
`P1` are new. `P3` is `T6.1` moved earlier. Nothing in the teardown register is
cancelled; the ordering changes and two phases are inserted ahead of it.
