# Per-source databases and the record spine

Captured 2026-07-29. The question is what it would take to bring the Spacedrive
Native data shape into V2: one database per location, a universal record spine,
and type structure in facets rather than core columns. Reference implementation
is `~/Projects/spacedriveapp/native` (`crates/model`, `crates/facets`,
`crates/harness`, `crates/sources/*`, about 3,600 lines total), and its own
design notes in `native/docs/architecture.md`.

The short version: half the port already landed. `crates/archive` carries the
spine as of 2026-07-28. What remains is that files and photos still live in a
different world, and closing that gap is a different project from adopting the
schema.

---

## Already done

`crates/archive/src/spine.rs` (commit `da1b28e8f`) is native's
`crates/model/src/schema.rs` with different types on two columns. Both define:

- `record(uuid, external_id, type, title, created_at, modified_at, parent_uuid, content_id, version, scan_epoch)` as the universal spine, identity assigned at
  discovery rather than derived from content.
- `content(id, sampled_hash, integrity_hash, size, kind)` as the tiered async
  hash, never the primary key.
- `edge(src_uuid, dst_uuid, type, ord)` for membership and relations.
- Facet tables keyed on `record_uuid` with `ON DELETE CASCADE`, where the facet
  owns its own DDL and the core never learns what it contains.
- One disposable index per source, plus a durable layer that outlives it and
  rebinds by `(type, external_id)`.

Native's flag #3 accepted that per-source databases owe a cross-source search and
merge layer up front. V2 already has it: `search/router.rs` fans out per-source
FTS5 and LanceDB, merges with reciprocal rank fusion, and composes durable
overlays onto the hits before returning them. That was the expensive part of the
design, and it is paid for.

So there is no schema to port. There are four divergences to settle, and one
world to move.

---

## Divergences to settle

**`uuid TEXT` against native's `BLOB`.** 36 bytes instead of 16, on the primary
key, in every index, and on both sides of every edge. Cheap to change while the
only rows are 11 adapters' worth of email and notes. Expensive once a filesystem
source has put millions of records on it.

**`UNIQUE(type, external_id)` against native's `UNIQUE(external_id)`.** The
archive scopes the stable key by type, native scopes it by source alone. Whichever
wins has to match the durable layer's rebind key.

**`scan_epoch` semantics conflict.** The archive stamps it as provenance and
deliberately never sweeps, because script adapters are cursor-resumed delta
streams and a sweep after an incremental run would delete every record that run
did not touch. Native sweeps on it, because a filesystem scan is authoritative
about what still exists. Both are right for their sources, so this becomes a
per-source policy on the registry row rather than a property of the crate.

**Two overlay layers.** The archive has `record_overlay` in registry.db. Core has
`user_metadata` in library.db, already keyed on `entry_uuid` and
`content_identity_uuid` rather than integers, which makes it closer to the target
than it looks. `space_items` moved to `entry_uuid` in
`m20250103_000001_migrate_space_item_entry_id_to_uuid`. These converge or the
same fact ends up asserted in two places.

---

## What has not moved

`entries` is one global tree in library.db with integer primary keys, and a
Location is itself an Entry (`location.entry_id` references `entries.id`). 61
files under `core/src` reference the entry entity, 33 reference location, and 210
of 794 touch SeaORM.

Four things block splitting that table per-location.

**Integer joins.** `entry_closure(ancestor_id, descendant_id)`,
`directory_paths(entry_id)`, `collection_entry(entry_id)` and `entry.parent_id`
all assume one database. Most of this is benign, because a location is a subtree
and closure and path caching are intra-location by nature. They move into the
index alongside the records they describe.

**FTS5.** `search_index` is an external-content table over `content='entries'`
driven by insert, update and delete triggers
(`m20250120_000001_create_fts5_search_index.rs`). External content follows exactly
one table in one database, so the split ends it. The replacement is the archive
router, which means this is a migration onto something that exists rather than
new construction.

**Sync.** `Entry` implements `Syncable` with device-owned and shared rows, and
there are 7,256 lines in `core/src/infra/sync` plus 8,315 in `core/src/service/sync`
built around a single shared library.db. Native dropped sync entirely for v1 and
proposed a three-layer replacement (one-way replication for single-writer
disposable indexes, LWW merge for the durable library, iroh-blobs for bytes) that
was never built. This is the only piece with no implementation in either repo,
and it is the reason migrating `entries` is the wrong place to start.

**Ingest throughput.** The archive's only ingest path is the JSONL script adapter
protocol, which suits Gmail and cannot carry a filesystem walk at 8,500 files per
second. A filesystem source needs a Rust connector writing batched transactions
straight into a `SourceDb`. Native has that seam already: `crates/harness` is 280
lines of `Connector`, `Sink` and epoch handling, and `sources/filesystem` is 434.

---

## Recommended path

Add filesystem as a data type on the existing spine and run it beside `entries`.
Do not migrate `entries`.

This keeps sync, the FTS triggers and all 122 registered ops untouched, including
the 54 in files, locations, indexing, spaces, tags, search and media that read the
entry world. It also turns the question of retiring `entries` into a measurement
against a working alternative instead of a bet taken up front.

**Phase 1. Fix the spine types.** Move `record.uuid` to BLOB, settle the
uniqueness key, and add the sweep policy column to the registry. Reindex the 11
adapters, which is what they are for.

**Phase 2. Add a non-script ingest seam.** Lift native's `harness` into
`crates/archive` as a `Connector` trait and batching `Sink` over a `SourceDb`
transaction. The `Engine` currently assumes `AdapterRegistry` is the only way in.

**Phase 3. Port the file facet and a filesystem connector.** `facets::file` is
140 lines and `sources/filesystem` is 434. Feed it from V2's existing ephemeral
indexer walk rather than native's directory walker, so the arena, string interning
and throughput carry over.

**Phase 4. Add a global content map.** A table in library.db from content hash to
`(source_id, external_id)`, written when a hash tier completes. Without it,
per-source `content` tables mean cross-source dedup cannot be answered.

**Phase 5. Point one console view at it.** Run the same folder through both paths
and compare listing latency, scan time, memory and search results before deciding
anything about `entries`.

---

## Costs being accepted

Per-source databases are worth it for what they make possible: deleting a source
is deleting a file, rescanning one location never touches its neighbours, a source
can carry its own schema, and a large scan stops writing into the same database
every read is served from. That last one is the current pain, not a theoretical
benefit.

The bill:

- **Global queries stop being SQL.** Everything sorted by size across twelve
  locations is an N-way merge in application code. The router does this for ranked
  search, not for arbitrary sorted pagination over a whole library.
- **Dedup needs the content map above.** Today a shared `content_identity` answers
  "where else is this file" in one query, for free.
- **No cross-database transactions.** A move between locations is currently one
  transaction. `ATTACH` covers part of this, with a default limit of 10 databases
  and a compile-time maximum of 125.
- **N pools, N handles, N journals.** Twelve locations is nothing. Two hundred
  needs an LRU over pools before the open-everything-to-answer-a-query pattern
  becomes the bottleneck.

---

## Open

- Whether `entries` eventually retires or files stay on their own storage
  permanently. Phase 5 informs this. Nothing before it should assume an answer.
- How the two overlay layers merge, and which key wins.
- Whether the ephemeral indexer writes through the connector seam or keeps its
  own snapshot path and folds into the index behind it.
- Sync across per-source databases. Deferred, and it stays deferred as long as
  files remain on `entries`.

---

## Addendum (2026-08-17)

The recommended path above — add filesystem as a data type beside `entries`,
migrate nothing, measure — rested on sync as the immovable blocker ("the reason
migrating `entries` is the wrong place to start"). That premise was retired by
decision on 2026-08-17: all sync work is stopped, and local-only is the product.

The strategy is now **replacement under contract**: ops keep their signatures,
locations migrate one at a time onto per-location spine stores, and the
`entries` schema is deleted when the last location leaves. The divergences and
costs cataloged above remain accurate and are absorbed as prerequisites — the
global content map (phase 4 here) is designed in
`docs/core/design/cross-location-content.md` and moves first, seeded from
`content_identity` so cross-location queries never go dark; the overlay-layer
merge and the sorted-enumeration bill land as prerequisites P2 and P3.
Execution: `docs/plans/2026-08-17-record-spine-transplant.md`.
