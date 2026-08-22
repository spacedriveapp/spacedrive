# Entries Teardown — One System

> **Progress lives in `2026-08-20-entries-teardown-execution.md`** — its status
> table is the record, and it also holds the task register, the verification of
> this document's claims against the tree, and pointers to the previs and the
> pre-teardown brief. Start there to find out where things stand.
>
> Written 2026-08-20. Supersedes workstream C (per-location cutover) and
> contract 4 (migration state machine) of
> `2026-08-17-record-table-transplant.md` / `2026-08-18-storage-consolidation.md`.
> Contracts 1–3, the collapsed storage model, and the bug-register gate stand
> unchanged. Everything else in those docs remains the reference for *what to
> build*; this doc replaces *how the old world dies*.

## The stance change

The transplant plan was written as replacement under contract: residence
dispatch, golden parity recordings, per-location cutover, a durable state
machine so a mid-migration crash is recoverable. That machinery protects a
production install base. There isn't one. Since the plan was written:

- Locations died as a UX object (decided 2026-08-19); the sidebar runs on
  sources.
- The coherent-startup work made the ephemeral arena the primary read path —
  directory listing, collections, recents, search, and the sources media view
  all serve from it today. The entries branch of those queries is already the
  cold path.
- Sync is frozen by decision, and nothing reads `sync_enabled`.
- File actions (copy, rename, delete, create_folder) turned out to be
  path-native already — they never touch entry entities.

So the cutover is **reindex-from-zero**: the entry schema is dropped, sources
re-index, and the migration for user data is "the durable stores exist before
the drop." No residence dispatch, no dual-write window, no golden recordings
against a schema scheduled for deletion. The parity bar is redefined as: every
op the UI and CLI actually invoke works on the sources path. Ops that existed
only to serve the entries world are deleted with it.

## What is accepted as temporarily dark

Stating these up front, since the teardown must not wait for their
replacements:

- **Cross-source content answers** — alternates ("this file is also on that
  drive"), redundancy summary, at-risk filters. Return with `catalog.db`
  (phase 5).
- **Detail sidecars** — thumbstrips, proxies, OCR, transcripts, embeddings,
  gaussian splats. The generation jobs park until enrichment drains exist
  (phase 3); grids don't need them once the hot tier serves cells.
- **Location export/import** — deleted, not ported.
- **Sync** — already frozen; its tables and service go with the schema.

Not accepted as dark: browsing, search, collections, sizes, tags surviving a
restart, thumbnails in grids. Those gate their phases.

## Phase 0 — Gate fixes

The arena becomes the sole read substrate and then feeds the durable drain, so
the identity-corrupting bug register (consolidation doc, "Gate") goes first:

1. Duplicate-path adds never update metadata (`ephemeral/index.rs:203`) —
   modifications and rollups go stale.
2. Snapshot rewrite skipped on unchanged entry count (`ephemeral/cache.rs:457`)
   — renames and balanced churn never persist.
3. Root-match fallback hands a familiar mount path's source id to a different
   drive (`ephemeral/sources.rs`).
4. Attachment by path existence instead of fingerprint (`set_detached` sites)
   — same-named volumes cross-route. Fix is fingerprint-verified attachment on
   `VolumeMountChanged`, already a known follow-up.

Plus the three plausible-unverified reports from the external review (verify
while in the files): recursive-reindex orphans, registry save failures
swallowed, snapshot `source_id` unchecked on restore.

Acceptance: the round-trip test extended with a rename-only change and a
same-name different-drive mount; both survive snapshot save/restore with
correct routing.

## Phase 1 — Durable substrate (slice 2, unchanged in content)

The one real prerequisite: nothing durable may exist only in `library.db`'s
entry tables when they drop. Per the consolidation contracts:

1. **`source.db` per physical filesystem** (contract 1) — one transaction
   boundary holding the rebuildable generation (records, facets, FTS) and the
   durable tables (identity ledger, assertions). Archive-stack blockers land
   here: batched transactional `apply_mutations`, adapter.toml removal for
   Rust connectors, pooled source handles.
2. **Connector fan-out** (contract 3) — one observation stream from walk +
   watcher; uuid minted at first sight; arena and `source.db` are both
   consumers. Generation/watermark checkpointing in the same transaction as
   each batch.
3. **Identity ledger** (contract 2) — evidence-tuple rebinding so record uuids
   survive reindex, rename, and move. Two-factor rebind minimum.

Acceptance: index a source, tag a file, delete the rebuildable generation,
reindex — the tag is still on the file. Unplug mid-index, replug — no
duplicate records.

## Phase 2 — Re-anchor the survivors

Everything that reads the entry schema either moves to the sources path or is
deleted. The register, from the current tree:

**Move:**

- **Tags** (`ops/tags/*` — apply, unapply, files_by_tag, manager): a rewrite
  against `docs/core/design/tags-and-assertions.md`, not a re-key. Definitions
  and assertions both live in `source.db`; definitions replicate into every
  source that uses them; assertions are append-only, carry a record uuid and a
  content id, and are ordered by HLC. `library.db` keeps sidebar pinning,
  ordering, color overrides and definitions not yet applied anywhere. The
  fields the current schema carries that cannot merge across libraries
  (`composition_rules`, `applied_variant`, `confidence`, the four-way naming)
  are deleted rather than ported.
- **Path gates** — `is_path_allowed` in `file_transfer.rs:404` and
  `file_delete.rs:167` re-anchor from location roots to registered source
  roots, as the byterange protocol already does.
- **Watcher** — boot-time watched roots come from the source registry, not the
  locations table; events route by source-root prefix (workstream B move 4).
  The persistent change-detection handler (`change_detection/persistent.rs`)
  is deleted with the entries write path; its processor hooks return in
  phase 3 as drain consumers.
- **Enrichment policy** — `job_policies` on location rows becomes policy rows
  on source subtrees in `source.db` (the "what a Location becomes" decision,
  now with a home). `trigger_job` re-anchors to (source, subtree).
- **Library statistics** — file counts and sizes from arena rollups
  (`subtree_bytes` already landed) and source stores; the entry-table scans go.
- **Space items pointing at files** — already uuid-keyed; verify they resolve
  through the arena/ledger rather than entry rows, then no schema work.

**Delete:**

- `ops/locations/*` wholesale (add, remove, rescan, enable_indexing, list,
  update, validate, suggested, trigger_job after its policy move, export,
  import).
- The DB branches of `directory_listing`, `media_listing`, `search/query.rs`,
  `file_by_id`, `file_by_path` — the ephemeral/sources branch becomes the only
  branch. `alternate_instances` returns empty until phase 5 rather than
  keeping `content_identity` alive.
- `spaces/get_layout`'s entry joins (serve from arena lookups).
- `watcher_old`.

Acceptance: full UI pass (browse, search, collections, media view, tag a
file, spaces) with the entries branches compiled out behind a temporary flag,
before anything is dropped.

## Phase 3 — Media in the record table

The pieces that today justify `content_identity`, `sidecar`, and the media
tables:

1. **Hot tier** (workstream D / gpui plan A3–A4): daemon writes
   `sources/<id>/thumbs.pvcache` via the bake producer chain (sidecar decode →
   sd-imageio/QuickLook → optional ffmpeg → icon tile); fill trigger replaces
   the ephemeral writer's no-op stub, viewport-first; `/hot-thumb/:source_id/
   :record_uuid` on the mounts server encodes cells for DOM clients;
   `Thumb.tsx` prefers it. This removes the grid's dependency on sidecars
   entirely and is independent of phases 1–2 — start it in parallel.
2. **Content hashing** as the tiered ladder writing candidate/confirmed hashes
   to `source.db` records instead of `content_identity` rows.
3. **Media metadata as assertions** — EXIF, duration, dimensions become facet/
   assertion rows written by drain processors, replacing
   `image/video/audio_media_data`. (Blurhash is dropped, not ported.)
4. **Derivatives as assertions** — the surviving sidecar kinds re-home to
   content-keyed per-source storage recorded as assertions; regeneration is
   the migration, nothing copies the old `<library>/sidecars` tree. The two
   shell sidecar servers die; the daemon serves derivatives.

Acceptance: fresh library on a clean build shows thumbnails in every grid
surface (ephemeral included) with the sidecar tables gone; quick preview
still plays originals.

## Phase 4 — The drop

One commit series, after phases 1–3 hold:

- **Tables**: `entry`, `entry_closure`, `directory_paths`, `content_identity`,
  `content_kind`, `mime_type`, `location`, `indexer_rule` (rules live in
  settings/lens code now), `sidecar`, `sidecar_availability`,
  `image/video/audio_media_data`, `user_metadata`, `user_metadata_tag`,
  `metadata_label`, `label`, `collection`/`collection_entry` (superseded by
  ephemeral collections), the sync tables (`sync_conduit`, `sync_generation`,
  `device_state_tombstone`), FTS5 tables and triggers.
- **Migration history squashed**: `library.db` gets a fresh baseline schema —
  source registry, settings, devices, volumes, space definitions, spaces, tag
  presentation and unapplied tag definitions, audit log, cloud credentials,
  jobs. Early-stage rule applies: existing
  libraries are recreated, not migrated; a version check refuses old files
  with a "reindex" message.
- **Code**: the entities modules for dropped tables; `DatabaseAdapter` and the
  `Persistent` indexing mode (the sink enum collapses); the sync service
  (`service/sync`, ~264K) and its resource-manager hooks; `SidecarManager`'s
  DB flows; the golden-parity scaffolding that never got built stays unbuilt.
- **Surface**: dead ops unregistered, TS types regenerated, Locations settings
  UI removed, CLI location commands removed.
- The record table rule in CLAUDE.md (workstream G) flips from "do not deepen the
  entries world" to "the entries world does not exist."

Acceptance: clean build with no reference to dropped entities; fresh-start
daemon reaches the full coherent-startup experience; grep for
`entities::entry` returns nothing.

## Phase 5 — Catalog rebuilds the cross-source answers

`catalog.db` (consolidation model, store 3): global enumeration + placement
rows swept from source stores. Brings back alternates, redundancy,
detached-drive global search, and content-attached tag projection. Explicitly
after the drop — the teardown does not wait for it, and its trust-level rules
(never destructive decisions from projection rows) apply from day one.

## Ordering summary

Phase 0 → 1 → 2 → 4 is the dependency chain. Phase 3.1 (hot tier) is
parallel with everything and worth starting first — it pays off visibly
before the drop. Phases 3.2–3.4 gate phase 4's sidecar/media table drops.
Phase 5 is unblocked after 4.
