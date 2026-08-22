# File-Backed Sources

> **Status:** First slice implemented 2026-08-19 (see "Implementation status" below) — design captured from James, 2026-08-19
> **Companions:** `docs/plans/2026-08-18-storage-consolidation.md` (contracts 1–2), `docs/plans/2026-08-17-record-table-transplant.md`, `docs/core/design/zero-onboarding-startup.md` (rules as lenses)

## The rule

When an adapter's subjects are local files, **the filesystem index owns the
record and the adapter contributes knowledge about it.** The real files are
indexed first — identity minted at discovery by the walk, one namespace per
physical filesystem (contract 1) — and the adapter's database supplies
metadata those files cannot: capture dates, camera EXIF, favorites, titles,
album and people membership.

The current Apple Photos adapter predates this rule: it mints a parallel
record universe pointing at the same bytes the volume source will index,
which is exactly the second-namespace shape contract 1 exists to forbid.
The symptoms are visible in the UI — 0-byte sizes and UUID display names,
because surfaces read the shadow record instead of the file.

## Adapter taxonomy

- **File-backed adapters** (Apple Photos; later Lightroom, Capture One, any
  media catalog): enrich records owned by a filesystem source. Their file
  rows become assertions joined to filesystem records; only their genuinely
  non-file entities (albums, people) remain records, with member edges
  pointing at filesystem record uuids.
- **Foreign adapters** (Gmail, Slack, GitHub, browser history): no local
  files; their records are primary, unchanged from today.
- Apple Notes stays foreign for now — bodies live inside Apple's store, not
  as files on disk.

## Mechanics

1. **The walk must see the bundle.** Indexing rules currently exclude
   `.photoslibrary`; under rules-as-lenses the walk indexes
   `originals/**` and the *lens* hides bundle internals from ordinary
   browsing. Enrichment policy on the subtree (a policy row, per "what a
   Location becomes") drives hashing and thumbnails like any other files.
2. **Join by evidence.** The adapter knows each asset's relative original
   path (`originals/<dir>/<file>` from `ZASSET`); resolution to the
   filesystem record uses path evidence first and content hash as the
   stronger rebind key — contract 2's ladder, reused.
3. **Enrichment lands as assertions** on the filesystem record (durable,
   travels with the source), not as new records. Albums/people are grouping
   records with edges to filesystem record uuids.
4. **Ordering:** the adapter sync depends on the subtree being indexed;
   when it runs first, its assertions hold path evidence and bind when the
   records arrive — the same late-binding the drain pipeline already
   requires.

## What this buys

Real sizes and names from the file record; dedup and "this photo is also on
that drive" through content identity; thumbnails through the standard
pipeline instead of depending on the catalog's derivatives; correct
analyzer attribution; and no double identity when the whole-volume startup
index inevitably walks the same bytes the adapter describes.

## Adapter disk cost

The Photos sync snapshots the catalog database by clonefile on APFS
(`snapshot_file` in the adapter): instant, byte-identical, and zero
additional space until the original diverges, with a byte-copy fallback on
filesystems without cloning. The snapshot-before-query discipline stays;
only its cost went away.

## Implementation status (2026-08-19)

The first slice is built as read-time binding — full assertion storage on a
durable filesystem record store waits on the storage consolidation (contracts 1–3).

- **The walk already sees the bundle.** The indexer rules exclude only the
  catalog's internal dirs (`database`, `resources`, …); `originals/**` was
  never excluded, so filesystem records for the real files exist today. No
  rule change was needed.
- **Lens** (`core/src/ops/indexing/lens.rs`): `is_bundle_internal` is a
  pure path predicate (any strict ancestor ending `.photoslibrary`),
  applied at the read sites — directory listing (grouped with the hidden
  filter, so `include_hidden` is the show-all escape hatch), collections,
  recents, and ephemeral search. The analyzer keeps summing raw. No stored
  flag: the verdict derives from the path alone, so there is nothing to
  recompute on restore.
- **Assertions**: the adapter's photo rows now carry library-relative
  `original_path`/`thumb_path` — pure path evidence — and each sync stamps
  the resolved library root into `_sync_state` (`file_root`). A source with
  a file root is file-backed; without one it is foreign. Albums and people
  remain records with membership edges to the assertion rows.
- **Read-time binding** (`sources.media_listing`): each assertion joins to
  its filesystem record by path against the ephemeral partition covering
  the file root. Bound files carry the filesystem uuid and real size; the
  assertion supplies capture time, kind, thumbnail, and the catalog's
  original filename as display name. Ordering is moot: binding happens at
  every read, so adapter sync and the walk can land in either order.

Decided along the way:

- **Presentation titles** live in the projection: the assertion's
  `filename` (Photos' original name) beats the on-disk UUID name in media
  surfaces. A view concern, not a stored precedence field.
- **Migration**: none needed. External ids are stable, so re-syncing the
  existing shadow source overwrites its facets with relative evidence in
  place; binding picks it up on the next read.
- **iCloud-evicted originals** surface unbound — stable archive identity,
  zero size — which is the "known but not resident" state for now; a
  distinct UI treatment is still open.

## Still open

- Assertions as durable rows in the filesystem source's own record table (with
  bound record uuids cached, not recomputed per read) once filesystem
  sources gain durable stores — the storage-consolidation ladder then
  upgrades path evidence to content-hash rebind.
- Membership edges pointing at filesystem record uuids directly (today
  they reach them transitively through the assertion rows).
- Batched transactions for adapter sync — every photo is still 2–4
  autocommit statements; the dominant cost on a 143k-asset library.
