# Coherent Startup

> **Status:** Day plan — in execution
> **Captured:** 2026-08-19, from James's product direction
> **Companions:** `docs/core/design/zero-onboarding-startup.md` (the indexing side of the cold open), `docs/plans/2026-08-18-storage-consolidation.md` (the stores), `docs/core/design/mounts.md`

## The directive

Spacedrive opens today with no locations and an empty-feeling sidebar. What
it should open with is **collections of things already identified**. The
day's goal: a coherent working Spacedrive whose visible surface runs on the
new sources world — no reliance on the legacy entry system, no sync — with
a sidebar that answers "what do I have" the moment the app opens.

The pieces, as stated:

1. **Identified collections, not dynamic pattern matching.** Screenshots
   are the archetype: every screenshot tool names files predictably, so
   classify at index time — once, during the walk or as a dedicated pass —
   and render "Screenshots" in the sidebar by default, opening in media
   view. The sidebar item is a *predefined search pinned to the sidebar*,
   backed by identification that already happened. Matching patterns at
   query time is rejected.
2. **Knowledge lives with the source.** Sources now live in multiple
   repositories, so a tag (or classification) belongs *locally, with that
   source* — the same rule that puts derivative media and sidecars next to
   the source. This is consolidation contract 1's spirit applied to
   knowledge: per-source stores own their records' classifications.
3. **macOS import at startup.** The amazing cold open for a macOS user:
   offer to import the Photos library (harvest it the way native did),
   Apple Notes, and Mail — searchable and *presented well*, not just
   ingested. Each section visible in the sidebar from first launch with a
   dedicated import button; empty state is an invitation, not a blank.
4. **The file-types section goes.** Removed from the sidebar quickly.
5. **Analyzer as a sidebar tab.** The analyzer is just a view over the
   index, loadable on any directory. The dedicated tab shows every volume
   rendered as a small analyzer in a scrolling grid; click a drive to load
   it in full fidelity.
6. **New identification steps run as dedicated jobs** on the new
   index/sources system — and the job system's entry-dependence needs
   auditing, because most existing jobs are believed to sit on entries.

## Slices (ordered)

1. **Sidebar coherence** — remove the file-types group; add pinned
   predefined searches ("Screenshots" first) as a sidebar concept; import
   sections (Photos / Notes / Mail) with buttons, present before any data
   exists. Depends on recon of in-flight sidebar work from parallel
   sessions before touching shared components.
2. **Screenshot identification** — index-time or dedicated-job
   classification over each source's arena (filename heuristics per
   platform: `Screenshot`/`Screen Shot`/`CleanShot`/`SCR-` patterns plus
   image kind), stored per source, surfaced through a query the sidebar
   item and media view consume.
3. **Analyzer surface** — the volumes-as-analyzers grid tab, then the
   full-fidelity single-volume view; both read the existing arena rollups
   (`subtree_bytes`), no new data layer.
4. **macOS imports** — Photos library harvest as a dedicated job (asset
   records, albums, EXIF; thumbnails through the existing pipeline), Notes
   and Mail through the archive adapter system if recon confirms adapters
   exist; otherwise the smallest honest importer per source.
5. **Jobs-on-sources audit** — inventory which jobs require entries; new
   jobs land entry-free by construction; note migration debt for the rest.

## Status

**Slices 1–2 landed (2026-08-19).** `ItemType::Collection { slug }` exists;
the default space seeds "Screenshots" where File Kinds used to be, and the
File Kinds item is deleted from existing libraries by deterministic uuid on
open. Collection flags live in the ephemeral arena (`collections.rs`),
classified at add time from name + kind and recomputed on snapshot restore
exactly like size rollups — no snapshot version bump, no derived file, and
heuristic changes re-classify for free. `files.collection_listing` fans out
across all sources (restoring registered ones first, so detached drives
answer). The UI gained a collection explorer mode, `/collection/:slug`
routing (defaults to the media grid), and a "Your Mac" sidebar group with
one-click Apple Notes import and Photos/Mail placeholders. Verified
end-to-end on an isolated daemon: 3 screenshots and 1 screen recording
identified across nested directories, still answered after restart via
snapshot restore.

**Resolved open questions:** classifications live in the arena (recompute
beats persistence while classification is a pure function of indexed data);
pinned searches are `ItemType::Collection` — a Space item, as directed.

**Found en route:** subdirectory browse jobs don't trigger snapshot saves —
after restart only the first browse's entries survived. Same family as the
known shutdown-save gap; the fix belongs with the snapshot save policy, not
collections.

**Slice 3 landed.** `files.size_tree` serves depth/top-capped hierarchical
sizes straight from arena rollups (a whole-volume tree ships in kilobytes;
beyond-top-N children aggregate into `other_size`). `ItemType::Analyzer` is
seeded into the sidebar; `/analyzer` renders every source as a pure-SVG
two-ring sunburst card in a scrolling grid — detached drives included, with
an unplugged badge — and clicking a drive opens the full-fidelity view:
depth-3 sunburst with click-to-descend, hover readout, and a largest-items
list. No d3, no new data layer.

**Slice 4, part one landed.** `sources.search` exposes the archive engine's
FTS router as a library query (`query, source_id?, data_type?, limit?`) —
the "archive search is dead code" gap is closed; imported notes answer
queries the moment sync completes. UI integration with the main search
surface is a design decision still open (the `/search` route is a stub).
Part two — the Apple Photos native-adapter port from the donor repo — is in
flight.

**Slice 4, part two landed.** Apple Photos ships as the archive system's
first native Rust adapter (`crates/archive/src/adapter/apple_photos.rs`):
read-only harvest of assets, albums, and people onto the spine with the
full EXIF facet, preserving the donor's hard-won pieces verbatim
(snapshot-before-query, the shape-resolved Core Data album join, thumbnail
fallback chain, EXIF sentinel cleaning). Smoke-tested against a copy of a
real 5.5GB library: 143,904 assets, 84 albums, 99 named people, zero
errors — and the shape resolution handled a newer Photos schema than the
donor ever saw (`Z_32ASSETS`, ten decoy tables skipped). Porting it forced
the harness fix that matters beyond Photos: the Adapter trait gained
`schema()`/`kind()`/`config_fields()`, so native adapters can create and
sync sources at all (previously hardwired to `adapter.toml`), and
`adapters.list` reports real kinds. The sidebar's Photos row is one-click.
Deliberately deferred: PhotoKit materialization, mutations, thumbnails,
and a stale-epoch deletion sweep. Harness debt noted by the port:
per-statement commits make big syncs ~4ms/record (232s for 143k) — batch
transactions or a version column on upsert is the fix; `SourceDb::link`
can't carry edge order.

**Remaining:** search-surface UI (unify `sources.search` with file
search), archive thumbnails for photos (pvcache/bake are unconnected),
slice 5 (jobs audit — recon done: the job manager is per-library and
`JobContext.library` is non-optional; the three entry-free jobs and
ephemeral IndexerJob are the precedents).

## Open questions (to resolve during the day)

- Where per-source classifications live *today*, before `source.db`
  exists: in the arena + snapshot (version bump), or a small per-source
  sidecar file in `sources/<id>/`.
- Whether "pinned search" is a Space item type (new) or a new sidebar
  group; James framed it as "a space item type that we don't have."
- How much of native's Photos harvest ports directly vs gets rebuilt on
  the adapter system.
