# Two GPUI Interfaces

Captured 2026-08-25. Spacedrive ships two GPUI apps as official interfaces: a
full explorer and Photos. They are separate binaries with separate windows,
they share their rendering and design tech through crates, and they coordinate
only through the daemon. The React app in `apps/tauri` keeps shipping alongside
them for as long as it is the more complete surface.

Companion: `company/gpui-research/` holds the August 2026 ecosystem read that
this plan takes its layering decision from. Everything in it that could be
checked against our pinned tree was checked, and the results are below.

This supersedes the deferral in `docs/plans/2026-08-18-native-app-gpui.md`,
which parked the native explorer behind Photos ("a full native explorer stays
the eventual destination"). Everything else in that document still holds: the
fork mechanics, the license rule, the hot tier, the loot table. Read it first.

## The decision

Two apps, one stack. The grid, the tokens, and the components live in crates
that neither app owns, because the moment Photos and the explorer render the
same folder with two grids, they drift. The seam is drawn where the reference
app already drew it: the grid is a pure consumer of a tile source, and the tile
source is the client half of the thumbnail hot tier.

The explorer is built side by side with the React app rather than as its
replacement. Both connect to the same daemon, both publish navigation focus, and
Photos follows whichever one is in front. That property is already true and
already tested: `navigation.set_focus` is published by `ExplorerProvider` today,
and nothing about the publisher is web-specific.

## What is already true

Read on 2026-08-25. `cargo check -p sd-native` passes clean on the `sources`
branch in 84 seconds.

| Piece | Where | State |
|---|---|---|
| Fork and pin | root `Cargo.toml` | `spacedriveapp/zed` at `e0931d5a`, patch-free, `[patch]` collapses gpui-component onto it. One gpui in the graph. |
| Grid | `apps/native/src/grid/mod.rs`, 481 lines | Custom gpui `Element` over gpui's own Metal sprite atlas. 129k cells at a locked 120Hz, p50 frame 8.33ms, ~0.3ms paint CPU. The B4 gate passed without a custom renderer. |
| Tile transport | `crates/pvcache`, `core/src/service/thumbs/` | Daemon owns every writer; clients map read-only and upload straight to the atlas. No HTTP, no decode on the hot path. |
| Bake | `crates/bake` | Producer chain, priority pool. Also carries `render_file_cell`: baked name and detail bands with glyph coverage in alpha. macOS only. |
| Data plane | `apps/native/src/data.rs`, 1015 lines | Frame loop never awaits. Tokio on its own thread, UI reads `Arc<FocusSnapshot>` from a watch channel, `has_pending()` settles so the app idles at 0% CPU. |
| Client | `crates/sd-client`, 962 lines | `CoreClient`, socket resolution, `ensure_daemon`, `SubscriptionBroker` with reconnect and drop-oldest fan-out, `LibraryContext`. |
| Tokens | `apps/native/src/theme.rs`, 214 lines | The full `theme.css` surface as `Hsla`, names transliterated one to one. Dark only. |
| Components | `apps/native/src/ui/`, 122 lines | `Button`. That is the whole library. |
| Launch | `apps/tauri/src-tauri/src/apps.rs` | Apps menu spawns `spacedrive-native` with nothing handed over. It finds the daemon itself. |

The pinned gpui-component rev carries two crates, and the split is the reason
this plan changed shape. `gpui-component` is the styled layer: `sidebar`, `dock`,
virtualized `table` and `list`, `tree`, `menu` plus real `native_menu`,
`breadcrumb`, rope-backed `input`, `title_bar`, `status_bar`, `form`, `chart`.
Below it sits **`gpui-base`**, whose module doc states the rule outright:
"Primitives deliberately avoid presentation styles. Layout, positioning, colors,
sizing, and motion belong to applications." It carries `focus_trap`,
`positioner`, `popup`, `virtual_list`, `table`, `tree`, `text_selection`,
`motion`, `input/`, `resizable/`, `scrollbar`, `macos_accessibility`, and a
`SemanticThemeTokens` struct that is explicitly free of component names.

The surface being matched is 48,613 lines of TypeScript across 282 files in
`packages/interface`: explorer 13.5k, shared components 20.5k, Spacebot 2.4k,
settings 1.2k, top bar 615.

## The middle layer

The interesting thing about `gpui-base` is that it is already in our dependency
graph at the pinned rev, and nothing in `apps/native` uses it directly.

Building on it rather than on styled gpui-component is the right call, and it is
the same conclusion Longbridge reached from the other side. Behavior and
interaction geometry belong below. Product presentation belongs above.

```
gpui
  ↓
gpui-base          focus, popups, virtualization, text selection, a11y semantics
  ↓
sd-ui              SpaceUI tokens and components. Spacedrive's visual language.
  ↓
apps/desktop  apps/photos
```

gpui-component stays a dependency, used as stock where a widget is genuinely
generic and as source material everywhere else. It is reference, not our design
system. The things worth taking from below are the ones nobody wants to
rediscover: focus restoration, keyboard wraparound, IME, popup collision,
selection geometry, scroll positioning.

This also changes what the theme bridge targets, though not as far as first
written. `SemanticThemeTokens` was read on 2026-08-26: 165 lines, seventeen
colours in shadcn's vocabulary (`background`, `foreground`, `primary`,
`secondary`, `muted`, `accent`, `destructive`, `border`, `input`, `ring`) plus
radius, spacing, typography and shadow scales. SpaceUI does not map onto it, it
collapses into it. `app_box`, `sidebar`, `ink_dull` and the `status_*` set have
no destination there. So it is the right thing to fill in for any gpui-base
component that paints, and it is not where the SpaceUI token set lives.
`docs/plans/2026-08-26-gpui-architecture.md` puts that in `sd-tokens`.

## The fork question

Four GPUI trees matter to us and we should know where we stand against each.
There are more than four in the wild; `company/gpui-research/` counts them.

**Upstream Zed** is what we pin. Zed's workspace marks gpui `publish = true` and
crates.io carries 0.2.2, but the published 0.2.2 and the git tree that also calls
itself 0.2.2 are materially different APIs. Our manifest already handles this,
and its comment already explains the version ambiguity. Keep pinning the rev and
treat the rev as the version.

**GPUI-CE** is a community fork that publishes as `gpui-ce` while keeping the
`gpui::` namespace, and it is taking the general-purpose work Zed declines:
corner smoothing, overflow edge fades, window capture, WGPU surface filtering,
iOS. Letter spacing, which is on the known-holes list, went there after upstream
did not take it. Do not depend on it. Do build `sd-ui` against it periodically,
because that is a cheap escape hatch and it stays cheap only if it is exercised.

**crabtalk/zed** publishes as `bezel-gpui` 0.3.3 and carries bezel, a component
library worth reading. It is unreachable from here: `[patch]` matches on package
name and version, so a fork that renames and bumps cannot be collapsed onto
ours. `company/gpui-research/bezel.md` has the detail.

**Our fork** stays patch-free for as long as possible. When something is needed,
try upstream first, GPUI-CE second, a Spacedrive patch third. The rule is worth
more than it looks: a library's components can depend on its fork's private
additions, so adopting a component library increasingly means adopting its gpui,
and a patch-free tree is what keeps us able to move between them at all.

## The shared tech

Three new crates. The first two are extraction-ready by construction, which the
existing modules already are.

**`crates/sd-ui`** takes `theme.rs` and `ui/`, adds `gpui-base` as a direct
dependency, and adds the token bridge onto `SemanticThemeTokens`. Components are
ours: `gpui::div()` chains live in here and application screens compose sd-ui,
which is what keeps a future GPUI migration a small job instead of a sweep
through every screen.

Accessibility semantics go in from the first component. gpui carries accesskit
at our pinned rev and `gpui-base` has `macos_accessibility`, so the plumbing
exists even though the platform coverage is partial. Adding roles and labels
while writing a component is nearly free. Retrofitting them across a design
system is not.

Modules `tokens` and `components` stay separate inside the crate so the later
move to `spaceui/crates/{tokens,ui}` splits on a line that already exists.

**`crates/sd-grid`** takes `grid/` and `source/`, and it takes the tile half of
`data.rs` with them. The seam is not the element. A grid that paints tiles but
leaves each app to invent identity windows, completion routing, and viewport
prioritisation has shared the easy 481 lines and duplicated the hard ones. What
moves: the `TileSource` trait, `PvcacheSource`, `VisibleRange`, the
`thumbs.request` conversation, and the completion demultiplexer that maps a
baked uuid back to a grid index.

**`crates/sd-platform`** is the escape hatch, and it does not exist yet. GPUI is
not AppKit and should not pretend to be. Both apps want QuickLook, NSMenu,
pasteboard drag, window materials, and the magnify gesture, and the explorer
wants drag to Finder and sharing on top. Cap reached NSWindow through
raw-window-handle for exactly this class of thing, and hit a trap worth
inheriting for free: mutating an AppKit window from inside a GPUI window update
re-enters GPUI and panics with `RefCell already borrowed`. Their fix was to
retain the native window and mutate it from a spawned task. Everything here is
`#[cfg(target_os = "macos")]` with the other platforms as empty modules.

What stays out of all three crates: daemon liveness, navigation focus, and
library selection. Those are not gpui concepts and `sd-client` is already their home.
Liveness polling and the focus follower move there, so the CLI and any future
client get them too.

The gpui bridge for all of it is small enough to stay in the apps: a watch
receiver awaited on gpui's executor, calling `cx.notify()`. `data.rs` already
does exactly this and it is about fifteen lines.

## Naming

`native` stops meaning anything when both apps are native.

| Now | Becomes | Binary |
|---|---|---|
| `apps/native` | `apps/photos` | `spacedrive-photos` |
| new | `apps/desktop` | `spacedrive` |

`crates/sd-platform` is new. `apps/tauri` keeps its name and its binary. `apps.rs` in the Tauri shell updates
its one binary string, and grows a second entry once the GPUI explorer is worth
launching from a menu.

While auditing: `sd-bake` is declared in `apps/native/Cargo.toml` and never
imported. It belongs to `sd-grid` if anything ends up baking client side, and to
nothing otherwise.

## State lives in entities, not in windows

Cap's rewrite settled one thing worth copying directly. Their `RecordingSession`
is a GPUI `Entity` observed by both the main window and the controls bar, and
their target selection is an application-scoped entity that polls in the
background and repaints several overlay windows. Windows are views onto shared
entities rather than autonomous state machines.

Two apps and several windows per app is exactly the case that punishes getting
this wrong. The entities Spacedrive wants are already named by the daemon:
library selection, navigation focus, explorer session, selection, job queue,
mount state, search session. `LibraryContext` in `sd-client` is one of them
already, in watch-channel form rather than entity form.

The rule that follows: the UI does not own Spacedrive, it views and controls it.
Core stays independently usable, which is what keeps the CLI, server, and any
later surface honest.

## The evidence, and what it is worth

Cap merged a 185-commit Tauri to GPUI conversion on 2026-08-23 and posted
same-machine numbers.

| Workload | GPUI | Tauri | Electron |
|---|---|---|---|
| Playback CPU | 17.5% | 76.6% | 53.2% |
| Playback RAM | 440 MiB | 3,724 MiB | 1,035 MiB |
| Recording CPU | 20.3% | 37.6% | |
| Recording RAM | 256 MiB | 1,983 MiB | |
| Packaged size | 162 MiB | 204 MiB | 410 MiB |

These are the vendor's own measurements, not independent ones. The magnitude is
still large enough to be strategically relevant, and the shipping strategy is
the one this plan already picked: GPUI opt-in beside the existing app rather
than a cutover.

The more useful part of that PR is not the numbers. Read from the clone, their
`apps/desktop-gpui` is 113k lines of Rust, so the conversion is real rather than
a demo. Two things to take from it and one to avoid.

Take `platform.rs`, 1,811 lines of documented AppKit escapes, which is the best
available specification for `sd-platform`. Take `ui/`, now 21 modules, extracted
partway through a rewrite that started without it. Build sd-ui first and skip
that step.

Avoid their file layout. `apps/desktop-gpui/src/` is flat, around fifty modules,
with `editor_window.rs` at 8,997 lines and `settings_pages.rs` at 8,664. That is
what a fast conversion looks like from the inside, and it is the argument for
doing P0 before the screens exist rather than after.

## Reference codebases

Cloned shallow to `~/Projects/gpui-refs`, alongside `~/Projects/gpui-component`
and `~/Projects/comet` (Zeron), which were already local.

| Repo | Size | Why |
|---|---|---|
| `noh-rs/nohrs` | 13.3k lines | A GPUI Finder alternative, and the closest thing to this plan that exists. Crate split is `nohrs-{core,models,store,services,pages,ui}`, which is the shape argued for above, arrived at independently. Explorer splits into page, state, view, navigation, entries, preview, search, types, with 965 lines of tests. File list is gpui-component's `ListState` behind a `FileListDelegate`. |
| `CapSoftware/Cap` | 113k lines in `apps/desktop-gpui` | The Tauri conversion. `platform.rs` and `target_thumbnails.rs` are the two files to read. |
| `vicanso/zedis` | 95k lines | Ships on three desktops with Homebrew, Scoop and AUR packaging. Virtualizes with gpui's own `uniform_list` rather than gpui-component's, which is a live question for our list view. `zedis-ui` is 2.6k lines over gpui-component. |
| `egoist/waku` | | A smaller, more recent app structure. Signed and notarized. |
| `crabtalk/bezel` | 37.6k lines | A component library layered theme / motion / ui, which is the sd-ui split reached independently. Take `Brand` and the glass recipes as source; the crates themselves sit on their own gpui fork. `company/gpui-research/bezel.md`. |
| `MatinAniss/gpui-book` | | Teaching material for the concepts, useful for onboarding. |
| `gpui-ce/gpui-ce` | 19 MB | The community fork, for periodic compatibility checks. Crate split mirrors upstream and adds `gpui_media`, `gpui_wgpu`, `gpui_web`, `gpui_scheduler`. |

`target_thumbnails.rs` is worth calling out because it is our problem in
miniature. Its header documents the deviations from the Tauri version it
replaced: no base64, no PNG round trip, RGBA straight to `RenderImage`, and
results streaming one at a time instead of arriving as one `Vec`. That is the
same shape as tiles riding the mmap instead of loopback HTTP, reached
independently.

## The explorer's spine

Not a parity port. The order below is by what unblocks the next thing, and the
right-hand column is what carries it.

| Surface | Carried by |
|---|---|
| Window, title bar, traffic lights | gpui + `title_bar` |
| Sidebar with spaces and tags | `sidebar`, `tree` |
| Tabs | `tab`, `dock` |
| Path bar and top bar | `breadcrumb`, `input` |
| Grid view | `sd-grid` |
| List view | gpui-component `table` (virtualized) |
| Column view | `dock` splits over `list` |
| Media view | `sd-grid` at a different cell policy |
| Inspector | `sidebar` panel, `description_list`, `form` |
| Context menus | `native_menu` for real NSMenu, `menu` elsewhere |
| Quick preview | Native escape. QuickLook on macOS, `webview` where it is not |
| Job manager | `list`, `progress`, `chart` |
| Settings | `form`, `sidebar` |
| Search | `input`, `searchable_list` |
| Spacebot | `text` markdown with selection, `input` |

## Gaps that need answers

Each one is cheap to settle and expensive to discover late.

**Cells have to become files.** The grid paints images and nothing else. An
explorer cell carries a name, a badge, and a selection state. Two ways to get
there. Bake the label into the tile, which is what the reference app does and
what `sd_bake::render_file_cell` already implements down to stashing glyph
coverage in alpha so a shader can repaint the name white over a selection pill.
That approach needs a shader we do not have on gpui's sprite path, so it means a
fork patch or an offscreen wgpu blit, and it is macOS-only as written. Or paint
gpui text and quads over the image, which costs a text layout for each visible
cell. The visible set is around a hundred cells. Measure the second before
paying for the first, because staying on gpui's sprite pipeline is what made the
B4 gate pass in the first place.

**There is no listing op an explorer can scroll.** `files.directory_listing`
branches: `find_parent_directory` against the `entry` table, falling through to
`query_ephemeral_directory_impl` over the ephemeral cache when that lookup
fails. With entries empty the indexed branch never runs, so Photos already lists
through the ephemeral path and the teardown takes a branch it does not use.

What the ephemeral path gives it is whole `File` objects, a default limit of
1000, and no cursor. That is enough for a folder of photos and useless for a
source with half a million records. `sources.list_items` has the offset shape and
is flat and untyped for this, and offset is the wrong shape regardless: paging by
offset re-scans from the top of the folder on every page.

The op is designed in `docs/plans/2026-08-26-gpui-architecture.md` under "The
listing op". Photos moves onto it because one listing path is better than two,
not because anything forces it.

**One TCP round trip per op.** The daemon closes the connection after every
non-subscribe response. The 2026-08-18 plan accepted this for Photos on the
grounds that tiles ride the mmap, and that reasoning is sound and still holds.
An explorer issuing many small ops per navigation is the case that changes the
answer. Keep-alive with request ids is a daemon protocol change. Gate it on a
profile taken once the explorer navigates for real, not before.

**Core is in the build graph of every client.** `sd-native` links `sd-core`
purely for op input and output types: 172k lines and the whole SeaORM stack, for
structs. The observed cost is 84 seconds of `cargo check` on a warm tree, which
is survivable and worth re-measuring once two apps and two shared crates are
iterating. If it stops being survivable the answer is a types-only crate that
core and clients both depend on, and that is a large mechanical change to make
on a whim. Decision point, not an action.

**Image lifecycle is our sharpest edge, and we are already on the right side of
it.** There is a public report of a GPUI app rendering a handful of large images
at roughly 300 MB of CPU memory, fixed by forking so uploaded bytes were not
retained, down to about 12 MB. A file manager is the worst case for this: 100k
thumbnails, several density levels, rapid scrolling, atlas churn. The grid
already owns the pipeline rather than leaning on `img()`, with an explicit LRU,
`drop_image` on both eviction and redelivery, and a stated budget of 512 tiles
at roughly 128 MB CPU plus the same in atlas pages. Keep owning it. What is
missing is memory pressure response and decode cancellation, and the second one
matters the moment someone flicks through a large folder.

**Build iteration.** Cap treats compile time as an architectural constraint and
configures optimized dependencies against an unoptimized app crate. Our
`[profile.dev.package."*"] opt-level = 3` already does this and goes further
than theirs. `cargo check -p sd-native` at 84 seconds is the current number to
beat. sccache is not installed on this machine and should be.

**gpui's known holes.** No div scale transforms, no group-hover, no
letter-spacing, no built-in text selection outside gpui-component's own text
elements. Zeron hit all of these and their fork carries Apache-2.0 patches with
attribution kept. Cherry-pick on demand with a reason in the Cargo.toml
changelog, and send anything general upstream. Do not pre-port.

**Mac-first, cross-platform-shaped.** gpui now decomposes into `gpui_platform`,
`gpui_macos`, `gpui_linux`, `gpui_windows`, `gpui_wgpu`, `gpui_web`, and Zed and
Zedis both ship on all three desktops. Practical readiness still ranks macOS
first, Linux second, Windows third, with more renderer edge cases visible on
Windows. What is ours to keep portable is the AppKit set: `sd-bake`'s icon and
cell rendering, QuickLook, NSMenu, the magnify monitor. All of it is already
`#[cfg(target_os = "macos")]` or trivially made so, and `sd-platform` is where
it goes.

## Distribution

Two GPUI binaries plus the Tauri shell is three apps for one product. The
current arrangement bundles `spacedrive-native` as a Tauri `externalBin`, which
works while Photos is a thing the explorer launches. Once the GPUI explorer is
its own front door that inverts, and the question is one bundle with helper
binaries against separate bundles that each find the daemon. The daemon already
makes either work, since no client is handed anything at launch. Settle it when
the explorer is worth shipping, not now.

## Phases

P0 through P2 are mechanical and unblock everything. P3 and P4 are the two real
questions. Nothing after P4 is ordered by anything but taste.

**P0 — Extraction.** `crates/sd-ui` and `crates/sd-grid` carved out of
`apps/native` with no behaviour change, `gpui-base` added to the workspace at the
same rev gpui-component is pinned to. The app becomes a consumer. Gate: the
`SD_GRID_BENCH=1` flywheel reproduces its numbers from the crate.

**P1 — Tokens and the base layer.** SpaceUI tokens compile to
`gpui_base::SemanticThemeTokens`. The `Button` that exists is rebuilt on
`gpui-base` behavior with sd-ui presentation, as the pattern every later
component follows. Accessibility roles go in here, not later. Gate: a widget
gallery pointed at our tokens reads as Spacedrive.

**P2 — Two binaries.** `apps/native` becomes `apps/photos`. `apps/desktop`
appears: window, sidebar, tab bar, daemon connection through `sd-client`, and it
publishes `navigation.set_focus` the way the React explorer does. Gate: Photos
follows the GPUI explorer with no code that knows which shell it is following.

**P3 — Cells become files.** Labels and selection in `sd-grid`, gpui text first,
measured against the same flywheel. Gate: 129k cells still at frame rate with
labels on. This is go/no-go for the explorer grid the way B4 was for the grid.

**P4 — A listing that scales.** The windowed record-table listing op, Photos
ported onto it, and `sd-grid` reading it. Gate: a source with six figures
of records scrolls without the listing showing up in a profile. Take the
round-trip measurement here and decide keep-alive.

**P5 — Views.** List over gpui-component's table, column, media. The inspector.

**P6 — Chrome and escapes.** `crates/sd-platform` appears: native context menus,
QuickLook, pasteboard drag, window materials, the glass toolbar. This is where
the deferred B6' Photos work lands, and it lands once, in shared code. Retain the
native window and mutate it off the update path.

Pinch zoom is not part of this and does not need `sd-platform`. gpui delivers it
at our pinned rev as `PinchEvent { position, delta, modifiers, phase }`, built
from `NSEventTypeMagnify` and `magnification()` in `gpui_macos`. The 2026-08-18
plan's instruction to port the reference app's NSEvent magnify monitor is
obsolete: that monitor existed only because iced 0.13 dropped the gesture before
any widget saw it. `PinchEvent` implements `MouseEvent`, so the grid subscribes
with the same `window.on_mouse_event` call it already uses for the scroll wheel.
It belongs in P3 with the other grid work.

**P7 — The rest.** Settings, jobs, search, Spacebot.

Standing tasks, not phases: install sccache, keep the fork patch-free, and build
`sd-ui` against GPUI-CE often enough that the escape hatch stays open.

## Out of scope

- Retiring `apps/tauri`. It ships until the explorer is better, which is a
  judgement to make with both in front of you.
- The spaceui merge and the token compiler. Still the settled destination, still
  after the prototype proves the shapes. P0 makes the split line explicit, which
  is the whole preparation needed.
- The types-only core crate. Measured, not acted on.
- Daemon keep-alive, until P4 profiles it.
- Windows and Linux polish.
- Mobile.

## Open

- One window with tabs, or a window per space? gpui-component's `dock` supports
  either and the React app answered tabs, but that answer was constrained by the
  browser.
- Whether Photos keeps its own window chrome or becomes a dock panel the
  explorer can also host. Standalone was the point, so probably both, and the
  cost of both is roughly zero once P0 lands.
- How much of `gpui-base` to take before it becomes the dependency we cannot
  move off. Focus, popups, input, and virtualization are clearly worth it. Its
  editor is not, at least not yet.
- gpui-component is reportedly growing a shadcn-style `registry` of copy-in
  source components. No such directory exists at our pinned rev. If it lands, it
  is a better donor model for sd-ui than depending on the styled layer, and it
  changes nothing about the layering above.
