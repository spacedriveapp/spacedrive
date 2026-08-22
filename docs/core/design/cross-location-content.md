# Cross-Location Content Identity

> **Status:** Design for the global content map — a prerequisite of the per-location migration
> **Captured:** 2026-08-17
> **Companions:** `docs/plans/2026-08-17-record-table-transplant.md` (execution), `docs/plans/2026-07-29-per-source-databases.md` (analysis this builds on)

## The problem

Per-location databases break the one query the monolith gave us for free. Today a shared `content_identity` table in `library.db` answers "where else does this file exist" with a single join. Once each location owns its own store, that fact is scattered across N independent databases — some of which belong to drives that are not plugged in. The redundancy view ("what on this drive is not also on that one"), dedup checks before copies, and "is this backed up anywhere" all depend on answering it anyway, cheaply, including for offline sources.

## The principle: projection, never authority

Each location's `content` table remains authoritative for its own records. The **global content map** (`projections/content-map.db`) is a derived, rebuildable index over all of them. If it is lost or corrupted, it is rebuilt by sweeping the per-location stores. Because it is never the source of truth, its consistency model can be relaxed: rows are appended by batched, fire-and-forget writes on hash-completion events, staleness is tolerated, and repair is a sweep — not a recovery procedure.

This mirrors the architecture's general rule: the parts are authoritative, combined views are derived.

## Identity model

Record identity and content identity are different things and never collapse:

- **Record identity is assigned** — uuid v7 at discovery, before any bytes are read. A record exists the moment the walk sees it.
- **Content identity is discovered, in tiers**, asynchronously and off the indexing critical path:
  - `(size, sampled_hash)` — **candidate** sameness. Bounded reads per file, computable near walk speed. Same pair means *probably identical*.
  - `integrity_hash` (full BLAKE3) — **confirmed** sameness.

Views speak this vocabulary honestly: "17 files verified identical, 240 probably identical (pending verification)." No view pretends a sampled match is proof.

## The map is a file, not a library.db table

The map lives in its own database — `projections/content-map.db` — never inside `library.db`. Two reasons this is load-bearing, not cosmetic: the map grows with *file count* (millions of rows across a fleet), and putting it in `library.db` rebuilds the monolith through the cache door; and hash-completion upserts arrive *during indexing*, so a shared file would recreate the write-contention that per-source stores exist to eliminate. As its own file it has its own WAL, contends with nothing, and is deletable — the correct property for something rebuildable. The general rule: **a cache is a file you can delete, and a file you can delete does not belong inside the database you can't.**

Schema, keyed by the rebind key so re-added sources re-link instead of duplicating:

```sql
content_map (
  source_id      BLOB    NOT NULL,
  external_id    BLOB    NOT NULL,
  size           INTEGER NOT NULL,
  sampled_hash   BLOB    NOT NULL,
  integrity_hash BLOB,                -- NULL until verified
  last_seen      INTEGER NOT NULL,
  PRIMARY KEY (source_id, external_id)
);
-- indexes: (size, sampled_hash), (integrity_hash)
```

Write path: when a location's hasher completes a tier for a record, it emits an event; a small maintainer batches map upserts. Nothing in the read or indexing path ever blocks on the map.

Whether a source is *present* is not a per-row concern — the source registry row carries attached/detached state, and map queries join against it when presence matters.

## Query patterns

All the cross-location questions become single indexed queries against the map:

- **Where else does this content exist** — rows matching the hash, any source.
- **Content in A not in B** — hashes with a row for source A and no row for source B. This *is* the redundancy view.
- **Dedup before copy** — does the destination source already hold this hash.
- **Duplicate sets for the analyzer** — group by `(size, sampled_hash)`, refine by `integrity_hash`.

## Offline sources are the feature

Map rows for a **detached** source are retained, with `last_seen` — that is exactly what makes "is this backed up anywhere" answerable for a drive in a drawer. Rows are pruned only when a source is **deleted** (an explicit user act, distinct from unplugging). Rendering results for offline sources uses the durable layer's cached display data (title, path) keyed by the same `(source_id, external_id)` — no per-source database is opened.

## Demand-driven verification

The integrity-hash queue is prioritized by what views are looking at: opening a comparison between drive A and folder B promotes that comparison's candidate pairs to the front. The system never hashes the world to answer one question — it hashes what the question touches, and background capacity works through the rest.

## Attribution policy hook

The map is the one place that knows about sameness, so content-sharing policy lives here rather than scattered through view code:

- **Hardlinks** within a location share content by construction; the analyzer's attribution policy (primary link counts, others zero — the DaisyDisk convention) keys off the map.
- **APFS clones** share allocated blocks while reporting full logical size. Rollups track logical and allocated size separately (see the living-index research); the map is where clone-aware dedup and "real cost to delete" queries anchor.

## Bootstrap: never a dark moment

The existing `content_identity` table already holds hashes for everything indexed today. **Before the first location migrates, the map is seeded from it.** Cross-location queries therefore work continuously through the entire per-location migration: the old world populates the map, the new world maintains it, and no feature regresses while locations cut over one at a time.

## Rejected: query-time ATTACH fan-out

Attaching per-location databases at query time fails on exactly the cases that matter: detached sources cannot be attached at all, SQLite's ATTACH limit (default 10, compile-time max 125) caps fleet size, and N-way join latency scales with the slowest store. The map trades a little write-path bookkeeping for O(1) read-path answers that work offline. 

## Relationship to the merge layer

The map and the cross-location merge layer solve different problems and are both migration prerequisites. The map answers *identity and set* questions (one table, one query). The merge layer answers *enumeration* questions — sorted, paginated listings across N stores ("everything in this Space by size") — as an N-way streaming merge in application code, with ranked search already handled by the existing RRF router. Neither substitutes for the other.

## Sidecars: the content-addressed store

The sidecar tree (`sidecars/content/{h0}/{h1}/{content_uuid}/…`) is the third addressing scheme in the architecture and the one piece of V2 storage already built for it — it keys by content, never by record, so per-location migration doesn't touch it. The full picture:

1. **Per-source `index.db`** — record-addressed, disposable. Structured extraction you query (facets, EXIF rows). Travels with the source.
2. **Per-source `meta.db`** — record-addressed, **durable**. Assertions about this source's files: tags applied, corrections, notes. Survives index deletion, travels with the drive — plug your drive into another Spacedrive and your tags come along. (This splits what native kept central; the unplug story requires the split.)
3. **`library.db`** — about the *library*, never about files: source registry, settings, tag/space definitions (denormalized into meta.db rows for portability), and genuinely cross-source assertions (a Space spanning three drives is a claim about the combination no single source can own).
4. **Sidecar tree** — content-addressed. Derived artifacts you open or serve (thumbnails, transcripts, proxies). Rule of thumb: rows you query → facets; files you serve → sidecars.
5. **`projections/`** — disposable cache files (this content map, the sidecar index), one file each, rebuilt from the stores above and never inside `library.db`.

Consequences of content addressing:

- **Sidecar trees are mergeable caches.** Same content → same path → same bytes, so copying entries between trees can never conflict. Placement becomes policy: library-central by default (dedup wins), an on-drive subset for self-contained drives — a keyspace copy, safe in either direction. A drive's index + sidecar subset is a complete catalog; snapshots may reference sidecars by content uuid.
- **Sidecars require the integrity hash** (content uuid is UUIDv5-of-BLAKE3), so they exist only *after* content identity is discovered. The record-keyed hot tier (`thumbnail-storage.md`) covers the before; the two tiers are the before and after of content identity being known, not redundancy.
- **GC is a map query.** A sidecar content dir is orphaned when this map holds no row for its hash — with the detached-source courtesy: content that exists only on an unplugged drive keeps its sidecars. This gives the current GC stub (`cleanup_orphaned_thumbnails`, returns `Ok(0)`) its real definition.
- **The `sidecar` table is a projection** — rebuildable by scanning the tree — and lives in `projections/` beside this map, not in `library.db`. FSI's "per-file derived knowledge" (OCR, transcripts, classifications) rehomes to the tree unchanged.

## Worked example

Three sources: location A on an external drive (on-drive placement), Desktop and Downloads internal.

> **Amended 2026-08-18** (`docs/plans/2026-08-18-storage-consolidation.md`): superseded in two ways, though the placement story it illustrates stands. Desktop and Downloads are not sources — they are durable subtrees of the system-volume source (contract 1), so the internal `sources/` entries below collapse into that volume's store. And each `{ index.db, meta.db }` pair is one `source.db` with a durable/rebuildable logical split; `content-map.db` is a table set within the derived `catalog.db`.

```
.sdlibrary/
  library.db                    # 3 registry rows; desktop/downloads → internal,
                                #   loc-A → on-drive, volume fingerprint, attached state
  projections/
    content-map.db              # hash rows for ALL THREE — including A when unplugged
  sidecars/content/…            # central tree for desktop+downloads content (dedup'd)
  sources/
    desktop/    { index.db, meta.db, thumbs.pvcache, ephemeral.snapshot }
    downloads/  { index.db, meta.db, thumbs.pvcache, ephemeral.snapshot }

/Volumes/Archive/
  .spacedrive/  { index.db, meta.db, thumbs.pvcache, sidecars/…, ephemeral.snapshot }
  …the files…
```

**Attached:** all three load; the watcher routes per-source; the merge layer composes listings; the UI shows one world.

**Unplug Archive:** A's arena unloads. Its map rows stay (detached, last-seen), so the redundancy view still answers "is Desktop backed up to Archive?" and search can surface A's files as *on Archive (detached)* via cached display. A's tags are in its `meta.db` on the drive — unqueryable while gone, never lost. (Full-content search of A left with its FTS; a per-source policy may optionally cache a search projection locally for chosen drives.)

**Replug:** fingerprint match → restore arena from the on-drive snapshot — instantly browsable, thumbnails from the on-drive pvcache — then reconcile in the background and refresh map rows. Plug it into a *different* Spacedrive instead and the same happens there: index, tags, and thumbnails all arrive, because the drive is self-contained.

**Cross-cutting:** a file in both Downloads and A = one row-set in each `index.db`, two map rows (same hash), one sidecar set (content-addressed; A's on-drive subset may carry a copy). A Space containing Desktop + A files lives in `library.db`; while A is detached its members show offline via cached display, and light up on return.

**Overlap policy (settled 2026-08-18, consolidation doc contract 1):** subsumption. One physical filesystem = one source = one record namespace; Desktop and Downloads are durable subtrees carrying policy inside the volume source, and the registry rejects a root nested under an existing source's root. Nothing is ever migrated twice — promoting a subtree to durable is a policy row, not a new store.

## Open questions

- Sampling spec for `sampled_hash` (how many bytes, from where, versioned so a spec change invalidates cleanly).
- Map growth and pruning policy for very large fleets (rows are ~100 bytes; a million files per source × dozens of sources is still modest, but the policy should be written down).
- Whether snapshot artifacts include content hashes by default (they should when known — it makes snapshots diffable by content, not just structure).
- If device sync ever returns: the map is per-library and rebuildable, so it never needs to sync — each device derives its own. Worth preserving this property in any future design.
