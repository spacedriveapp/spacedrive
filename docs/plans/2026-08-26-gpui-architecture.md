# GPUI Architecture

Captured 2026-08-26. The target shape for Spacedrive's two GPUI apps, written
against what `apps/native` already is and what nineteen reference codebases in
`~/Projects/gpui-refs` turned out to have solved.

`docs/plans/2026-08-25-gpui-interfaces.md` is the roadmap and the phase order.
This is the structure those phases build toward. Where the two disagree, this
one is later and wins; the differences are called out under "Changes from the
interfaces plan" at the end.

## The rule everything else follows from

**One gpui in the dependency graph, and it is ours.**

That was already the plan's position. The survey turned it from hygiene into the
load-bearing constraint, because the ecosystem is fragmenting faster than it is
consolidating. Trees found in active use across the clones: upstream
`zed-industries/zed`, `gpui-ce/gpui-ce`, `Havunen/gpui-ce` (a fork of the fork,
carrying GitComet), a copy of gpui-ce vendored wholesale into `frame`,
`gpui-unofficial` on crates.io at 1.17.0-pre (carrying gpuikit and hummingbird),
`crabtalk/zed` published as `bezel-gpui` 0.3.3 (carrying bezel), and ours.
Version strings in play for one framework: 0.2, 0.2.2, 0.3.3, 1.14.2, 1.17.0-pre.

Two properties of Cargo make this a constraint rather than an annoyance. `[patch]`
matches on package name and version, so a fork that renames or bumps is
unreachable from our manifest. And a library's components can depend on its
fork's private additions, the way bezel's glass needs `Window::paint_backdrop_blur`
from `crabtalk/zed`. So adopting a component library increasingly means adopting
its gpui, and two libraries on different forks cannot coexist in one binary.

The rule has three consequences that shape the crate graph below:

1. Third-party GPUI component libraries are **source material, not dependencies**,
   unless they sit on the tree we pin. gpui-component qualifies today because our
   manifest patches it onto our fork. bezel, gpuikit, and anything on
   `gpui-unofficial` do not.
2. Our own fork stays patch-free for as long as possible, so that moving between
   trees stays a rebase rather than a rewrite.
3. The graph is checked, not assumed. `xtask` grows a `one-gpui` check that fails
   if more than one package resolves to a gpui, and it runs in CI. Discovering a
   split graph from a window that paints shapes with no text is the failure mode
   bezel documents, and it is worth thirty lines to make impossible.

## Crate graph

```
gpui, gpui_platform, gpui_macros      spacedriveapp/zed @ e0931d5a, patch-free
        |
   gpui-base                          focus, popups, virtual_list, text selection,
        |                              positioner, a11y semantics
   sd-tokens                          palette, type scale, radii. gpui only.
        |
   sd-ui                              components, motion, the watch bridge
        |
   +----+--------------------+
   |                         |
sd-grid                 sd-platform   AppKit escapes, cfg-gated
   |                         |
   +----+--------------------+
        |
  apps/desktop        apps/photos
```

Every arrow points down and none point sideways. `sd-client` sits off to the
side of all of it, knows nothing about gpui, and is where daemon liveness,
navigation focus, and library selection already live.

### `crates/sd-tokens`

The interfaces plan had tokens as a module inside sd-ui. They get their own crate
because there is a real consumer that wants tokens without components: `sd-grid`
needs the selection tone and the label ink to paint a cell, and pulling popovers
and comboboxes in to get two colours is the dependency shape bezel split its
`theme` crate to avoid. One consumer that actually exists is the bar for a new
crate, and this clears it.

Depends on gpui alone. Contains the `Theme` struct, `Appearance`, and the palette
generator.

**The palette is generated, not transliterated.** `apps/native/src/theme.rs` today
is a hand-derived one-to-one transliteration of `theme.css`, which was the right
first move and does not survive a second theme. The replacement is a small brand
input that produces the full token set:

```rust
pub struct Brand {
    /// Hue and chroma for the neutrals. At chroma 0.0 this is the shipped grey.
    pub tint: Tint,
    /// The accent hue. Spacedrive's blue by default.
    pub accent: Tint,
    /// The button corner. Every other radius is a ratio of it.
    pub radius: Pixels,
}
```

Lightness is never an input. Each tone is tuned once against a measured contrast
ratio and a brand rotates hue while leaving those ratios in place, so light mode
and a future user accent cannot silently produce unreadable text. Tokens that are
already grey take the tint; tokens that already carry meaning (`status_error`,
`status_warning`, `status_success`) keep their hue. Radii derive from one number
so the concentric relationships hold when it moves.

This is bezel's `Brand` model rebuilt against SpaceUI's token names. The model is
worth taking; the crate is not, because of the fork.

Light mode is designed rather than derived. The dark ladder distinguishes
elevation by lightness, and light mode has nothing above white to climb to, so it
separates with border and shadow instead. Two designed palettes, one token set.

### `crates/sd-ui`

Components, and the only crate in the graph that writes `gpui::div()` chains for
product surfaces. Both apps compose sd-ui rather than gpui directly, which is
what keeps a future framework move a job in one crate.

Three modules:

- `components` splits by widget the way `apps/native/src/ui/` already does. Built
  on `gpui-base` behavior with sd-ui presentation, which is the layering
  Longbridge drew from the other side and the reason `gpui-base` exists. Take the
  things nobody wants to rediscover: focus restoration, keyboard wraparound, IME,
  popup collision, selection geometry.
- `motion` is the animation vocabulary, as durations and curves rather than as
  per-call-site literals. Small enough to stay a module.
- `bridge` is the one piece of glue the design needs, described below.

Accessibility roles and labels go in with each component. gpui carries accesskit
at our pinned rev and `gpui-base` has `macos_accessibility`, so writing a role
while writing a component is close to free and retrofitting one across a design
system is not.

`components` and `tokens` splitting into separate crates now means the later move
to `spaceui/crates/{tokens,ui}` is a directory rename.

### `crates/sd-grid`

`grid/`, `source/`, and the tile half of `data.rs`, moved intact. The element is
not the interesting part; a grid that paints tiles and leaves each app to invent
identity windows, completion routing, and viewport prioritisation has shared the
easy 624 lines and duplicated the hard ones.

The seam that already exists is right and does not change: `TileSource` produces
pixels off the UI thread, the grid culls, paints, and evicts, and `VisibleRange`
is one atomic word so neither side locks.

What is added is a second, orthogonal input:

```rust
/// What a cell says about itself. Photos supplies `NoCells`; the explorer
/// supplies a window over the record table.
pub trait CellModel {
    fn label(&self, idx: u32) -> Option<SharedString>;
    fn kind(&self, idx: u32) -> CellKind;
    fn state(&self, idx: u32) -> CellState;  // selected, cut, hidden
}
```

Two sources rather than one extended source, because the two have different
lifetimes and different owners. Pixels arrive from the bake pool over minutes;
labels arrive from a listing op in one round trip; selection changes on every
click. Folding them together forces Photos to answer questions it does not have
and forces a label refresh to invalidate a tile.

**Selection is not the grid's.** The grid renders a selection it is handed and
emits intents. Selection lives in an entity above the views, because grid, list,
column, and media all select the same records and a user switching views mid
selection expects it to survive. This is the single most common thing to get
wrong in a file manager and the cheapest to get right at the start.

Labels paint as gpui text and quads over the image first, and get measured against
the same `SD_GRID_BENCH=1` flywheel before anything more exotic is considered.
`sd_bake::render_file_cell` bakes labels into the tile with glyph coverage in
alpha, which is the reference app's answer and needs a shader we do not have on
gpui's sprite path. Staying on the sprite path is what made the 129k cell gate
pass. The visible set is around a hundred cells, so a hundred text layouts is the
number to beat before paying for a fork patch or an offscreen wgpu blit.

Pinch zoom belongs here. gpui delivers `PinchEvent { position, delta, modifiers,
phase }` at our pinned rev, it implements `MouseEvent`, and the grid subscribes
with the same `window.on_mouse_event` call it already uses for the scroll wheel.

### `crates/sd-platform`

GPUI is a rendering framework and AppKit is an operating system, and the gap is
real. Both apps want QuickLook, NSMenu, pasteboard drag, window materials, and
the services menu; the explorer wants drag to Finder on top.

Everything is `#[cfg(target_os = "macos")]` with the other platforms as empty
modules that return `Unsupported`, so call sites do not grow cfg blocks.

One rule is inherited from Cap's `platform.rs` and is worth stating where the
compiler cannot enforce it: **mutating an AppKit window from inside a gpui window
update re-enters gpui and panics with `RefCell already borrowed`.** Retain the
native window and mutate it from a spawned task. Every function in this crate
that touches an `NSWindow` takes the retained handle rather than a `&mut Window`,
which makes the correct thing the only thing that compiles.

Cap's 1,811 lines of documented escapes are the best available specification for
what belongs here. Loungy is the second reader for the same material.

## State lives in entities, and entities mirror the daemon

The daemon already names the state: library selection, navigation focus, explorer
session, selection, job queue, mount state, search session. `sd-client` already
holds several of them as watch channels, and `LibraryContext` is one today.

The rule is that **the UI views and controls Spacedrive, and does not own it.**
Core stays independently usable, which is what keeps the CLI, the server, and any
later surface honest. Concretely: no state that the daemon could answer for lives
only in a window.

Windows are views onto shared entities. Two apps with several windows each is
exactly the case that punishes autonomous per-window state machines, and Cap's
rewrite settled this by making `RecordingSession` an entity that both the main
window and the controls bar observe.

The glue is one function, written once in `sd_ui::bridge`:

```rust
/// Mirror a tokio watch channel into a gpui entity that any window can observe.
/// The task ends when the entity drops.
pub fn mirror<T>(rx: watch::Receiver<T>, cx: &mut App) -> Entity<T>
where
    T: Clone + Send + Sync + 'static,
```

`apps/native/src/data.rs` already does this by hand for focus, in about fifteen
lines. Written once, every daemon-backed entity gets it, and no screen contains a
`cx.spawn` loop.

The direction of travel is one way. Entities are read by views and written by the
daemon. A view that wants to change something sends an op and waits for the
resulting event, which means optimistic local state is a deliberate exception with
a reconciliation story rather than the default.

What moves into `sd-client` as part of this: liveness polling and the navigation
focus follower, both of which are in `apps/native/src/data.rs` today and both of
which the CLI would want.

## App layout

The lesson from the survey with the shortest path to regret is Cap's:
`apps/desktop-gpui/src/` is flat, around fifty modules, with `editor_window.rs` at
8,997 lines and `settings_pages.rs` at 8,664. That is what a fast conversion looks
like from the inside, and it is the argument for doing the extraction before the
screens exist rather than after. nohrs and GitComet both split by feature and both
stayed readable at 13k and 547k lines respectively.

```
apps/desktop/src/
  main.rs           window options, entity construction, one open_window
  state.rs          the mirrored entities, constructed once at app scope
  shell/            titlebar, tabs, sidebar, status bar
  explorer/
    session.rs      the entity: cwd, view mode, selection, sort
    navigation.rs   history, focus publish and subscribe
    views/          grid, list, column, media
    inspector.rs
  search/
  jobs/
  settings/
  spacebot/
```

`apps/photos/src/` keeps its current shape, which is already this shape at one
tenth the size: `main.rs`, `data.rs`, and a grid it does not own.

The four explorer views are functions of one `ExplorerSession` entity and share
the selection, the sort, and the cwd. A view is a rendering choice, so switching
one is a field write and a repaint.

## What carries each surface

| Surface | Carried by |
|---|---|
| Window, titlebar, traffic lights | gpui + gpui-component `title_bar` |
| Sidebar, spaces, tags | `sidebar`, `tree` over sd-ui presentation |
| Tabs and splits | `dock` |
| Path bar, top bar | `breadcrumb`, `input` |
| Grid and media views | `sd-grid` at two cell policies |
| List view | gpui-component `table`, virtualized |
| Column view | `dock` splits over `list` |
| Inspector | `sidebar` panel, `description_list`, `form` |
| Context menus | `native_menu` for a real NSMenu, `menu` elsewhere |
| Quick preview | `sd-platform` QuickLook. `gpui-video-player` and zorite's `gpui-pdf` are the fallbacks where QuickLook is not available |
| Job manager | `list`, `progress`, `chart` |
| Search | `input`, `searchable_list` |
| Spacebot | `text` markdown with selection, `input` |

## The data the explorer needs

`files.directory_listing` returns whole `File` objects with a default limit of
1000 and no cursor, and it reads `entry` and `directory_paths`, which is the world
`docs/plans/2026-08-20-entries-teardown-execution.md` is removing. Photos avoids
the problem by listing a folder into a `Vec<PathBuf>` once and windowing
identities through `thumbs.request`. An explorer over half a million records
cannot.

What is needed is a windowed listing over the record table returning exactly the
columns a cell draws and nothing more: identity, name, kind, size, modified. That
is the backing store for `CellModel`, and it is built against the record table so
the explorer is born in the new world rather than migrated into it.

Two questions that this makes concrete rather than settles. Whether the explorer
reads the record table only, and is therefore empty for anything not yet migrated,
or carries the entries path alongside and builds the seam twice. And whether the
daemon's one-round-trip-per-op protocol survives an explorer issuing many small
ops per navigation, which is a profile to take once it navigates for real.

## Where the reference codebases are actually used

Not a reading list. Each one answers a question the design above leaves open.

| Question | Read |
|---|---|
| What goes in `sd-platform`, and in what shape | `Cap/apps/desktop-gpui/src/platform.rs`, then `Loungy` |
| How an explorer splits into modules | `nohrs`, then `GitComet` for the same shape at 547k lines |
| Which virtualization for the list view | `dbflux/crates/dbflux_ui_document/src/data_grid_panel`, `zedis` for gpui's own `uniform_list` |
| Whether the token model holds | `bezel/crates/theme` |
| Image lifecycle at volume | `hummingbird`, and `Cap/target_thumbnails.rs` for streaming results one at a time |
| Gallery as a permanent fixture | `gpui-storybook`, `bezel/apps/gallery` |
| Icons | `gpui-symbols`, 9,000 SF Symbols typed |

## Standing rules

- One gpui in the graph, checked by `xtask` in CI.
- The fork stays patch-free. Upstream first, GPUI-CE second, a Spacedrive patch
  third, each changelogged in `Cargo.toml` with its reason.
- `sd-ui` builds against GPUI-CE periodically. The escape hatch is cheap only
  while it is exercised.
- A component lands in the gallery the day it lands in an app. The gallery
  composes each component exactly once, so the documentation cannot drift.
- Accessibility roles are written with the component, never retrofitted.
- No screen owns state the daemon could answer for.

## Changes from the interfaces plan

The phase order in that document is unchanged. Four things are different:

1. **Tokens become their own crate** rather than a module in sd-ui, because
   `sd-grid` is a real consumer of tokens that does not want components.
2. **Tokens are generated from a brand** rather than transliterated from
   `theme.css`. This is what makes light mode and a user accent possible without
   a second hand-derived palette.
3. **Cells get a `CellModel` alongside `TileSource`** rather than a `TileSource`
   extended with labels, which keeps Photos out of it and decouples P3 from P4.
4. **The one-gpui property becomes a CI check**, on the evidence that six trees
   are in circulation and Cargo cannot always be told which one to use.
