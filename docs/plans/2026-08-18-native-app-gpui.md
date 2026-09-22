# Native App on GPUI

Captured 2026-08-18, revised same day: the prototype is self-contained in
`apps/native` (no spaceui dependency — merge later), and the daemon-side
thumbnail hot tier lands before client work starts. The closed-source Native
strategy is superseded (North Star, "stopped" register) — the `native/` repo
stays frozen as the reference codebase we loot for techniques, and its iced
shell retires with it. Companions:
`docs/core/design/thumbnail-storage.md` (the pvcache
hot-tier design this plan executes), `docs/plans/2026-08-18-storage-consolidation.md`
(the per-source store layout the hot tier lives inside).

> **Revised 2026-08-21.** `apps/native` is Photos, an app launched from
> Spacedrive rather than a second Spacedrive. It follows a file explorer
> window through `navigation.focus` and renders that folder as a media grid.
> B5 and B6 below are superseded by B5' and B6'; a full native explorer stays
> the eventual destination and nothing here forecloses it.

## The decision

The native app is GPUI + gpui-component, pinned to a fork we control. Not iced,
not a webview, not vendored copies. For the prototype, everything lives inside
`apps/native` — its own theme tokens hand-derived from SpaceUI's `theme.css`,
its own component module — so early work never fights external repo state. The
spaceui repo grows its Rust arm later by extracting what the prototype proves,
not by blocking on it.

Order of operations: the daemon learns to serve tiles first. The thumbnail hot
tier (pvcache) is ported into core and filling before the client is built,
because the client's centerpiece — the grid — is a consumer of that tier, and
building it against the current HTTP + WebP path means building it twice.

## The evidence

Three codebases were read end-to-end on 2026-08-18:

**Zeron** (`~/Projects/comet`, github.com/zeronsh/comet) proves the ceiling:
~118k lines of Rust on GPUI, 610 commits and 73 release tags in ~30 days, ~880
tests, macOS + Linux shipping, fully custom design system. It also proves the
cost of raw GPUI: they run a personal fork of Zed (pinned rev, ~8 surgical
renderer patches, changelogged in Cargo.toml), use no component library, and
hand-built a 6.5k-line text input, their own markdown renderer, and text
selection — because GPUI ships none of those. Their comments are a free audit of
GPUI's gaps: no div scale transforms, no group-hover, no letter-spacing, no
built-in text selection, cosmic-text ignoring variable font weights on Linux.

**gpui-component** (`~/Projects/gpui-component`, longbridge, Apache-2.0) is most
of that bill already paid: 2 years, 2,077 commits, ~818 tests, 62 component doc
pages, shipping in Longbridge Pro. Coverage includes everything a file manager
needs: virtualized list and table, tree view, dock/split/tiles, context menus
plus real NSMenu, resizable panels, scrollbars, dialogs, sidebar, text input and
a Rope-backed editor, markdown with working selection, charts, a webview crate
(gpui-wry). Themes are JSON against a published schema, hot-reloadable. Its one
disease: it tracks Zed's default branch with no rev pin — only its lockfile
holds it still.

**Our iced app** (`~/Projects/spacedriveapp/native`) is the quiet case against
iced: the crown jewel is a custom wgpu pipeline inside an `iced::widget::shader`
— iced is chrome, and routed around whenever it matters (AppKit NSMenu because
iced's context menus aren't native, an NSEvent monitor because iced 0.13 has no
magnify event anywhere, 13 hand-copied hex constants because there's no token
pipeline). iced's stability is narrowness; we had already stopped asking it for
things.

## Why GPUI

- **Styling is Tailwind-isomorphic.** `v_flex().gap_2().bg(...)` is a
  transliteration of the classes already in `packages/interface`. Porting a
  SpaceUI component to GPUI is transcription; porting it to iced's per-widget
  style closures is reinterpretation — which is exactly how the 13-constant
  drift happened.
- **Editor-grade text.** GPUI carries Zed's shaping, IME, and layout; iced has
  no text selection at all.
- **Battle-tested at scale** — Zed on macOS, Linux, and Windows — with two
  ecosystems (Zeron-style bespoke, gpui-component) proving it works outside Zed.
- Honest cons, accepted: the API churns, external users pin or fork, and some
  gaps remain (documented above). The pin-and-patch mechanics below are the
  mitigation.

## Dependency mechanics

Fork, don't vendor. gpui lives in-tree at `zed-industries/zed` with its
dependency closure; vendoring severs the upstream merge path exactly where the
API churns most.

- **`spacedriveapp/zed`**, branch `spacedrive`, based at the rev
  gpui-component's lockfile already resolves
  (`e0931d5a9dbf4f781b336fdf448739e74a2ac0b5`) so everything compiles on day
  one. The fork starts patch-free.
- **One gpui in the graph**: our crates depend on
  `gpui = { git = "spacedriveapp/zed", rev = "…" }`, and the workspace adds
  `[patch."https://github.com/zed-industries/zed"]` for `gpui`,
  `gpui_platform`, `gpui_macros` — rerouting gpui-component's dep (which points
  at Zed's default branch) onto the pinned fork. `[patch]` matches by version,
  so the fork never bumps gpui's version numbers.
- **gpui-component pinned by rev** too. Upgrading is one deliberate ritual, not
  a lockfile accident: bump gpui-component, move the zed rev to its lock, rebase
  our patch stack, run the smoke suite.
- **Patches are cherry-picked on demand**, not pre-ported. When we hit walls
  Zeron hit, their fork (`wingleeio/zed`, Apache-2.0 patches, attribution kept)
  has candidates ready: sprite-atlas eviction (`ImageSource::evict`), GPU memory
  bounding, the line-wrap fix, macOS 26 backdrop blur. Every patch carries a
  reason in a Cargo.toml comment changelog, and anything general goes upstream
  as a PR so the stack stays thin.
- **License rule**: gpui and gpui-component are Apache-2.0 — fine. Zed's `ui`,
  `editor`, `markdown`, and `theme` crates are GPL-3.0. Importing one forces
  the combined work to GPL-3.0, which is not what the repo ships under. Never
  import them.
- The existing `apps/gpui-photo-grid` prototype (currently commented out of the
  workspace) already depends on gpui from zed main — it gets folded into
  `apps/native` and re-pinned along with everything else.

## SpaceUI stays in-app for the prototype

`apps/native` carries its own design layer, no spaceui repo dependency:

- **`apps/native/src/theme.rs`** — the full token set hand-derived from
  `spaceui/packages/tokens/src/css/theme.css` (all ~50 colors as Hsla, radii,
  type scale), dark first, structured as one struct so later themes are data.
  Not 13 constants this time — the whole surface, named identically to the CSS
  variables so drift is visible in review.
- **`apps/native/src/ui/`** — SpaceUI-shaped components over
  gpui-component/gpui-base, named after their `@spacedrive/primitives`
  counterparts (Button, CircleButton, sidebar chrome, context menus). The web
  rendering is the spec.
- The module tree is kept extraction-ready: `theme.rs` and `ui/` depend on gpui
  crates and each other, never on the app's data or view code, so the later
  move to `spaceui/crates/{tokens,ui}` is a lift, not a rewrite. That merge —
  and the token compiler that makes one source emit CSS and Rust — is the
  settled destination, deferred until the prototype has proven the shapes.

## The daemon side comes first: thumbnail hot tier

What exists today (read 2026-08-18): thumbnails are WebP sidecars, per-library,
content-addressed by `content_uuid` under
`sidecars/content/{h0}/{h1}/{uuid}/thumbs/{variant}.webp`
(`core/src/ops/sidecar/path.rs`), generated by `core/src/ops/media/thumbnail/`
(job + inline + processor entry points, macOS ImageIO backend), served over
loopback HTTP by two independent axum implementations (`apps/tauri` and
`apps/server`), URL-built at one frontend chokepoint
(`ServerContext.tsx:140`). The ephemeral index generates no thumbnails at all —
`run_processors` is a deliberate no-op stub — because sidecars require content
hashing, which ephemeral browsing skips.

`docs/core/design/thumbnail-storage.md` already designs the fix: sidecars stay
as the durable/serving tier; **pvcache comes in as the hot tier** —
`sources/<source_id>/thumbs.pvcache`, mmap'd BGRA slots keyed by record/entry
uuid (not content uuid, so ephemeral entries participate), per-slot version from
`(size, mtime)`, arena-style capacity doubling (`ephemeral/arena.rs` is the
in-repo precedent), a four-stage producer chain (sidecar decode → platform
decode → FFmpeg → icon tile), and a `/hot-thumb/:source_id/:record_uuid` route
for DOM clients.

The part that matters for this plan: **for the native client, the pvcache file
is the transport.** The daemon writes tiles; `apps/native` opens the same file
mmap'd read-only and uploads straight to its atlas — zero-copy, no HTTP, no
WebP decode on the hot path. Ops and events carry only metadata: what's filled,
what changed, what to prioritize. This is the property that makes the grid
possible, and it's why this tier lands before the client exists.

Sequencing dependency: the hot tier lives inside per-source stores
(`sources/<id>/…`), which are designed
(`docs/plans/2026-08-18-storage-consolidation.md`) but not yet built. The
pvcache port lands on that layout, not ahead of it.

## The client foundation

`sd_core::client::CoreClient` (`core/src/client/mod.rs`) is already a real
typed client — `Wire`-trait method safety, `query`/`action`, event and log
subscription streams — proven at scale by the CLI. `crates/sd-client` also
exists but is a prototype (per-call `eprintln!`s, one hardcoded op, response
shape sniffing) and buys nothing since it depends on sd-core anyway.

The move: **promote `CoreClient` + `infra/daemon/{client,types}` into
`crates/sd-client`, replacing its contents**, re-exported from core so the CLI
keeps compiling. Then add what a windowed client needs and the CLI never did:

- socket-address resolution (the port derivation currently copy-pasted in
  three places) plus ping-and-spawn daemon liveness;
- a subscription broker — one daemon connection per distinct filter, fanned out
  to many in-app receivers, with reconnect. The daemon keys subscriptions by
  connection (a second `Subscribe` on one connection replaces the first), so
  this layer must be client-side; it's the Rust equivalent of ts-client's
  `subscriptionManager.ts`, which has no Rust counterpart today;
- a `LibraryContext` (current-library state lives only in the CLI today).

Known and accepted for the prototype: the daemon closes the connection after
every non-subscribe response, so each op is a fresh TCP round trip. Tolerable
because tiles don't ride ops — they ride the mmap. Keep-alive with request ids
is a daemon protocol change, deferred until profiling says otherwise.

## What we loot from `native/`

The repo stays as-is (supersession notes added to its `CLAUDE.md` and
`PRODUCT.md`); its techniques move, not its code:

| Technique | Where it lives today |
|---|---|
| GPU tile-atlas grid — instanced draw, atlas UV from instance index, storage-buffer selection animation | `crates/app/src/view/grid.rs`, `shader.wgsl` |
| Frosted-glass toolbar (separable Gaussian over the scene texture) | `crates/app/src/glass.wgsl` |
| Cell pre-rasterization — all content baked to BGRA8 tiles off the hot path | `crates/bake` |
| mmap'd write-through tile cache | `crates/pvcache` |
| Async data plane / sync render seam — the frame loop never awaits | `crates/app/src/data.rs` |
| Baked text labels recolored in-shader (no second layout system over the grid) | `docs/plans/2026-06-17-file-cell-selection.md` |
| Native escapes worth keeping even under GPUI: QuickLook, NSOpenPanel, pasteboard drag | `crates/app/src/native_menu.rs` |

## Inherited fixes and dead code

Found during the reads; cheap to clear while working in these areas, and each
one misleads whoever touches the area next:

- `core/src/ops/registry.rs` — dead near-duplicate of
  `infra/wire/registry.rs`; nothing imports it. Delete.
- `core/src/ops/media/thumbnail/utils.rs` — describes a cas_id-sharded layout
  the system doesn't use; only its own tests exercise it. Delete.
- `apps/tauri/sd-tauri-core` — a 54-line stub bridge nothing uses. Delete when
  tauri collapses onto `sd-client`.
- `apps/tauri/src-tauri/src/server.rs` hand-builds sidecar paths instead of
  using `SidecarPathBuilder` (duplicated `transcript` special-case included).
- `SidecarManager::enqueue_generation`'s `Thumb` arm is
  `#[cfg(feature = "ffmpeg")]`-gated, so lazy image thumbnails silently no-op
  on non-ffmpeg builds.
- `sidecar.version` is hardcoded to 1 while HTTP responses say `immutable` —
  regeneration serves stale to browsers.

## Phases

Track A is the daemon; the client app does not start until A is done. A2–A4
and B1–B2 have no dependency on each other and can run concurrently.

**A1 — per-source stores land.** The `sources/<id>/` layout from the
storage-consolidation plan, at least far enough that a source has a directory
the cache file can live in. This is the storage-consolidation lane's work; the
thumbnail port waits for it rather than inventing an interim layout.

**A2 — `crates/pvcache`.** *(landed 2026-08-21)* Ported from `native/` per the
thumbnail-storage design: uuid-keyed slots, per-slot `(size, mtime)` version,
capacity doubling, write-through. Core owns every writer handle on the machine
(`core/src/service/thumbs/`), which is what makes the format's single-writer
contract true rather than hoped for; the file treats read-only external mappers
as first-class.

**A3 — the producer chain fills it.** *(landed 2026-08-21, on demand)* Nothing
is baked speculatively. `thumbs.request` names the paths a client is about to
draw, in draw order; the daemon resolves each to its record uuid and `(size,
mtime)` version, and queues a bake for anything not already fresh. Completions
go out on the bus as batched `thumbnail` resource events. The producer chain is
raster decode then the platform icon tile; FFmpeg and sidecar-decode stages are
still to come, as is `run_processors` as a second, index-driven fill trigger.

**A4 — serving and parity.** *(route landed 2026-08-21)*
`/hot-thumb/:source_id/:record_uuid/:version` serves DOM clients out of the same
cache file, opened read-only and encoded to PNG on the way out; the explorer's
`Thumb` prefers it over loading a full-resolution original, which is what
browsing an unindexed folder did before. Sidecars stay the durable tier. The
inherited fixes listed above are not cleared yet.

**B1 — `sd-client` promotion.** `CoreClient` moves in, prototype contents
replaced; socket resolution, ping-and-spawn, subscription broker,
`LibraryContext` added; CLI keeps compiling via re-export.

**B2 — fork and workspace wiring.** `spacedriveapp/zed` at the pinned rev,
`[patch]` unification, gpui-component pinned, a hello-world window compiling in
the workspace.

**B3 — `apps/native` scaffold.** Workspace member; window chrome; `theme.rs`
with the full token surface; `ui/` with the shell's minimum components checked
against web rendering; `apps/gpui-photo-grid` absorbed and its workspace entry
removed.

**B4 — the grid gate.** The atlas grid as a GPUI custom Element (or offscreen
wgpu blit if elements can't carry it), fed by read-only mmap of
`thumbs.pvcache`, selection animation included. Acceptance: six-figure cell
count at frame rate. This is go/no-go for everything visual that follows.

**B5' — Photos follows a window.** *(landed 2026-08-21)* Navigation focus in
core (`core/src/ops/navigation/`): an in-memory, group-keyed row holding an
`SdPath`, published by `navigation.set_focus` and read by `navigation.focus`,
emitted as a `navigation_focus` resource so followers read the new position
straight off the event. The explorer publishes from `ExplorerProvider`; the
grid retargets and the previous fill is cancelled. `apps/native` lost its
sidebar and list pane and is one grid with a Follow toggle; the Apps menu in
Tauri launches it.

**B6' — Photos becomes an app.** Pinch zoom (`MIN_CELL`/`MAX_CELL`, an
NSEvent magnify monitor as in the reference app), the photo view adapter,
selection, glass toolbar, QuickLook and native menus. The bar is the reference
app side by side on the same folder. Selection and tag mode landed 2026-09-21:
the explorer's selection rules, T for the explorer's tag mode with the number
keys toggling the library's first ten tags, and tag dots on tagged tiles.

**Deferred with the native explorer:** the sidebar, list view, and inspector
(B5 as written), and the parity audit against the web app (B6 as written).

## Out of scope

- The record table transplant and the storage-consolidation internals (their
  own plans; A1 is a dependency on that lane, not a duplication of it).
- The spaceui merge — extraction of `theme.rs`/`ui/` into
  `spaceui/crates/{tokens,ui}` and the token compiler. Settled destination,
  after the prototype.
- Daemon keep-alive protocol (revisit if op latency shows up in profiles).
- Mobile SpaceUI.
- Windows/Linux polish — GPUI supports both and nothing here forecloses them,
  but the wedge is Mac-first.
- Retiring `native/` — it retires by standing still.
