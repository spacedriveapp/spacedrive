# Zero-Onboarding Startup

> **Status:** Design for first-launch and every-launch indexing behavior
> **Captured:** 2026-08-17
> **Companions:** `docs/plans/2026-08-17-record-table-transplant.md`

## The promise

Open Spacedrive and it is already working. No onboarding, no library wizard, no "add your first location," no scan button. First launch: the app opens into a working file manager and starts building the living index immediately — the whole filesystem hot, the home folders durable. Every later launch: the index restores from snapshots before the window finishes appearing, and reconciliation happens behind it. The only "setup" a user ever sees is the OS asking for permission, in context, when a view first needs it.

Native already proved the shape: on a clean database it indexes discovered defaults (Photos, Notes, Desktop, Documents, Downloads, Pictures, Movies) with no ceremony. This design makes that Spacedrive's default behavior, at filesystem scale.

## Startup profiles

Behavior keys off how the process was started, configurable in `AppConfig`:

```
startup:
  profile: desktop | server | cli | custom
  hot:      [all-volumes]              # ephemeral, watcher-maintained
  durable:  [~/Desktop, ~/Downloads, ~/Documents, ~/Pictures]
```

- **Desktop** (default): hot-index every mounted volume, durably index the home set above. This is the flagship behavior.
- **Server**: nothing automatic — headless installs index what they're told (`sd op sources.register`).
- **CLI**: inert; commands do what they say.
- **Custom**: explicit lists, for people who know what they want.

The durable set is a default, not a decision the user must make — removable and extendable at any time from settings or by right-clicking any folder ("Keep indexed").

## The launch sequence

### Cold start (first ever launch)

1. **Shell opens immediately.** A default library auto-creates silently (no `CreateLibraryModal` on the critical path — the modal remains for *additional* libraries). The Explorer renders the home directory via the ephemeral on-demand path that already exists: browsing a directory with no indexed children dispatches indexing for it, so the first screen is real within milliseconds.
2. **Hot pass starts, home-first.** Whole-volume ephemeral indexing (the machinery in `ops/volumes/index/` today) walks `~` first, then the rest of the system volume, then other mounted volumes — visible-directory-first priority on top, so whatever the user looks at always wins the queue.
3. **Durable pass follows.** The startup profile's home set registers as durable sources and indexes behind the hot pass. Thumbnails appear from the hot tier immediately; sidecars and content hashes fill in as background tiers complete.
4. **The analyzer is live from minute one** — over whatever has been walked so far, growing in real time. Its "not yet walked" remainder is an honest wedge that shrinks, not a progress bar that blocks.

### Warm start (every launch after)

1. Restore every attached source's arena from its snapshot — the app opens *populated*.
2. Reconcile per source in the background (mtime walk against the restored arena, budgeted, visible-first).
3. Watchers re-register per source; detached sources stay listed with cached display, per the living-index lifecycle.

No scan ever runs at startup unless a snapshot is missing, version-bumped, or a bounded recrawl was flagged.

## Permissions are the onboarding (macOS)

A full-filesystem hot pass collides with TCC, and pretending otherwise breaks the promise. The posture:

- **Index everything permission allows, immediately.** The walk treats permission denials as skips, never errors; denied subtrees are recorded as *ungranted*, not empty.
- **Contextual prompts, never a wizard.** First touch of `~/Desktop`/`~/Documents`/`~/Downloads` triggers the OS's own per-folder prompt exactly when the user opens or the indexer reaches them — the OS dialog *is* the onboarding.
- **The analyzer sells Full Disk Access honestly.** Where DaisyDisk shows an "unscanned" wedge, Spacedrive shows an **"ungranted"** wedge sized to what TCC is hiding, and clicking it calls `sd-fda`'s `DiskAccess::request_fda()` (the crate exists) to deep-link System Settings. Granting it visibly fills the chart. No nag screens; the missing data argues for itself.
- Removable volumes generally carry no TCC restriction — external drives hot-index fully without any prompt, which makes the first plugged-in drive the best first impression.

## The hot index seeds the durable index

The hot index is the **required first stage of all indexing** — nothing writes a durable store except from the same observations that fed an arena. There is no second walk, ever:

> **Amended 2026-08-18** (`docs/plans/2026-08-18-storage-consolidation.md`, contract 3): the arena is a cache, not the durable event source. The connector's observation stream fans out to the arena and to the source store; the diagram's "durable drain" is that second consumer, fed the same batches. Both invariants below — one walk, one continuous identity — are unchanged.

```
walk / watcher ──▶ arena (hot, per source)
                     │
                     ├─▶ rollups, views, UI events
                     └─▶ durable drain (source store)   for subtrees marked durable
```

- **One walk, one ordering, one pipeline.** The volume walk covers everything including `~/Desktop`; durability is a property of a *subtree within the source*, and the drain streams that subtree's arena entries into the source's record store — initially as a bulk drain behind the walk front, thereafter incrementally from watcher deltas the arena has already absorbed. Double indexing is structurally impossible, not merely avoided.
- **This dissolves the overlap problem.** Desktop and Downloads are not separate sources competing with the volume source — there are no overlapping sources at all. A source is a physical root (a volume, a drive, an explicit mount); durable subtrees live inside it. The prefix-routing ambiguity, the `dp.path LIKE` leak, and the shadowing rule this section previously proposed all evaporate.
- **Identity is continuous by construction.** The arena entry's uuid v7 is the record uuid the drain persists — the "ephemeral uuids preserved on promotion" behavior the docs claim (and nothing implements) becomes true structurally rather than by bookkeeping. One identity from first sight of the file to its durable record.
- **Marking a folder durable is instant.** "Keep indexed" on any folder = flag the subtree + drain what the arena already holds. No walk, no progress bar — the data was already hot.
- External drives are simply sources whose durable subtree is the root (fully durable), or nothing (hot-only browse), per placement policy.

## What has to change

Mapped against the transplant plan's workstreams; items 1–4 are new scope.

1. **Silent default library.** First-run auto-creation, no modal on the critical path.
2. **Startup profiles in `AppConfig`** with per-app defaults (desktop on, server off, CLI inert).
3. **The ephemeral memory diet — the big one.** The arena is ~50 B/node, but the side tables dominate real memory (`detailed_memory_breakdown`, `index.rs:529`): `path_index: HashMap<PathBuf, EntryId>` and `id_to_path` store full owned paths per entry. At whole-filesystem scale (3–5M nodes) that is gigabytes of `PathBuf`s for data the tree already encodes. Paths must be resolved by walking parent chains (names are already interned) with a bounded LRU for hot lookups; the stored path maps go. Without this, "hot index the entire filesystem" is a memory bug with good intentions.
4. **Politeness envelope.** First-run walk at low I/O priority, background hash/thumbnail tiers deferred until the walk settles, throttle-on-battery. The first five minutes must not spin fans — "feels like a feather" is a startup requirement, not a rendering one.
5. **Lifecycle wiring** (already on the plan, B3 + living-index gaps): per-source arenas, dump-on-quit/restore-on-launch, staleness metadata, the ephemeral reconcile detector, snapshot re-keying by source id.
6. **Workstream B's sink composition refines into a pipeline.** The ephemeral sink is not one option among sinks — it is mandatory and first; the record table sink consumes from the arena (bulk drain + watcher deltas), never from the walk directly. The durable pass at startup is just the profile's home subtrees being flagged durable on the volume source.
7. **Rules become lenses, not walls.** Today the ephemeral walk applies the same `RuleToggles::default()` as persistent indexing (`job.rs:129/143/161`), which rejects system files, git internals, gitignored files, and dev directories — so the "whole volume" hot index silently omits `node_modules`, `target/`, and `.git` stores, frequently the majority of a dev machine's used disk. An analyzer on that index under-reports catastrophically. The change: **the walk records everything; rules decide what surfaces.** Nodes gain a flags byte (hidden / system / dev-dir / gitignored / cloud-dataless); Explorer defaults to the filtered lens with a show-all toggle; the analyzer sums raw; the durable drain applies rules as persistence policy (excluding gitignored noise from the record table store is fine *because* the hot layer still counts it). Gitignore is the expensive flag — evaluate it lazily at drain/view time rather than slowing the walk. The sweep's filter fixes land here too: the `.tmp` substring bug and the dotfile rule that currently makes `.spacedrive/` invisible to its own watcher. macOS nuance: the sealed system volume is a read-only snapshot — walk `/System/Volumes/Data`, report the sealed volume as a fixed quantity.
8. **TCC-aware walking**: denial-as-skip with *ungranted* accounting surfaced to the analyzer, plus the `request_fda` affordance.

## What this replaces

Any onboarding flow, library-creation gate, or "add a location to begin" empty state on the desktop app's first run. Locations-as-a-concept survives for explicit user intent (network mounts, unusual roots, servers); the default experience simply never requires expressing intent to get a working product.

## Open questions

- Whether the hot pass includes other *users'* home dirs when FDA is granted (lean no — system volume yes, foreign homes opt-in).
- Durable default set per platform (Windows: Desktop/Downloads/Documents/Pictures; Linux: XDG dirs).
- Drain scheduling: how far behind the walk front the durable drain runs (lean: drain in large batches once a subtree's walk completes, so record writes are big transactions rather than trickle).
- iCloud Drive / OneDrive / Dropbox folders: dataless files must not be materialized by the walk (stat-only, never open) — needs an explicit test, materializing a user's cloud drive on first launch would be catastrophic.
