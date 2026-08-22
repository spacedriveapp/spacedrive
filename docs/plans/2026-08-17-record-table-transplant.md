# Record Table Transplant — Phase 1

**Strategy revision (2026-08-17, superseding the earlier draft of this plan):** the earlier draft followed `docs/plans/2026-07-29-per-source-databases.md` — run the record table beside `entries`, migrate nothing, measure, then decide. That doc's recommendation rested on sync as the immovable blocker, and the sync freeze retired the premise. The strategy is now **replacement under contract**: Spacedrive works the same, on per-location databases. Ops keep their signatures; locations migrate one at a time onto the record schema; the `entries` schema is deleted when the last location leaves. See the addendum on the July doc.

**Amendment (2026-08-18):** `docs/plans/2026-08-18-storage-consolidation.md` collapses this plan's storage model (per-source `index.db`+`meta.db` → one `source.db` with a durable/rebuildable logical split; `content-map.db`+`global-index.db` → one derived `catalog.db`), restructures the drain (the connector fans out to arena and store — the arena is a cache, not the event source), and states the four contracts (source ownership, identity/rebinding, drain sequencing, cutover state machine) that gate slice 2 wiring. Inline notes below mark the superseded passages; the workstream structure and everything else stands.

Three commitments define the strategy:

1. **Parity is defined at the ops boundary, not the SQL boundary.** All registered ops keep their input/output contracts — every client (UI, CLI, API, agents) speaks ops, so that is where "works the same today" is measured. Golden tests capture op outputs on a seeded library before migration and replay them after.
2. **Locations migrate to the record schema directly** — per-location `index.db` with records + file facet — not to per-location copies of the entries schema. The expensive work is the write-path re-plumbing and the cross-location merge layer; paying it twice to preserve a schema scheduled for deletion would be waste.
3. **Granularity is per-location, and that is the entire risk story.** A location lives either in `library.db` or in its own store; reads dispatch by residence. Migrate one location, run the parity suite, migrate the next. Nothing big-bang.

## Where things stand

- **The record schema landed 2026-07-28** (`da1b28e8f`): `crates/archive/src/record.rs` — records with assigned uuid identity, tiered async content hashing, typed edges, cascade-owned facets, disposable per-source index + durable rebinding layer.
- **Cross-source *ranked* search is paid for**: `search/router.rs` fans out per-source FTS5 + LanceDB, merges via RRF. Sorted/paginated enumeration across stores is not built — it is prerequisite P3 below.
- **imageio ported 2026-07-30** (`9a95fbfda`) as `sd-imageio`, macOS-gated, alongside the platform-derivatives rework of `core/src/ops/media/thumbnail/generator.rs`.
- Still only in native: `pvcache`, `bake` tiers, `harness` (Connector/Sink seam), `facets/file`, `sources/filesystem`.
- The `entries` world: one global tree, integer PKs, 87 files touching the library DB, 76 referencing `Entry`, FTS5 triggers, `content_identity`, and the sync machinery (now frozen by decision, no longer load-bearing for this plan).
- The ephemeral index: fast, arena-based, already serving Explorer listings, search, and whole-volume indexing — but globally singular, with no per-source partitioning, no lifecycle wiring, no staleness metadata, and silently lossy watcher overflow (audit in `2026-08-17-SPACEDRIVE-LIVING-INDEX.md` § Design gaps).
- **Commit the untracked docs**: `docs/plans/2026-07-29-per-source-databases.md`, `docs/core/design/cross-location-content.md`, `docs/core/design/thumbnail-storage.md`.

## Pre-flight findings (2026-08-17 codebase sweep)

Five targeted sweeps (ops-contract leakage, sync severability, archive fitness, ephemeral/watcher/volumes, cross-location SQL) before any work begins. Full detail lives in the sweep outputs; this is the register the workstreams must absorb.

### Fix regardless of migration (data safety)

1. **Mass deletion on unmount.** A volume unmounting can flood Remove events, and `PersistentEventHandler` deletes DB rows with no volume-online or `path.exists()` guard. Add the guard before anything else ships.
2. **Resumable indexer jobs persist integer entry ids** (`indexing/state.rs:118-127`, rmp-serialized whole-job). A job interrupted pre-migration resumes post-migration on dangling/reused PKs — silent corruption, not failure. Invalidate persisted indexer job state at migration time (all three fields rebuild from paths).
3. **Ephemeral snapshots are globally destructive**: restore for path A replaces every other tree's entries (`cache.rs:143`), every snapshot file contains the whole index, and files are keyed by `DefaultHasher` of the canonical path — unstable across Rust releases and across `/Volumes/Name 1` remounts. Workstream B move 3 re-keys by source id and bumps `SNAPSHOT_VERSION`; old snapshots are deleted, not migrated.

### Blockers found in the archive stack (absorb into P4/B5)

- **`adapter.toml` is mandatory** — `Engine::create_source` hard-errors without a script-adapter manifest, so a Rust connector cannot create a source. Fix: `Adapter::schema()` on the trait, drop the file check.
- **Screening costs ~3M autocommit statements per 1M files even when "skipped"** (`mark_all_unscreened_safe` walks records one UPDATE at a time, plus per-record FTS refresh). Needs a bulk-UPDATE fast path and a per-source `screening_enabled` flag. Related: unscreened records are FTS-invisible, so watcher-created files wouldn't appear in search until a screening pass — wrong default for filesystem sources.
- **No source-pool cache**: every `open()` builds a fresh `SqlitePool` and replays full DDL; the search router opens every source per query. 200 sources = 200 pool creations per search. Needs a pooled/LRU source-handle cache before the fleet grows past a handful.
- **Ingest is 5–7 autocommit round-trips per file, no transaction** — the Connector seam (B5) must land a batched transactional write API, or filesystem throughput dies on arrival.
- **Epoch sweep is documented as unimplemented** — exactly the primitive rescans need; P4's sweep-policy work builds it, not just the column.
- FTS creation only indexes the schema's primary model — a `file` + `directory` schema loses one. Registry pool lacks `foreign_keys(true)`/`busy_timeout` (silent FK no-ops).
- Reassuring: LanceDB, FastEmbed, and Prompt Guard are all stubs today — their per-source eager-open and embed-everything costs are latent, catchable at wiring time (add per-source embedding opt-out then).

### The entries couplings that bite the cutover (absorb into C)

- **`locations.entry_id` → global entries is the deepest single-DB assumption**: rescan, enable-indexing, trigger-job, and export all error if the root entry leaves `library.db`. Becomes "root of this location's store" (implicit) during residence dispatch.
- Location create does a device lookup purely to emit a sync StateChange and `?`-propagates failure on the critical path (`location/mod.rs:290-297`).
- Entry deletes emit UI events only through the sync call path — per-location deletes need explicit `ResourceManager` emission.
- FTS5 triggers live on `entries`; per-location stores recreate them or search silently dies for migrated locations.
- `sidecar.source_entry_id: i32` becomes a cross-DB FK — move to uuid or drop (consumers select by kind, never id).
- **Sync is otherwise severable** — no log rows or triggers on entry writes, backfill peer-gated, P2P degrades to zero. But `LibraryConfig::sync_enabled` exists, defaults false, and *nothing reads it*: nine tasks spawn per library regardless. Honoring it at the two `init_sync_service` call sites is the whole off-switch.

### Ops-contract leaks (fix before golden tests freeze the contract)

The boundary is otherwise uuid-clean — `File`, locations ops, events, frontend selection/DnD all key on uuids. Delete now, free breaking changes: `tags/apply` output `tagged_entry_ids: Vec<i32>` (no consumers), `TagTargets::Entry(Vec<i32>)` / `ApplyToTargets::Entry(Vec<i32>)` input variants (frontend already sends uuids), `Sidecar.id: i32` on the `File` object (consumers select by kind).

### Cross-location SQL (scopes P1/P3)

27 query sites inventoried. Every *hard* one reduces to content-placement cardinality — redundancy summary, at-risk/on-volumes filters, `unique_to_location` (the only closure query that crosses locations, via `ancestor_id != ?`), per-volume unique bytes — all served by the content map, confirming P1 as the keystone. Also: **no cross-location transaction exists to preserve** (copy is a job; there is no move op), `SearchScope::Location` is a no-op TODO (today's "location search" is secretly global — the parity bar is the floor), `get_total_count` loads all rows into Rust (replace with per-shard counts), and facet counts are already computed app-side (merge-friendly).

**Decision needed before C:** search's public contract bakes in `offset`/`current_page`, but cross-shard merge wants keyset pagination. Either the merge layer emulates offset (bounded ugliness, contract preserved) or the search op versions (`search.query` v2 with cursors, v1 emulated on top). Decide, don't discover.

**Decision needed in P2:** tags resolve through entry uuids *and* content-identity uuids. Record-attached tags → per-source `meta.db`; content-attached tags are cross-source by nature → library-level assertion, projected via the content map.

### Volume identity and on-drive storage (new task, C-adjacent)

- **External volume fingerprints hash in `device_id`** (`domain/volume.rs:53-59`) — the same drive fingerprints *differently on another machine*, which breaks the plug-into-any-Spacedrive story outright. Drop device identity from the fingerprint; the `.spacedrive-volume-id` dotfile is the right anchor, with a filesystem-UUID/serial fallback for volumes where the dotfile can't be written (read-only NTFS, full exFAT — the write already fails silently at `utils.rs:245`).
- **Location paths are absolute strings, never rebased through `volume_id`** — a remount at `/Volumes/Name 1` orphans every location on the volume. Per-source stores key by source id and rebase paths through the volume at open.
- **WAL is forced unconditionally on every pool** (core and archive). WAL needs `-shm` shared memory — unreliable-to-corrupting on exFAT/SMB/NFS. On-drive stores need per-path journal policy (`DELETE`/`TRUNCATE`, `mmap_size=0`, longer busy_timeout) and a checkpoint-on-unmount hook (archive source DBs currently have none — a surprise unplug strands a `-wal`).

### Watcher gaps (absorb into B4)

No overflow/rescan path of any kind: unhandled `notify` event kinds are silently dropped, buffer overflow logs and drops, nothing maps to FSEvents' rescan flags — the bounded bankruptcy-recrawl in B4 is new construction, not a tweak. Mount/unmount is detected (`VolumeMountChanged`) but wired to nothing — no watch re-registration on mount, no cleanup on unmount. Routing picks the *first* prefix-matching config (wrong winner with nested watches); the ephemeral handler requires `parent == watched` exactly (recursive watches drop grandchildren); the default dotfile filter makes `.spacedrive/` events invisible; and `should_skip` matches `.tmp` as a substring of the whole path — a source under any folder containing `.tmp` is entirely muted.

### Ephemeral identity (absorb into B3)

Frontend-visible uuids are reminted on rescan and snapshot-miss (v4 minted before the dedupe check, discarded on duplicate; `get_or_assign_uuid` returns throwaway randoms for unknown paths), the "ephemeral uuids preserved on promotion" claim in docs and TS bindings is implemented by nothing, synthetic `AtomicI32` ids collide across adapters and restarts, and `get_path_by_uuid` is a linear scan on the `file_by_id` path. Stable v7 identity is safe to adopt — nothing anywhere branches on uuid version — and makes the documented promotion behavior true for the first time. Search routing defaults unknown paths to Ephemeral; per-source routing replaces that guess with a registry lookup.

## Workstream A — Prerequisites (before any location migrates)

Ordered; each is load-bearing for the cutover and none is optional.

**P1. Global content map, seeded from `content_identity`.** Design: `docs/core/design/cross-location-content.md`. The map is its own database file (`projections/content-map.db`) — never a `library.db` table, so hash-completion writes contend with nothing and the file stays deletable/rebuildable. Seeded from the existing `content_identity` table *first*, so cross-location dedup and "where else" never go dark during migration. Acceptance: map populated for the current library; "content in A not in B" answers correctly against two existing locations; hash-completion events upsert; deleting the map file and rebuilding from source stores reproduces it.

**P2. Durable knowledge split.** `user_metadata` and the archive's `record_overlay` converge (the July doc flagged that they must, or the same fact gets asserted twice) — but the destination is **per-source `meta.db`**, not a central overlay: tags applied, corrections, and notes are knowledge about a source's files and travel with the source (on-drive when the source is). `library.db` retains only library-scoped data: source registry, settings, tag/space *definitions* (denormalized into meta rows for portability), and genuinely cross-source assertions such as Spaces spanning multiple sources. Acceptance: tagging survives index.db deletion and rescan; a source's meta.db relocates with the source; cross-source Space membership survives a member source detaching.

*Amended 2026-08-18: the destination is durable tables inside one per-source `source.db`, not a separate `meta.db` file — reindex replaces the rebuildable generation and never touches the durable tables, so the acceptance criteria hold inside one transaction boundary. The identity ledger (consolidation doc, contract 2) lives in the same durable set. Acceptance unchanged, restated: tagging survives a rebuildable-generation replacement.*

**P3. Cross-location query strategy — no N-way SQL merge on any hot path.** At 100+ sources, per-query fan-out over SQLite stores is disqualified (the sweep already caught the router opening every store per search). Tiered instead:

- *Scoped queries* (a folder, a source) hit one store — free.
- *Global interactive views* iterate the loaded **arenas** — in-memory merge, no SQLite; the hot tier is the query tier.
- *Global search, detached drives included*, is served by a **global enumeration projection** (`projections/global-index.db`: one row per record — source, uuid, name, size, mtime, kind — plus global name FTS), maintained from drain/watcher events, rebuildable by sweeping source stores, deletable like every projection. This is the rant's "combined search database, integrated in the other direction," and it absorbs the path-fragment search residual (indexed ancestor terms) and makes keyset pagination trivial over one index.
- *Spaces spanning a few sources* compose small-K; the only many-store sweep that survives is the projection rebuild, a background job.

Acceptance: global name search answers from the projection with detached sources included and correctly badged; listing parity on identical data split across three stores via the scoped and arena paths; projection deleted and rebuilt reproduces itself.

*Amended 2026-08-18: `global-index.db` and P1's `content-map.db` merge into one derived `catalog.db` (enumeration, detached display, global FTS, placement rows as tables). Splitting for write contention is deferred until measured. A detached source's catalog rows are availability-bearing: the previous-good generation is retained until a replacement publishes, and destructive decisions never rest on projection rows (consolidation doc, artifact classes).*

**P4. Record type fixes.** `record.uuid` TEXT → BLOB, uniqueness key settled against the durable rebind key, per-source sweep-policy column on the registry. Done now while the record table holds 11 adapters' worth of rows; reindex them. Acceptance: BLOB uuids throughout, adapters reindex clean.

## Workstream B — Indexer restructure: from mode flag to per-source sinks

The bridge between the walk and the stores. The seam exists — `IndexPersistence` (`core/src/ops/indexing/persistence.rs`) with `DatabaseAdapter` (entries) and `MemoryAdapter` (arena) — but it is a binary mode (`Persistent | Ephemeral`, branched ~10 times in `job.rs`), speaks entries vocabulary (`i32` ids, `location_id`), and carries no source identity. Five moves, in order:

1. **Source identity first-class.** Registering a root creates a source row (reusing the archive sources registry) with `source_id`, root path, placement, sweep policy. `IndexerJob` carries `source_id`.
2. **Sink composition replaces the mode enum — as a pipeline, not a fan-out.** The ephemeral sink (the source's arena partition) is mandatory and first: the walk and watcher write only to it. The record-store sink (the source's `index.db`, through the Connector/Sink seam lifted from native's `harness`) *drains from the arena* — bulk behind the walk front, then incrementally from absorbed watcher deltas — for subtrees marked durable. Double indexing is structurally impossible, and the arena entry's uuid v7 is the record uuid the record table persists (identity continuous from first sight to durable record; see `docs/core/design/zero-onboarding-startup.md`). `Entries` (legacy) remains the read/write path for not-yet-migrated locations only. Branches collapse into sink capabilities; the trait generalizes to `(source_id, external_id)` keys.

   *Amended 2026-08-18: the durable sink does not read the arena's memory. The connector produces one observation stream and fans out to both consumers — arena for display, `source.db` via transactional `apply_mutations` batches with generation/watermark checkpointing (consolidation doc, contract 3). The two invariants this move existed for are unchanged and now explicit: one walk ever, and the record uuid minted at first sight before fan-out. "Drains from the arena" describes the *bulk* case only, where the arena replays its contents as the initial batch set.*
3. **Partition the ephemeral cache per source.** `EphemeralIndexCache` becomes `source_id → EphemeralIndex`; snapshots become genuinely per-source dumps. Fixes three of the four audit landmines; prerequisite for the analyzer and for on-drive placement.
4. **Watcher routing by source.** Events resolve to `source_id` by root prefix, dispatch to that source's sinks; overflow recovery becomes a bounded bankruptcy-recrawl of one source.
5. **File facet + filesystem connector.** The walk writes through the record table sink into the source's `index.db` at existing-indexer throughput.

Caution preserved from the earlier draft: do not sweep every `is_ephemeral` branch in one pass — several are progress/reporting concerns, not sink concerns. Land moves 1 and 3 early (the analyzer needs exactly those), and let the enum die when its last branch loses its reason.

### What a Location becomes

**A Location = a subtree + a durability flag + an enrichment policy.** The drain
pipeline gives each part a home: durability marks the subtree for the record table
sink; the policy decides which enrichment tiers follow the drain (hash tiers,
thumbnails/sidecars, media metadata, OCR, embeddings). Policy rows live in the
source's `meta.db` — assertions about a subtree that travel with the drive —
with library-level defaults for the startup profile. Policy changes are
retroactive drains over records the arena already holds, never rescans. This
graduates the existing per-location processor config from a bolted-on flag to
the definition; whether Location survives as a distinct UX object or folds into
the sources UI is slice 2's first product decision.

**Decided 2026-08-19 (James): Location does not survive as a UX object.** The
sidebar's Locations group is retired (seed removed, existing libraries
converge by deterministic-uuid delete), and the locations table goes with the
entries tear-out. What survives is the function under its real name: policy
rows on source subtrees. Teardown dependencies to migrate before the table
can drop: the watcher's boot-time watched-roots load, thumbnail/media job
discovery (location→entry joins), `directory_listing`'s index-mode routing,
the file-transfer `is_path_allowed` gate (must re-anchor on sources, as the
byterange protocol already does), and location rows in sync.

### The closure table does not port

`entry_closure`'s jobs are individually replaced: `aggregate_size` → the
incremental arena rollups; subtree scoping → the per-source store *is* the
scope; the one cross-location closure query (`unique_to_location`) → the
content map. **`directory_paths` does not port either** — its two purposes
both dissolve: path-string dedup has nothing to dedupe when truth is
`parent_uuid` + `name` (the hot layer already interns names and derives
paths), and SQL path operations are replaced structurally — full paths
assembled at query time by parent-chain walks memoized *per directory, not
per row*; subtree queries as recursive CTEs on the indexed parent column,
which also retires the `dp.path LIKE` string-prefix bug class (nested-root
leakage). With no stored paths anywhere, a directory move touches one row:
the closure/paths move-surgery is not replaced by something cheaper, it is
replaced by nothing. Residual for the search design: path-fragment queries
("projects/logo") rode on stored path strings and need indexed ancestor
terms or query-time chain filtering. No surviving consumer needs closure's
both-direction arbitrary-depth joins.

## Workstream C — Per-location cutover

The migration itself, once A and B are in place:

1. **Golden parity suite first.** Seed a test library; record outputs of the file/location/search/tag/space ops that read the entry world (the July doc counts 54). These recordings are the contract.
2. **Residence dispatch.** A location is marked `entries` or `record`; directory listing, search enumeration, and file ops dispatch by residence. The merge layer (P3) composes mixed-residence results during the transition.
3. **Migrate location-by-location.** Cut a location over (re-index into its `index.db`, durable layer rebinds by `(source_id, external_id)`, map rows update), run the parity suite scoped to it, proceed. Old locations remain untouched in `library.db` until their turn.
4. **Retire.** When the last location leaves: FTS5 triggers and the entries tables drop, `content_identity` drops (the map long since took over), `DatabaseAdapter` deletes, and the entries branch of residence dispatch goes with it.

Acceptance for the phase overall: a library with locations in both residences passes the full parity suite; a fully-migrated library passes it with the entries tables gone.

## Workstream D — Media pipeline (hot tier)

Design: `docs/core/design/thumbnail-storage.md`, which supersedes the earlier "two-tier bake" framing — native's `_g/_d` deriver turned out to be deleted code, and spacedrive's sidecar system already *is* the durable WebP tier (content-addressed, variant-based, HTTP-served). What ports is the hot tier: `pvcache` as `sd-pvcache` (uuid-keyed slots instead of feed-order, capacity doubling, mandatory per-slot versions), `bake`'s producer/worker model behind a platform-first producer chain (sidecar decode → sd-imageio/QuickLook → optional ffmpeg → type tile), one cache per source living in the source's store. This also turns thumbnails on for ephemeral browsing, which currently has none. Acceptance: a source's `thumbs.pvcache` fills lazily on browse and survives restart; an ephemeral (un-indexed) directory shows real thumbnails; a default build without the `ffmpeg` feature degrades video to type tiles instead of erroring; macOS-gated, Linux builds green.

## Workstream E — Ops surface and CLI

**Record ops** surface through the existing `ops/sources` machinery where it fits; minimum new surface: register a filesystem source, run its connector, query records with facets. Do not invent a parallel namespace where `sources.*` already carries it.

**The `sd op` passthrough.** Transport is already generic (`DaemonRequest::Action { method, library_id, payload }` / `::Query`, `crates/sd-client`, dispatched by name over the four inventory maps in `core/src/infra/wire/registry.rs`):

```sh
sd op --list                                      # all registered ops, grouped by kind
sd op tags.create --json '{"name": "invoices"}'
cat batch.json | sd op files.move --stdin
```

- `registry.list` is a core *query* (name, kind, scope), registered through the normal ops path so web UI, Spacebot workers, and Console scripts share the same enumeration.
- `sd op <name>` resolves kind/scope via `registry.list`, builds the matching `DaemonRequest`, sends via `send_raw_request`; current-library resolution as `execute_action!`, `--library` overrides.
- Payload via `--json`/`--stdin` (default `{}`); no client-side validation — the registry's serde error is the correct error surface. JSON to stdout (pretty on TTY), errors to stderr, non-zero exit.
- Surface: one `Commands::Op` variant + `apps/cli/src/domains/op/`, plus the `registry.list` query. No daemon protocol changes.

`sd op` doubles as migration tooling: the golden parity suite drives ops through it.

## Workstream F — Ephemeral rollups (analyzer prerequisite, parallel)

Incremental directory-size rollups in the ephemeral index: per-file size exists in `PackedMetadata`; nothing sums it. Rollups recompute along the ancestor chain on watcher events — never by re-walking — and track logical and allocated size separately with the hardlink/clone attribution policy anchored in the content map (see `cross-location-content.md` § Attribution). Acceptance: subtree sizes queryable for any node; a file change updates ancestor totals without a re-walk; totals match a fresh scan on a test tree.

## Workstream G — The record table rule

Add to the repo root CLAUDE.md, verbatim:

> **Record table rule.** New surfaces, ops, and features build on the record table (`crates/archive` — records, facets, edges, per-source stores). The `entries` world is being replaced under contract: ops keep their signatures while locations migrate to per-location stores, and the entries schema is deleted when the last location leaves. Do not add features, ops, or schema to the entries world; do not deepen the single-database assumption anywhere. If a task appears to require either, it is actually a record-table migration task — stop and flag it.

## Execution slicing: archive-a-drive first

The first deliverable is the drive-archiving workflow (index an external drive → unplug → still browse), sliced through the workstreams so real use starts before the migration finishes. Nothing here is throwaway — slice 1 *is* B1+B3 with the volume-identity fix, sequenced use-case-first.

**Slice 1 — catalog MVP.** Task 0's unmount guard; B1 source identity with the machine-independent volume fingerprint; B3 per-source arenas, stable v7 uuids, source-id-keyed snapshots (version bump deletes old global dumps); attached/detached lifecycle with restore-on-demand read-only browsing for detached sources; archive walk profile (all rule toggles off — the config field exists); sources in the sidebar with attachment badges. Acceptance: plug in → index → quit → unplug → relaunch → browse the full tree with sizes from the snapshot.

*Status 2026-08-17: core landed (`e4264077b` unmount guard, `963d026d1` portable fingerprints with legacy row migration, `e872b40e2` per-source partitions + detached restore + rules-off archive walks, `80da050e3` sources in `core.ephemeral_status`); 319 lib tests green including a register→index→snapshot→unplug→restore round-trip. In flight: the sidebar sources section.*

*Status 2026-08-18, after a night of real archival use: live event streaming fixed (`7c9d1b7db` — the subscription filter compared device slugs by string equality, dropping every event for "local"-scoped subscriptions); persistent counts on overview volume cards (`13dd6ec39`, `ed90a9e3e`); **snapshot durability hardened against three corruption classes found in use** — concurrent-tmp interleaving and unrestored-partition clobbering (`3aa4867d4`: unique temp names, per-source save lock, restore gate, save invariant) and reboot truncation from missing fsync (`f85d2582a`); **workstream F's rollups landed** (`25e781c29`: incremental `subtree_bytes`, recomputed on restore, surfaced as directory sizes and per-source totals — logical size only; allocated/clone attribution stays with the content map). Known follow-ups: wire `VolumeMountChanged` to `set_detached`; shutdown save hook for post-index watcher deltas; multi-partition search fan-out; duplicate browse-job dispatch race (harmless to data, wasteful); task 0's remaining items (honor `sync_enabled`, delete the i32 tag-op variants); `sd-cli` looks for logs in the wrong directory and prints directories as `Type: File`.*

**Slice 2 — durable + searchable.** Archive-stack blockers (adapter.toml removal, batched transactional ingest, minimal pool cache); the drain into each source's `index.db`; per-source FTS (detached drives become full-text searchable); content map seeded (redundancy: "is this drive backed up anywhere" starts answering).

**Slice 3 — travels with the drive.** On-drive `.spacedrive/` placement with per-path journal policy; `meta.db`; pvcache for detached thumbnails.

The remaining workstreams (merge layer, golden suite, per-location cutover) proceed in parallel and are not gated by, nor gates of, these slices.

## Task order

0. Data-safety fixes from the pre-flight register: unmount deletion guard, ops-contract i32 deletions, honor `sync_enabled`. *(immediate, independent of everything)*
1. Commit the untracked docs. *(immediate)*
2. P4 record type fixes → P1 content map (seeded) → P2 durable re-keying → P3 merge layer. *(ordered; P1 may start alongside P4)*
3. Workstream B moves 1–3 (source identity, sink composition, per-source arena). *(parallel with 2 after P4; move 3 should not trail — the analyzer needs it)*
4. B moves 4–5 (watcher routing, filesystem connector). *(after 2 and 3)*
5. C golden suite → residence dispatch → first location cutover. *(after 2–4)*
6. D, E, F run in parallel with 2–4 (E's `registry.list` early — the golden suite uses it).
7. G lands with task 3; the foundation doc's phase-1 row is marked landed when C's first location passes parity.

## Out of scope

The analyzer view itself (parallel track on ephemeral — workstream F is its only dependency here). On-drive `.spacedrive/` placement (the cutover machinery makes it a path decision; foundation doc phase 3 owns the semantics). Snapshots-as-a-feature (living-index doc; rides per-source partitioning). Apple Photos/Notes connectors (photo surface). The iced/wgpu grid extraction and SpaceUI contract (later phase). Sync (frozen by decision). Full-fleet migration of every existing library — this phase proves the cutover on real locations; the mass migration is phase 2's opening act.
