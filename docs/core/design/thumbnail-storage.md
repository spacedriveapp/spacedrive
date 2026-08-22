# Thumbnail Storage: The Hot Tier

> **Status:** Design for porting native's pvcache as Spacedrive's GPU-resident thumbnail tier
> **Captured:** 2026-08-17
> **Companions:** `docs/plans/2026-08-17-record-table-transplant.md` (workstream D)

## The finding that reframes the port

The plan said "port native's two-tier bake + pvcache." The code audit says the two systems are complements, not competitors, and each repo shipped the half the other is missing:

- **Native's two-tier `_g.jpg`/`_d.jpg` deriver no longer exists.** It was the historical `crates/deriver`, deleted; `docs/design.md` describes it aspirationally. What native actually ships is `pvcache` — a fixed-layout mmap slab of raw 128px BGRA cells, filled lazily by a worker pool as you browse, uploaded straight into a `texture_2d_array` with zero decode on the render path — plus `bake` as the in-process cell producer (decode → aspect-fill crop → BGRA swap, visible-range-first scheduling, one thread per core).
- **Spacedrive already has the durable tier, and it's better than the dead deriver.** The sidecar system (`core/src/ops/sidecar/path.rs`) stores content-addressed WebP variants — `sidecars/content/{h0}/{h1}/{content_uuid}/thumbs/{variant}.webp`, keyed by UUIDv5-of-BLAKE3 so duplicate files share one thumbnail set automatically — with six defined variants (`icon/grid/detail` × `@1x/@2x`), served over an axum route in both shells with immutable cache headers, recorded in the `sidecar` table, chosen by the frontend from actual sidecar rows rather than guessed.

So the port is: **keep sidecars as the durable/serving tier, bring pvcache in as the hot tier**, and make the producer chain platform-first so the whole system is the default — FFmpeg becomes just one optional producer at the end of the chain, not a dependency of the system.

## Why a hot tier at all

The sidecar tier answers "give me a WebP over HTTP" in milliseconds — fine for a web DOM grid. It cannot feed a GPU grid at native's standard: rendering 100k cells at 60fps requires cells that upload without decoding, which means raw BGRA in a memory-mapped file, populated once, memcpy'd into atlas layers on demand. That is exactly what pvcache is, and nothing in spacedrive does it. It is also the only tier that can serve **ephemeral browsing**, which today has *no thumbnails at all* (`ephemeral/writer.rs:245` explicitly disables them) — the living index is currently blind, and the analyzer/photo-grid surfaces need it not to be.

## What must change in the port

Native's pvcache has three constraints that are fine for a static Apple Photos library and fatal for live filesystem sources:

**1. Positional keying to a feed snapshot.** Native's slot *K* is "the K-th uuid in the feed"; `open_or_create` reuses the file only if every slot uuid matches in order, and any membership change rebuilds from zero. A live source adds and removes files constantly. The fix is already latent in the format: `SlotRec` carries the 16-byte uuid, so the file is self-describing — the port drops the feed-order convention entirely. `slot_of` is rebuilt by scanning slot records at open; new records claim the next free slot; deletions tombstone.

**2. Fixed size with no growth.** The layout `[header][slots][cells]` is contiguous, so slots can't grow without moving cells. The port keeps the layout but adds **capacity doubling like the arena**: preallocate capacity for `next_pow2(thumbable_count × 1.25)` slots; on overflow, rebuild at 2× capacity by memcpying ready cells across — a copy, not a re-decode, so rebuild cost is IO-bound and rare. `set_len` preallocation gives a sparse file on APFS: unbrowsed cells cost no disk until written. Tombstoned slots are reclaimed during a rebuild.

**3. No per-record invalidation for mutable files.** Photos never change; files do. The format already has `version: u64` per slot and a `write_through_versioned` path — the port makes versions mandatory, derived from `(size, mtime)` (upgraded to content hash when a tier completes). The watcher's per-source event stream marks slots stale; stale cells re-produce lazily on next view, exactly like a miss.

Concurrency and durability carry over as-is: the lock-free EMPTY→WRITING→READY per-slot atomics are correct and simple, cells are immutable once READY, and the recovery story stays "cells are recomputable" — on open, any slot found WRITING resets to EMPTY; no checksums, no journaling. Pragmatism is the feature.

## Placement and keying

- **One pvcache per source**, living in the source's store: `sources/<source_id>/thumbs.pvcache` — beside `index.db`, covered by the same disposability rule, and **traveling with on-drive indexes**. Unplug-and-browse gets thumbnails for free: the drive in a drawer shows its photo grid from its own carried cache. (The catalog apps charge $40–80 and don't do this.)
- **Keyed by record uuid** (uuid v7, 16 bytes — `SlotRec.uuid` fits unchanged). Works for durable records and for ephemeral records (`entry_uuids` already exist in the ephemeral index), which is what turns thumbnails on for un-indexed browsing: no DB row required, the hot tier is self-sufficient.
- **Content thumbnails only.** Generic file-type icons never occupy cells — they render from a shared pre-rasterized tile set (native's `producer_icon_tiles` approach). Capacity is sized to thumbnail-able records (media, documents), not the whole source. Sizing reality: 128px BGRA = 64KB/cell; native's 118,910 photos → ~7.6GB file, sparse until browsed. A per-source cache plus view-driven fill keeps the resident set proportional to what's actually looked at — which also retires native's honest "resident-everything banks on 192GB" risk: unloading a source unloads its cache and its atlas layers, aligned with the living index's load/unload lifecycle.

## The producer chain

One chain, ordered by cost, replacing the current `is_ffmpeg_available` branching. Each producer either yields a BGRA cell or passes:

1. **Sidecar decode** — if a `grid@1x` WebP exists for the content, decode and crop it (~256px WebP → cell; cheapest real producer, and it means the hot tier warms from the durable tier without touching originals).
2. **Platform decode** — `sd-imageio`: ImageIO for images/RAW, QuickLook for video posters, PDFs, documents. On macOS this is full coverage with zero bundled codecs.
3. **FFmpeg** — only if the `ffmpeg` feature is compiled; covers video on Linux/Windows. (Windows later gets a WIC/Media Foundation producer in slot 2's role.)
4. **Type tile** — the shared icon tile, always succeeds.

This is the "adjacent system, default when FFmpeg is not present" — stated more strongly: it is the default *always*, and FFmpeg is demoted from a system dependency to one optional producer. A default macOS build loses nothing; a default Linux build degrades video to tiles instead of erroring (today it errors: `generator.rs` returns "requires FFmpeg feature" for video).

Write-through goes both directions: a cell produced from an original (producer 2/3) can optionally enqueue durable variant generation for the sidecar tier, so browsing warms both tiers; batch jobs (indexer deep mode) keep filling sidecars directly as today.

## Serving

- **GPU-native views** mmap the per-source pvcache directly — native's atlas consumption model carries over (texture array, per-frame upload budget, placeholder-cleared layers), with layers allocated per source instead of globally.
- **Web/DOM views keep the sidecar HTTP route** — it exists, it's cached-immutable, don't touch it. One addition: a `/hot-thumb/:source_id/:record_uuid` route that encodes a pvcache cell to WebP on demand (64KB BGRA → WebP is sub-millisecond work), giving web views thumbnails for **ephemeral-only** files that have no sidecar row. Grid parity across renderers without forcing the web onto mmap.

## Inherited fixes (do them inside this work, not before it)

The audit surfaced four defects in the current pipeline that this design absorbs rather than schedules separately:

1. The Tauri shell hand-builds sidecar paths (`apps/tauri/src-tauri/src/server.rs`) instead of using `SidecarPathBuilder` — one path builder, used by both shells.
2. Media metadata extraction runs only when `generated_count > 0` (`thumbnail/mod.rs`) — an already-thumbnailed file never gets EXIF/duration backfilled. Decouple metadata into its own pass keyed by content identity (it's facet data; see the cross-location content design).
3. `sidecar.version` is hardcoded to 1 and URLs are cached `immutable`, so forced regeneration serves stale from the browser. Use the version in the URL.
4. `cleanup_orphaned_thumbnails` is a stub returning `Ok(0)` — per-source stores shrink this problem (deleting a source deletes its cache), but content-dir GC for the sidecar tree still needs writing.

## What this changes in the transplant plan

Workstream D's task list becomes: port `pvcache` as `sd-pvcache` with the three changes above (uuid-keyed slots, capacity doubling, mandatory versions); port `bake`'s producer/worker model as the cell-production service behind the chain; do **not** port a "two-tier bake into the generator" — the durable tier already exists as sidecars and stays WebP. The `_g/_d` framing in earlier documents is superseded by this doc.

## Open questions

- Cell size: native uses 128px for photos, 256px for file cells. One cache with one `cell_size` per source, or 256px cells everywhere (4× the bytes, retina-clean at grid sizes)? Leaning 256px for filesystem sources, 128px acceptable for photo-library sources; the header's `cell_size` field already supports per-file choice.
- ASTC/BC7 compressed cells (native's roadmap noted 4× savings) — worth it only once atlas residency is measured; the format's `pixel_format` field reserves the door.
- Whether snapshot artifacts optionally include the pvcache (a browsable frozen drive with thumbnails is a strictly better catalog) — size vs utility tradeoff, per-snapshot flag.
- Detail-tier hot cells (native's dead deriver had a 1536px tier): the sidecar `detail@1x/2x` variants cover QuickPreview today; revisit only if preview latency measurably suffers.
