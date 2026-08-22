# Lighten the build, consolidate the archive

Four independent pieces of work in this repo. Each one is correct regardless of
how repo topology, licensing, or naming eventually land, so none of them wait on
those decisions.

Phases 1 and 2 are independent of each other and of phase 3. Phase 3 is
substantially larger than the other two and should land after them.

Phase 1 is done. So is the one part of phase 4 that affected the build, dropping
`wasm` from the default features.

---

## Phase 1 (macOS derivative backend) **done**

Replace the bundled-codec path on macOS with the system codecs, so the default
macOS build stops depending on the native-deps bundle.

Landed as `crates/imageio` plus a `System` variant in the generator, and the
bundle download moved behind `cargo xtask setup --native-deps`. Measurements and
the ImageIO behaviour worth knowing about are in
`docs/plans/2026-07-29-install-size.md`.

### What moves

`native/crates/imageio` → `crates/imageio`. 269 LOC, one dependency
(`core-foundation`). It is a thin FFI wrapper over:

- `CGImageSourceCreateThumbnailAtIndex`, decodes anything ImageIO supports
  (HEIC, RAW/ARW/DNG, JPEG, PNG, WebP, AVIF, GIF, PSD), applies EXIF
  orientation on the hardware path, writes a downscaled JPEG.
- QuickLook's legacy synchronous thumbnailer, content previews (PDF page,
  video poster, document) for anything Finder can preview, as a `CGImage`.
  Synchronous, which fits the existing blocking generator model.

It already handles the autorelease-pool drain per asset, which matters under
`spawn_blocking` where worker threads have no pool.

### What does not move

`native/crates/pvcache` stays where it is. It is a slot-addressed store of
square BGRA8 cells in an mmap, sized to feed a wgpu tile atlas and keyed on
Apple Photos capture-order slots. This repo serves encoded thumbnails over HTTP
to a React client, the storage shape doesn't apply.

### The seam

`ThumbnailGenerator::for_mime_type` (`core/src/ops/media/thumbnail/generator.rs:27`)
dispatches to `Image` / `Video` / `Document` generators, all behind one
`generate(source, output, size, quality) -> ThumbnailInfo` signature. Add a
macOS variant selected ahead of the mime dispatch under
`cfg(target_os = "macos")`, handling all three categories through ImageIO and
QuickLook.

`sd-images` (including its `heif` feature, ImageIO decodes HEIC natively) and
`sd-ffmpeg` stay for Linux and Windows, behind the feature flags they already
have.

### Payoff

On macOS the default build no longer needs `cargo xtask setup` to download the
native-deps bundle into `apps/.deps`, and the binary no longer loads dylibs from
the ~83MB `target/Frameworks/Spacedrive.framework` at runtime.

### Verification

Generate thumbnails for a fixture folder covering HEIC, RAW, JPEG, PNG, PDF and
a few video containers. Compare dimensions, orientation handling and wall time
against the current path before removing anything.

---

## Phase 2, compile-time gate for iroh

Make P2P opt-in at build time. Nothing gets deleted.

### This is not a size decision

Summed from `cargo bloat`, the entire P2P stack is ~2.4 MiB of `.text`: iroh
934.6 KiB, iroh_relay 415.0 KiB, igd_next 292.7 KiB, iroh_quinn_proto 270.8 KiB,
hickory_proto 189.8 KiB, swarm_discovery 163.8 KiB, portmapper 107.7 KiB,
hickory_resolver 84.5 KiB. Against a 44.4 MB binary that is 5%. The case for this
phase is simplification and making room for a Tailscale transport, and it should
be argued on those terms.

### What already exists

The runtime seam is done. `Services.networking` is
`Option<Arc<NetworkingService>>` (`core/src/service/mod.rs:41`) and
`CoreContext.networking` is `Arc<RwLock<Option<Arc<NetworkingService>>>>`
(`core/src/context.rs:32`), set after init via `set_networking`. Nothing assumes
the service is present.

`sd-server` already exposes `--p2p` (`apps/server/src/main.rs:386`, also
`SD_P2P`), though it defaults to `true`, so today the capability is opt-out at
runtime rather than opt-in.

### What's missing

A cargo feature. `iroh` is currently an unconditional dependency
(`core/Cargo.toml:141`), so the whole iroh + `quic-rpc` tree is in every build.

Scope, measured: `iroh` itself is named in 26 files, which is what the feature
gate has to cover in `Cargo.toml` terms. The `NetworkingService` type appears 59
times across 18 files. The word `networking` appears 1,012 times across 101 files,
which is the outer bound if the gate has to reach every call site rather than stop
at the service boundary. Scope this against the seam before starting: the point of
`Option<Arc<NetworkingService>>` is that the 18-file number should be the real
one.

Add `p2p = ["dep:iroh", "dep:quic-rpc"]` and gate:

- `core/src/service/network/`, 13 files, the bulk of it
- `core/src/service/file_sync/`, `core/src/service/sync/`
- call sites in `ops/network/pair/join/action.rs`, `ops/files/copy/strategy.rs`,
  `ops/devices/list/query.rs`
- `core/src/domain/device.rs`

Ops that need it return the same feature-disabled error shape the ffmpeg gate
already uses (`thumbnail/generator.rs:35`). One precedent, follow it.

### Check first

Whether the leaderless sync engine in `core/src/infra/sync/` (HLC clocks,
per-peer change logs, backfill) is separable from the iroh transport. If they're
welded, gate the transport only and leave the engine compiled, the engine is
worth keeping in the build graph either way.

---

## Phase 3, one archive

### The gap

`sd-archive` has no durable layer. Every source is a disposable per-source
SQLite database (`SourceManager::create` / `open` / `delete` in
`crates/archive/src/source.rs`). There is no overlay table, no cross-source edge
table, and nowhere for a user assertion to live. Deleting a source deletes
everything the user said about it.

Two other codebases hit this and solved it the same way: a disposable per-source
index plus one durable library holding overlay, edges and groupings:
`native/crates/model/src/schema.rs` and `console/archive/src/schema.ts`. Same
two-layer split, same `(src, dst, type, ord)` edge table, arrived at
independently.

### The design difference to resolve

`sd-archive` generates a bespoke schema per data type: `DataTypeSchema` → models
→ typed fields → generated DDL (`crates/archive/src/schema/`). The other two use
a universal record table with optional facet tables hanging off it.

Cross-source search and cross-source edges require the record table. You cannot join
across bespoke per-datatype schemas. That makes the record table a requirement, not a
preference.

Both mechanisms survive the merge: the record table becomes mandatory in every source
database, and the existing TOML→DDL codegen keeps its job but emits **facet**
tables keyed to the record table instead of standalone models.

### Target shape

Per-source `index.db` (disposable):

- `record`, uuid, external_id, type, title, timestamps, parent, content_id,
  version, scan_epoch
- `content`, sampled and integrity hash tiers, size, kind. Where this repo's
  existing BLAKE3 content identity and redundancy tracking plug in
- `edge`, intra-source membership and relations
- facet tables, generated per data type, self-registering their own DDL

One durable `library.db`:

- `sources`, registry
- `record_overlay`, favorite, hidden, locked, notes, custom data
- `grouping`, album/person/tag/collection, with a `predicate` column so smart
  lists are the same primitive as manual ones
- `lib_edge`, cross-source, user-asserted

### Order

1. Add `library.db` with overlay, grouping and cross-source edges. No change to
   source behaviour; the durable layer exists and is empty.
2. Add record tables to source databases, populated from the adapter stream
   alongside the existing model tables. Both paths live.
3. Route `search` and `list_items` through the record table. FTS and vector index
   against it.
4. Wire `content` to the existing content-identity path.
5. Drop standalone model tables. TOML models compile to facets only.
6. Migrate the 11 adapters in `adapters/`. The JSONL stdin/stdout protocol and
   the `adapter.toml` manifest do not change. This is a write-side change
   inside the engine.
7. Console cuts over: its TypeScript archive becomes a client of this engine
   over the already-frozen v0 MCP contract, and `console/archive/src/` is
   deleted.

Integration points in this repo are `core/src/ops/sources/` and
`core/src/data/manager.rs`.

---

## Phase 4, tear-outs

Two subsystems come out. They differ in kind: Spacebot is a separate product
that is no longer part of this one, so it gets deleted. WASM extensions are a
capability we are not carrying by default, so they get gated the same way iroh
and ffmpeg are, the code stays, the build stops paying for it.

### Spacebot UI

Delete. `packages/interface/src/Spacebot/` is 17 files and it is wired in, not
dead code, so the removal reaches further than the directory:

- `packages/interface/src/router.tsx` mounts it; `packages/interface/package.json`
  and `tsconfig.json` reference it
- `apps/tauri/src-tauri/src/windows.rs` gives it its own window, with
  `apps/tauri/src-tauri/capabilities/default.json`, `apps/tauri/src/App.tsx`,
  `apps/tauri/tsconfig.json` and both vite configs (`apps/tauri`, `apps/web`)
  pointing at it
- `apps/server/Dockerfile` and `apps/web/index.html` reference it
- `core/src/config/mod.rs`, `core/src/config/app_config.rs` and
  `core/src/ops/config/app/{get,update}.rs` carry Spacebot config
- `packages/ts-client/src/generated/types.ts` clears itself once core does
- `core/tests/file_sync_test.rs`, `file_sync_simple_test.rs` and
  `helpers/sync_harness.rs` reference it

Decide separately on `useTtsPlayback.ts`, `useAudioRecorder.ts` and
`windows/VoiceOverlay.tsx`, voice input is not inherently Spacebot-shaped and
may be worth keeping.

Four design docs describe the integration and go with it:
`docs/core/design/spacebot-{remote-execution,integration,spacedrive-contract}.md`
and `docs/design/POPOVER-REFACTOR.md`. Also `CONTRIBUTING.md:581`,
`docs/archive/README.md:11`, `adapters/opencode/adapter.toml`, and
`docs/core/design/{archive,file-system-intelligence}.md`.

### WASM extensions

Gate, don't delete. `wasm` has been dropped from `default` in `core/Cargo.toml`,
so `wasmer` and `wasmer-middlewares` are opt-in. That was the size-relevant half
and it is done. What remains is the tidying below.

The runtime seam already exists, same as iroh:
`CoreContext.plugin_manager` is
`Arc<RwLock<Option<Arc<RwLock<PluginManager>>>>>` (`core/src/context.rs:34`) and
`Core.plugin_manager` is `Option<_>` (`core/src/lib.rs:73`), constructed at
`core/src/lib.rs:451`. Nothing assumes it is present.

The work:

- `core/src/infra/extension/` is 1,404 LOC across 8 files, but only
  `manager.rs` and `host_functions.rs` touch `wasmer`, the rest is types,
  permissions, job registry and manifest handling
- `extensions/` keeps the SDK and the two examples (`photos`, `test-extension`,
  including a checked-in `test_extension.wasm`)

One thing to check before choosing gate over delete: the initial schema
migration (`core/src/infra/db/migration/m20240101_000001_initial_schema.rs`)
creates extension tables. Gating leaves them as inert empty tables, which is
fine. Deleting the system properly would mean a migration, which is the main
argument for gating instead.

## Out of scope

Repo topology, the open/closed line, and any renaming. All four phases hold
under every option on the table.
