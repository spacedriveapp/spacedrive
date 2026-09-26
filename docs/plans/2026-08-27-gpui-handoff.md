# GPUI Build Handoff

Written 2026-08-27 for whoever picks up the GPUI app work. Two documents already
describe what to build. This one exists so you do not have to reconstruct the
research that produced them, and so you do not repeat the six mistakes that were
made and corrected along the way.

Read in this order:

1. `docs/plans/2026-08-25-gpui-interfaces.md`, the roadmap, the fork decision,
   and the phase order P0 through P7.
2. `docs/plans/2026-08-26-gpui-architecture.md`, the target crate graph, the
   token model, the listing op, and the app module layout. Later than the
   interfaces plan and wins where the two disagree.
3. `docs/gpui/state.mdx`, how GPUI state works, written for someone arriving
   from React. Read this before writing a view.

Outside the repo, one level up in `~/Projects/spacedriveapp/company/gpui-research/`:
`gpui-research-seed.md` (the ecosystem survey), `bezel.md` (why that library is
source material and not a dependency), and `clones.md` (an index of nineteen
reference codebases in `~/Projects/gpui-refs`, each tagged with the question it
answers). Do not re-clone them; they are already there.

## Where the code is right now

`apps/native` is the Photos prototype. 4,881 lines across eleven files:

```
apps/native/src/
  main.rs        282   window setup, the Photos struct, focus subscription
  data.rs      1,015   liveness polling, focus follower, list_media, v5 uuids
  theme.rs       214   a hand transliteration of theme.css
  grid/         724    the virtualized tile grid and its bench stats
  source/     1,004    TileSource, pvcache, synthetic, empty
  ui/           131    Button, the one component
```

It builds and runs. `cargo run --release -p sd-native`. It is excluded from
`default-members` so ordinary checks do not compile the gpui tree.

`SD_GRID_BENCH=1` is the performance flywheel. It is the gate on P0 and P3, and
the number to reproduce is 129k cells at frame rate.

## Facts already verified, so you do not check them again

Each of these was established by reading the code, not inferred.

**The gpui pin.** `spacedriveapp/zed` at `e0931d5a9dbf4f781b336fdf448739e74a2ac0b5`,
with a `[patch."https://github.com/zed-industries/zed"]` block rerouting
gpui-component's unpinned zed dependency onto it. `Cargo.lock` resolves exactly
one gpui 0.2.2. The version string is explicit in the manifest because the zed
tree also contains a 0.0.0 `gpui` lint fixture that makes resolution ambiguous
without it.

**gpui-base is already in the lock.** It arrives transitively from
`longbridge/gpui-component` at rev `7acfc184382d30864a688fdaa6c9ff719efc53ae`,
the same repo and rev gpui-component itself is pinned to. P0's "add gpui-base at
the same rev" is one workspace dependency line, with no resolution work.

**`gpui_tokio` exists at our pinned rev and is unused.** `Tokio::spawn` returns a
gpui `Task` that aborts the tokio task when it drops. Relevant to the `bridge`
module in `sd-ui`.

**`sd-bake` is unused by `apps/native`.** The prototype at `a802e9053` used it
(BakePool, ImageProducer, IconProducer). The rewrite at `33a86dacd` removed the
import and left the Cargo line. The architectural reason it stays out: the daemon
owns every writer, and clients map `thumbs.pvcache` read-only.

**Photos does not use the `entry` table.** `files.directory_listing` branches on
`find_parent_directory` and falls through to `query_ephemeral_directory_impl`
when that fails, so with entries empty Photos has been on the ephemeral path all
along. Nothing breaks when entries are removed.

**`parent_uuid` is populated and indexed.** The filesystem indexer writes it on
every record, resolving through `parent_of(external_id)` against the ledger, with
`parents_first` ordering the batch so the self-referencing foreign key resolves.
`idx_record_parent` exists. Nothing in the tree queries `WHERE parent_uuid = ?`
yet except one test in `core/src/ops/indexing/ephemeral/store.rs`.

**`facet_file` carries `size`, `extension`, and `is_hidden`,** on a 1:1 primary
key join to `record`, written by `apply_files` in the same transaction. Sorting by
size is a keyed lookup and denormalization is not needed.

**No existing op can be thinly wrapped for a directory listing.**
`sources.list_items` returns presentation strings. `sources.list_records` returns
`Vec<serde_json::Value>` capped at 2000. `sources.media_listing` returns whole
`File` objects. All three are flat over the source, offset paged, and none takes
a parent.

## The first steps

### Step 1, the two guardrails

Both are small and both stop being small later.

- **`xtask one-gpui`.** Walk `cargo metadata`, assert exactly one package
  resolves to a gpui, that its source is `spacedriveapp/zed` at the pinned rev,
  and that no `gpui-ce`, `bezel-gpui`, or `gpui-unofficial` appears anywhere in
  the graph. Wire it into CI. `xtask/src/main.rs` dispatches on a string match
  at line 90; follow `test-core` for the shape. The failure this prevents is a
  window that paints shapes with no text, which bezel documents and which is
  expensive to diagnose from the symptom.
- **Drop `sd-bake` from `apps/native/Cargo.toml`.** It reads as load-bearing the
  moment `sd-grid` is extracted next to it.

### Step 2, P0, the extraction

No behaviour change. The gate is that `SD_GRID_BENCH=1` reproduces its numbers
from the crate, which is also the check that nothing crossed a boundary it
should not have.

- `crates/sd-grid` takes `grid/`, `source/`, and the tile half of `data.rs`
  intact. `TileSource`, `Bitmap`, and `VisibleRange` move unchanged. That seam is
  already correct: pixels are produced off the UI thread, the grid culls, paints,
  and evicts, and `VisibleRange` is one atomic word so neither side locks.
- `crates/sd-ui` takes `ui/`.
- `crates/sd-tokens` takes `theme.rs` as it stands. The generator is P1. The
  reason tokens are their own crate is that `sd-grid` needs the selection tone
  and the label ink without wanting popovers and comboboxes.
- `apps/native` becomes a consumer.
- Add `gpui-base` to the workspace at `7acfc184`, unused for now, so the pin is
  established before P1 needs it.

### Step 3, P1, tokens generated

Replace the transliteration with the `Brand` input described in the architecture
doc. Lightness is never an input; each tone is tuned once against a measured
contrast ratio and a brand rotates hue while leaving those ratios in place.
Rebuild `Button` on `gpui-base` behavior with sd-ui presentation as the pattern
every later component copies, with the accessibility role written in at the same
time.

## Open decisions

**The facet problem, due in P4.** `record` is universal and facets are per data
type, so `facet_file` will not exist for a mail source. Either
`records.list_children` returns record and content columns only, with the facet
fetched as a second projection, or the op names the facet it wants and returns
nulls where a source has none. The explorer draws size and extension in every
view, which argues for the second. Honesty about `record` being universal argues
for the first. James has not decided. It does not block P0 through P3.

**Daemon round trips.** Whether one round trip per op survives an explorer
issuing many small ops per navigation. Take the measurement in P4 and reach for
keep-alive with request ids if it does not.

## Traps

Six things were gotten wrong in the research and corrected. They are listed
because the same shortcuts are available to you.

1. **bezel was filed as theme tooling** on the strength of a post about a theme
   editor. It is a 37.6k line component library. Cloning it took two minutes and
   changed the conclusion.
2. **Cap was recorded as being on upstream gpui,** from grepping the root
   `Cargo.toml` rather than `apps/desktop-gpui/Cargo.toml`. Cap is on
   `wingleeio/zed`, pinned for a commit about destination alpha on transparent
   windows.
3. **SpaceUI was claimed to map almost directly onto `SemanticThemeTokens`.**
   Reading it showed 165 lines and seventeen colours in shadcn's vocabulary.
   SpaceUI collapses into it rather than mapping onto it.
4. **Photos was claimed to break when entries are dropped,** from reading the
   imports and the indexed SQL in `directory_listing.rs` and stopping before the
   fallthrough. James caught it. Read the whole `execute`.
5. **`size` was designed as living only on `content`,** which would have forced a
   join or a denormalization that `facet_file` already makes unnecessary. Also,
   the `content` join must be LEFT, because a record indexed before
   identification has no `content_id` and a cell with no thumbnail still draws.
6. **A clean tree was mistaken for lost work** during verification, because the
   git status in a session header goes stale. `git fsck --lost-found` found no
   orphans and the commits were already there. Check the log before believing
   something vanished.

The pattern in four of the six: a grep answered the question plausibly and the
next thirty seconds of reading would have answered it correctly.

## Standing rules

- One gpui in the graph, and it is ours. Checked by `xtask` in CI.
- The fork stays patch free for as long as possible. Upstream first, GPUI-CE
  second, a Spacedrive patch third, each changelogged in `Cargo.toml` with its
  reason.
- Third party GPUI component libraries are source material unless they sit on the
  tree we pin. gpui-component qualifies. bezel, gpuikit, and anything on
  `gpui-unofficial` do not.
- Selection lives above the views, in an entity. Grid, list, column, and media
  select the same records, and a user switching views mid selection expects it to
  survive.
- No screen owns state the daemon could answer for. The UI views and controls
  Spacedrive and does not own it.
- Mutating an AppKit window from inside a gpui window update re-enters gpui and
  panics with `RefCell already borrowed`. Retain the native window and mutate it
  from a spawned task.
- A component lands in the gallery the day it lands in an app.
- Accessibility roles are written with the component, never retrofitted.
- Build `sd-ui` against GPUI-CE periodically, so the escape hatch stays open.
- sccache is still not installed. Standing task.

## Working with James

- Never em dashes, and never the "not X, but Y" construction, in anything he
  might publish.
- Do not open PRs or push unless that is the most recent explicit instruction.
  Drafts first.
- No backward compatibility unless asked. This is an early stage project and a
  change is a change.
- Never run `cargo clean` without asking.
- No time estimates in design documents. Ordered phases only.
- Work directly in this checkout on the branch that is already here. No worktrees.
