# Project Status

> Last updated: 2026-10-07
> Code baseline: `cedc8d6`
> Branch of record: `main` (fast-forwarded from `sources` on 2026-10-05)
> Purpose: short-term working context across development sessions while
> `.tasks/` is frozen.

Read `AGENTS.md` first for repository rules. Read this file next for the current
state and immediate work. Design documents own rationale and long-range phase
plans. This file owns what is true now, what is being worked on, and what the
next session should do.

The product boundary lives in
[`docs/core/product-direction.mdx`](docs/core/product-direction.mdx). Spacedrive
is open personal data infrastructure that makes owned storage competitive with
cloud convenience. It protects a person's ability to choose where files and
metadata live as automated attacks and intelligent systems increase the cost of
centralized custody. Its cross-platform filesystem runtime provides the same
operations across local files, remote nodes, drives, and optional clouds.
Reliability, lightweight distribution, extensibility, and user ownership decide
scope.

## Current focus

NAS operating state (its datasets, deploys, backups and evidence) is
kept privately in the company repo. This file records engineering state.

Development focus is the product sequence below, starting with running
Spacedrive on the Mac and operating a paired NAS's sources from it.

The drive-catalog and physical-drives plans are committed (`2d380fd26`), but
the early `packages/drives` implementation described by previous handoffs is
no longer in the checkout. See the register row before restarting that work.
Physical-drive identity remains separate from product recognition and visuals.

## Immediate register

File System Intelligence is a flagship priority identified by James (Codex,
September 18). The product boundary now names persistent file and folder
understanding as a first-class user benefit, with optional cloud acceleration.
The adjacent website's `RELEASE_DIRECTION.md` records a positioning and demo
proposal: understand an unfamiliar drive, find by meaning, retain user context
and keep the saved understanding when disconnected. James chose the name
File System Intelligence and asked to retain the original website hero. Final
copy and supported release scope remain for review. Website presentation is
implemented on the adjacent `beta-release-direction` branch: `/intelligence`, a homepage
section and an interactive sample-drive walkthrough. TypeScript, the production
build and browser interaction checks pass, including mobile layouts and the
restored original hero. No intelligence runtime changed.


Website release direction is captured in the adjacent `spacedrive-web`
repo (Codex, September 17). James's existing rebuild is committed as `74ebe73`
on the new `beta-release-direction` branch. A stand-in 2.0 Beta blog post and
`RELEASE_DIRECTION.md` carry the agreed model commitment into the planned
redesign. The production build passes; HTTP checks verify local draft rendering
and a production 404, with no draft in the blog index or static routes. The
site's false stable-v1 label is corrected. This work does not publish the site
or change core runtime code.

Release identity agreed with James (Codex, September 17): the target is
Spacedrive 2.0 Beta. The alpha period established the direction; beta commits
to the source/library data model and preserving supported beta libraries as
schemas evolve. Keep the 2.0 generation and explain the skipped stable 1.0 and
the rewrite honestly in the launch post. The README and release policy now
name the beta target; `docs/plans/2026-09-17-beta-launch.md` records the rationale
and draft launch wording. The target moved from October 1 to November 1, 2026
on 2026-10-05 (Jamie). This is a release goal, not a claim that the gates
have passed. No package versions, tags, builds or published releases changed.

Alternate shell composition is implemented (Codex, 2026-09-15, uncommitted).
Inset content is the default; Settings > Appearance > Layout retains Floating
panels. The sidebar footer restores circular Activity and Settings controls,
including the activity count and busy indicator. Both modes share mounted
content and stable preview hosts. The local
preference updates other windows on the same origin. Preview and Size view
follow the sheet bounds, and Size thumbnails use container coordinates. Desktop
and web frontend builds pass. Typechecking reports no errors in the changed
files, but the full check still fails on existing dependency and SVG
declarations. Browser control was unavailable in this sandbox. Next proof:
switch layouts from Settings while browsing and playing video, then check Size
view, collapsed panels, inspector popout, light themes, and macOS window controls.

Source read routing is agreed: keep a loaded arena as the fast path when its
coverage and query support suffice; otherwise read SQLite. Select one primary
backend per source request, preserve the same result contract, and never retry
an empty result against the other backend. R6 in
`docs/plans/2026-09-15-source-runtime-reliability.md` records the decision,
including direct local reads before peer database delivery.

September 16 volume audit (Codex): the arena already saves snapshots without
source registration. On the running Mac, Untitled is detected but unmapped and
unwatched, while an older file exists at its expected snapshot path. Startup
maps attached external drives; the hot-plug monitor does not invoke that path.
Do not describe Add to Library as the sole way to persist a filesystem index.
See `docs/plans/2026-09-16-volume-discovery-research.md`. Checks were read-only.

Add to Library UX direction is agreed (Codex, September 16): use that action
everywhere, with one modal, configurable defaults and per-add overrides. Adding
a filesystem path also tracks its containing volume in the selected library;
adding the whole volume uses the same flow with root scope. Untitled remaining
untracked is expected, not a request to auto-add discovered drives. The user
chooses whether the source store lives in the Spacedrive data folder or on the
source under `.spacedrive`. `docs/plans/2026-09-16-add-to-library.md` records the
decision and proposed offline-copy, storage-resolution and setup behavior.
Implementation and the proposed defaults remain for review.

Remote catalog access is agreed: the source's Spacedrive instance opens the
store and serves or replicates it through Spacedrive. A client opens only its
local replica, never the remote live SQLite file through a network mount.
Store placement does not change this access contract.

Add to Library implementation research is recorded in that plan (Codex,
September 16), through `4cf25c17c` plus identified uncommitted database-export
work. Reuse the new read-only store path and exports. Remaining foundations:
consistent volume registration/defaults, library-scoped store resolution,
portable source identity, handle retirement, unconditional managed-directory
exclusions, and removal that preserves portable catalogs. Full offline store
copies are distinct from existing saved arena maps. On-source WAL databases
need supported local storage on their serving daemon. No runtime code or fleet
state changed; placement defaults and handoff/consolidation remain for review.

Library sync and membership research is ready for review (Codex), recorded in
`docs/plans/2026-09-15-library-sync-research.md`. The live Mac and NAS are paired
but hold different library UUIDs, each with one device row and zero eligible
sync peers. Overview storage totals include peer summaries while device counts
stay local. The sync service exists despite the false library setting; source
registration sync, reliable bilateral membership and library-scoped serving
remain gaps. Review the proposed small library-state contract and migration of
the two existing libraries. Evolve the existing `Syncable`/inventory framework:
retain domain-owned apply rules and shared transport, remove entry-specific
coupling, and repair membership and write durability. This research changed
no live membership or config.

The UI setup audit confirmed `SyncSetupModal.tsx` survives without callers:
`b653676e8` removed the overview's Setup Sync button. Pairing still closes
without library setup, and Library Settings only exposes the sync toggle.
The library-sync research now includes restoring setup entry points,
continuing after pairing, and resuming setup for already paired devices.

Locations research is recorded in
`docs/plans/2026-09-15-locations-research.md` (Codex). James confirmed this version
is not backwards compatible. Remove locations without migrating rows, retaining
IDs or adding compatibility APIs. Update current callers to sources, Space items
and policies. The audit found Add Storage calling the pin operation, incomplete
pin sync and duplicate Space shortcuts. Both live libraries returned no pins
during read-only checks. This audit changed no runtime code or live state.

The locations proposal was revised after user review: pins alone duplicate
Space items, while the old requirement to keep selected jobs processing a path
is still needed. T4.4's subtree enrichment-policy replacement has not landed.
James agreed to option 1 for addressing: policy creation accepts `SdPath`,
persists a separate `{ source_id, relative_path }` target, and resolves it to
an execution address through the source layer. Add no source variant to
`SdPath`. Space items own navigation and can reuse the target value. Reuse the
source change stream and job executor. Detailed policy storage and APIs remain
for review; the final-drop handoff has no location compatibility or migration
gate.

Tags on source stores is implemented through P5 (Fable, 2026-09-17,
uncommitted). Definitions and assertions live in each source store per
`docs/core/design/tags-and-assertions.md`; the library keeps only a staging
table for unapplied definitions and the assertion outbox. The old semantic
tag surface (`TagManager`, closure and conflict services, the facade,
`UserMetadataManager`, `ops/metadata`) is deleted, the slim `Tag` is
uuid/path/name/color/icon, and every read serves from stores directly, so a
cold daemon and a detached source both answer. Writes against replicas
owned by another device queue durably, overlay the replica listing
immediately, and deliver on the owner's ack. No legacy migration: zero tag
rows verified on both libraries.
Execution record and remaining P6 items are in
`docs/plans/2026-09-17-tags-on-source-stores.md`. This is source-store
assertion sync; library sync stays deferred.

Path-scoped thumbnail generation is implemented and verified (Codex, 2026-09-15, uncommitted). `thumbs.generate` accepts a file or indexed folder with missing/stale/force modes, uses resumable batches, and versions video tiles against host FFmpeg availability. File and folder context menus plus `sd thumbs generate` call the same operation. Ten focused tests, core/CLI/server/Tauri checks, generated types, CLI help, and the desktop frontend build pass. Changed hooks have no TypeScript errors; the full interface typecheck still fails in existing dependency and SVG declarations. Restart the daemon and reload the app, then regenerate a folder containing an old video icon to verify the live repaint. See `docs/core/thumbnail-generation.mdx` for coverage and resume semantics.

Update this table whenever work starts, lands, becomes blocked, or changes
owner. Keep only active and near-term work here. Move completed context to
Recent landed.

| Work                                              | Status                                 | Owner           | Next proof or action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------- | -------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source runtime reliability | R1-R3, R6, R7 and the R8 acceptance matrix landed | in progress | Execute `docs/plans/2026-09-15-source-runtime-reliability.md`. Landed: destructive paths removed, flush made a durability barrier, self-nudge loop stopped and hashing progress labeled (`bb033de1b`, `954c6d980`, `fb0dab3e6`); R1 ancestor synthesis (`9242a14a1`); R2 anchored volume roots (`0aa0d390c`); R3 persisted replica inventory (`59b05a6c8`) plus the fetch-generation fix (`98e8a1485`: header names the generation and blake3 of the bytes delivered, a delivery must parse before it replaces a good artifact, one failed source no longer aborts the device sync, fetch failures back off, an unchanged generation still refreshes owner facts, revocation unloads and quarantines replicas, snapshot decode is bounded); the coverage heal (`5a65d0998`); watchers made unconditional on a bare daemon (`8c8499413`, `847144ba0`, `3ee5449bf`); dirty-driven refetches paced to five minutes (`4a690c84a`). R6 (results in the plan): `9e29e7c46` opened stores read-only with listings, lookups and Unicode-folded name search; `4cf25c17c` routes one backend per source (arena when restored or walked, SQLite otherwise, no empty-result retry, `total_is_exact` on the wire, detached directories list from their stores); `5f1fdd287` delivers a nested source's replica as its own database with the snapshot header contract; the 100-store/1M-record cold fan-out measures 1.7 s (`33bd1ce5d`). R7 (results in the plan): stores validated by direct SQL with zero dangling ancestry, a cold restart with the owner down serves and searches every replica, byte read, remote op and remote log stream verified, and reconvergence transfers zero bytes. Since then: musl builds use mimalloc as the global allocator, since musl's allocator gives each allocation group its own mapping and exhausted the kernel's map limit within an hour of a restart; a nested source's generation is the store's revision, a count of committed row changes kept by triggers (`crates/store/src/revision.rs`), so a drained WAL no longer causes a full re-copy; the file type registry is built once per process, taking arena inserts from 3.00 ms to 7.2 µs and a large replica restore from 56 minutes to 19 s; video thumbnails scale ahead of the `thumbnail` filter and the decode pool is bounded by free memory (`bf1ca21a1`, `4d18db0ab`); the arena's backing file is unnamed, so the kernel reclaims it however the process ends (`f5bf69d60`). Tiles cross to replicas over `FetchTiles` (P6 of `docs/plans/2026-09-19-incremental-replication.md`), and every baked tile is also kept as WebP in its source's `sidecars.db`, which replicas copy page by page over `FetchSidecars` (P7, L5's first sidecar kind). R8 matrix landed as `docs/core/acceptance/source-runtime.md`: all 28 rows of the plan's table mapped to CI tests on the Linux runner; 21 rows pass outright, 3 have a passing and a failing half (3, 10, 25), 2 fail outright (11, 17), row 9's APFS half and row 26 are not automatable on Linux; the five failures stayed in the tree as ignored tests (`#[ignore = "R8: ..."]`) as fix briefs. Four are fixed since: a salvaged-away row is unsettled in the ledger so the repeat observation rewrites it (F1), an unreadable snapshot moves to `<name>.corrupt-<timestamp>` instead of being deleted (F2), a watch registers only after the OS accepts and a refusal is reported with its reason on `core.index_status` and retried every 30 s (F3), and store-backed library search de-duplicates nested stores by path, innermost owner first, before counting (F5). Row 17, arena slots growing with clear/refill history, remains the R4 brief. New suites: `source_runtime_acceptance_test`, the two-process `source_replication_test`, and the `sd-store` crate suites, as the `acceptance` job of `core_tests.yml` (`xtask test-core --acceptance`), since the integration job already runs an hour cold on Blacksmith. Remaining: the restart refetch (a replica publishes its share only after its arena rebuild, so the first sync pass refetches what it already holds; P1 of the replication plan), R4 compaction and memory honesty (the last R8 failure), R5 typed health and coverage, the R8 docs sweep, delta replication and transfer cleanup (its `temp_store` decision is James's), plus R6's follow-ons (request cancellation, the deferred title index). |
| Drive catalog H0                                  | plans committed, code missing          | unconfirmed     | The drive-catalog and physical-drives plans landed in `2d380fd26`, but the uncommitted `packages/drives` package earlier handoffs said to preserve is no longer in the checkout. Confirm whether that code survives elsewhere before restarting the package and archetype contract from the plan. The physical-identity firewall still applies.                                                                                                                                                                                                                                                                                                                       |
| Final entries drop                                | FD0–FD5 done; FDA matrix landed, all rows pass on Linux | Opus            | Execute `docs/plans/2026-09-15-entries-final-drop.md`. FD0 through FD4 are done (2026-09-18): no production code reads the entry tables, legacy row sync is gone, and a transactional migration drops 29 retired tables so every library converges on a 14-table schema. Both live libraries upgraded after backups. Locations are gone end to end: pins are Space items, Places come from `paths.system_folders`, and Add Storage tracks sources. FD5 is done: the ephemeral qualifier is retired (`VolumeIndex`, `Partition`, `Arena`, `core.index_status`), entry-era tests and examples are deleted or repointed, and the persistent-era search, event, status, and error surface is gone. FDA landed as `docs/core/acceptance/entries-drop-and-file-operations.md`: every row of the Acceptance section mapped to a CI test (`core/tests/entries_drop_acceptance_test.rs`: fourteen-table schema on a fresh and on a pre-drop library, no production reference to the retired substrate, registered sync models name current tables, pins are Space items with no indexing side effect and survive a restart, tags survive a reindex, a frozen store is self-describing and unwritten by a reader, an incompatible library is refused intact, non-UTF-8 names retained lossily and warned). The two rows that failed when the matrix landed are fixed: Linux trash restore records the item under `Trash/files` and takes the `.trashinfo` with it on restore and purge, and a rename over an existing file removes the overwritten row in the batch that lands the move (the matrix's F-a, F-b). `KeyManager::close` no longer creates a redb file named `:memory:` in the working directory, so the pin restart row runs in process. Not automatable on the runner: a pin following a volume remount, remote job watch and log stream (live in SPAC-12), the `wasm` and `sd-native` check lines. Follow-ups are in its record: redundancy over source stores, and `indexing.start` walking the caller's path spelling. Preserve source stores, paired-source replication, remote byte reads, remote operations, jobs, and logs. |
| Tags on source stores (FD2) | stretch 1 complete (Fable, 2026-09-18) | Fable | Landed in five commits through the outbox delivery fix. Verified against a paired NAS: queued outbox batches delivered on reconnect, acked and retired, the owner's stores answer `files.by_tag`, and both libraries' legacy tag tables verified empty, clearing FD4's preconditions. Search phase 4 landed: store-resolved include/exclude proven exact by CLI, replica hits removed under a tag filter, Tags advertised, and a picker pill in the refinement bar. P6 done across `tagging.mdx`, `library.mdx`, the design doc, and the stats tag count. |
| Search refinement                                 | phases 1 and part of 2 committed       | in progress     | Execute `docs/plans/2026-09-15-search-refinement.md`. Landed: `af3470373` made sort, pagination, and filters honest (pipeline module with the R6 backend-neutral candidate stage, true `total_found` past the old 200 cap, case-folded extensions, hidden excluded by default, date filters fail closed; verified live with `--limit`, sorts, and offsets), and `f34fc3c8a` fixed the debounce, preserved the picked scope across keystrokes, unified the length gate, and wired Cmd+F. `7733fe8a9` replaced the Location scope with a working Source scope resolved through `paths.context`, and `81accc3ad` plus `3f4b67100` built the refinement bar: kind, extension, size, modified-date, and hidden filters with facet counts folded over the full match set and a true result count, recomposed as one row of `@spacedrive/primitives` (ToggleGroup scope, SelectPill popovers, Switch) where each pill carries its own state. Phase 4 tag filtering landed 2026-09-18 (store-resolved include/exclude, replica honesty, a Tags pill; results in the plan). Remaining: per-tab search state and search as a navigation target (deferrable no longer now that the shell work landed), and phase 5 redundancy-view verification. Inspect the live panel, then consider consolidating onto ts-client `useSearchFiles`. |
| Sidebar simplification                            | first slice committed (`2097fc141`)    | unowned         | The sidebar has one library scope, flat daily destinations, Places computed from system folders without persisting them as locations, canonical rows, and an Activity/Settings footer. Topology groups retired; their pins survive seed convergence, and replica sources render under their owning device. Next: inspect the live library, then refine Home and Storage ownership from real use.                                                                                                                                                                                                                                                                    |
| Explorer path status                              | visual polish built, uncommitted; live proof pending | unowned    | `paths.context` distinguishes source-database existence from a committed record for the exact path. The PathBar uses neutral surfaces, floating Spacedrive source and volume assets, unfilled status chips, and large 36px circle actions for pinning, full-source reindex, navigation, and technical details. All explicit small CircleButton usages in Spacedrive are removed; the shared variant definition lives in the separate SpaceUI package. Pins rebuild from the registry's mounted source root. Earlier generated-client, focused Rust, scoped PathBar, and Fumadocs checks passed. The desktop frontend production build and scoped diff check pass after the styling change. Inspect the panel live. |
| Virtualized thumbnail remounts                    | committed (`4e53d58db`); live proof pending | unowned    | Hot-tier bake completion epochs now survive cell unmounts, and each thumbnail hides its fallback only after its current DOM image loads. Media rows now keep stable React identity as the virtual window advances, width is measured before paint, content is no longer deliberately invalidated for every render, and live row-count changes no longer snap the camera roll to the bottom. The desktop production build passes. Scroll and trigger selection or inspector rerenders in a large media view to confirm the grid remains stable.                                                                                                  |
| Quick Preview originals                           | committed (`4e53d58db`); live proof pending | unowned    | The Tauri asset boundary now maps canonical macOS APFS data-volume paths back to the scoped `/Users` and `/Volumes` aliases. Image, video, audio, text, mesh, and local thumbnail-original renderers share the fix. Open a home-directory image and an external-volume file to confirm originals load without a 403.                                                                                                                                                                                                                                                                                                                                                 |
| Permission-aware automatic startup                | first slice committed (`c32768f2e`)    | unowned         | Library loading no longer dispatches a new filesystem discovery pass. The desktop requests one idempotent pass after the connected shell renders; a bare daemon does not initiate discovery, and `sd index start --defaults` gives CLI and server operators the same explicit path. Restored volume snapshots now prevent repeat full walks. Next: model denied coverage and add the macOS permission explanation and Full Disk Access affordance.                                                                                                                                                                                                                   |
| Host media tools                                  | committed (`f3a1ede2f`); live proof pending | unowned    | The machine-scoped registry discovers host FFmpeg outside GUI PATHs, reports its version and media capabilities, and exposes explicit Homebrew/WinGet installation. Services settings now shows the resolved version, path, capability coverage, missing FFprobe state, refresh, and a confirmed package-manager install action. Default builds use QuickLook or host FFmpeg for hot video thumbnails and generate immutable 5 by 5 thumbstrips on hover. Core, linked-feature, server, Tauri shell, generated-client, and desktop production builds pass; real FFmpeg output is 384×216 and 800×450 in the focused fixtures. Restart the daemon, inspect Services settings, then open a video with no old sidecars and confirm its poster and hover scrub sheet. |
| Opening remote files                              | O1, O2 (desktop), O4 (macOS) and O6 (Photos) landed 2026-09-24, uncommitted; live proof on the NAS next | Opus            | Plan: `docs/plans/2026-09-21-opening-remote-files.md`. Quick Preview streams another device's originals from `files.stream_url`, the mounts share's loopback URL; Open and double-click open them through `files.local_path`, which mounts the SMB share at `<data-dir>/mount` with NetFS and returns the file's path inside it; Photos follows a folder on another device (`search.media` pages replicas from their index) and shows a cell in Quick Look through the mount. The mount unmounts at shutdown, a killed daemon's mount is removed at the next start, and volume detection skips it. The SMB frontend served one file's bytes under another's name after a listing (every file id was 0 and search patterns were ignored); both are fixed and covered by tests. The HTTP share refuses non-loopback Host headers and answers CORS only for Tauri and loopback origins; no share key, since RPC on 6969 is open to local processes anyway. Verified end to end on two paired throwaway daemons. Next, on the Mac against the paired NAS after restarting the Mac's daemon on this build: preview and scrub a remote video and read `mounts.cache_status`, Space on a remote clip in Photos, and open a remote clip larger than free disk in QuickTime. Open: O3 open by copy, O5 settings, the web client's `sd-server` route, Open With for remote files (needs a submenu that loads when the menu opens), and Show in Finder on sidebar items, which still takes a path unchecked. |
| File operations on preflight                      | F1 through F7 landed 2026-09-23; CI acceptance landed, Linux trash restore failing | unowned | Plan: `docs/plans/2026-09-23-file-operations.md`. Delete and duplicate in the explorer on preflight; rename and batch rename on preflight with filesystem name rules; the operation journal every mutating job writes, the trash with recorded locations and a Spacedrive trash directory where a volume has none, stashing on replace, and `files.undo` on preflight reached from the job list, ⌘Z and the trash view; mirror as `remove_extras` on merge; organize and flatten; zip and tar.zst archive and extract; attributes and links. CLI commands and dialogs for each, 30 tests over the fixture. The plan's acceptance case is now CI tests in `core/src/ops/files/acceptance.rs` (matrix: `docs/core/acceptance/entries-drop-and-file-operations.md`): a dated batch rename runs as previewed and undoes, a mirror's deletes equal the reversed path comparison, every file a job touched is in its journal, a 1,000-file batch rename previews under the ceiling. Failing on Linux and Windows, ignored: trash restore and mirror undo, because `trash_os` records the `.trashinfo` path as the item's location and `restore` renames it over the original (matrix F-a; half a day). Still live only: trash on a network mount, the preview-time measurement on the largest folder. |
| Action validation, preview, and folder merge      | V1 through V9 and dedupe landed; CI acceptance landed, NAS run still open | Opus            | Plan: `docs/plans/2026-09-22-action-previews.md`. Preflight (`validate:` and `preview:`), `FsPlan`, copy, move, merge, and delete on both methods, the merge job, dedupe as a delete target (first copy in walk order or chosen copies, read in full before removal) with `sd file dedupe` and a Duplicates screen, `sd file copy`, `merge`, and `delete` rendering preflight, the merge dialog, the modifier drop, the preflight-driven operation modal, plan handles, and overlay listings with the explorer's preview mode; the old validation hook is gone. Remote execution rides the existing `--device` forwarding. Two opt-in preflight methods on actions, both taking the action's exact input: `validate:` answers whether and how an operation runs (findings with stable codes plus execution facts, re-run server-side at dispatch, errors refuse) and `preview:` answers what will exist afterward (`FsPlan` from the index, advisory, browsable through plan handles as an overlay on `files.directory_listing`). The plan catalogs both methods across copy, move, merge, delete, batch rename, and dedupe. Folder merge is the first action on them: recursive merge with integrity-confirmed duplicate skipping, per-leaf conflict policy, optional source consumption, remote execution via `--device`. Also fixes copy's silent blind overwrite inside directory collisions, deletes the CLI's local conflict logic per the operations-are-the-unit principle, and removes the dead `ValidationResult`/`ConfirmationRequest` hook. Six decisions are James's. Landed ahead of preflight: `files.delete` takes a comparison as its targets (`sd file delete A --against B --show both`); the job derives the set from the index as it runs, reads both copies in full before removing one, checkpoints the compare cursor, and reports skips. The plan's acceptance clauses that a fixture can prove are CI tests in `core/src/ops/files/acceptance.rs` (matrix: `docs/core/acceptance/entries-drop-and-file-operations.md`): preflight makes no filesystem write and moves no store revision across six operations, copy/move/merge/delete previews project exactly the files their jobs leave, no destination file changes without a `Replace` row, and dedupe keeps a copy whose bytes differ from its keeper (with SPAC-19's `dedupe_own_hash_test` for the store side). Still live only: the preview-time measurement on the largest source and the consuming merge of the Expansion drive into `jamie-nas`. |
| Addressing                                        | P0 and P1a landed 2026-10-06; P1b, P1c and D1 to D6 open | unowned | Plan: `docs/plans/2026-09-25-addressing.md`. P0 rewrote `docs/core/addressing.mdx` to match the code. P1a landed: source stores carry a schema version in `PRAGMA user_version`, migration 1 rebuilt `content` so a row's integrity hash belongs to every record on it (candidate rows by sampled hash, confirmed rows by integrity hash, `candidate_uuid` beside `uuid`), and compare-delete and dedupe now read a hash a file's own bytes produced. Upgrade note: an existing store migrates to `user_version` 1 on its first writer open (daemon start or source attach); the rebuild is one transaction and measured 11 s for 1M content rows and 32 s for 3M in release on a cloud machine, so a 3M-record MacBook pauses that source for well under a minute once. Every row that held both hashes loses its integrity hash, so dedupe and compare-delete re-read each shared file once afterwards. Replicas keep version 0 until their owner upgrades and are read in their own shape. Next: P1b, network admission for fully paired devices only; P1c, refusals for addresses destructive operations can't honor (cross-device moves wait on D6); then P2 to P6 behind D1 to D6. |
| Locked and unmounted volumes                      | L1 and L2 landed 2026-10-07 (Fable, SPAC-25) | Fable           | Plan: `docs/plans/2026-09-28-locked-volumes.md`; acceptance: `docs/core/acceptance/volumes.md`, run on a loop-mounted image in the acceptance CI job. L1: the monitor marks a tracked volume offline when detection stops returning it, `attach_library` resolves against live detection and corrects a stale row, the volume index carries each mapped drive's mounted state, and a source whose volume is away comes up detached with its map restored read-only and no watch armed, reattaching when the volume returns. L2: attached means mounted; a mount point check (device differs from the parent's) runs before any walk, hash or thumbnail job and inside the jobs themselves, so the directory an unmounted drive leaves behind is never walked, a forced heal dispatches nothing, and `sources.track` on it refuses. Next: L3 reads ZFS `mounted` and `keystatus`, L4 follows mounts while running (the index already follows the monitor; the watcher does not yet re-arm on remount), L5 shows the state; L3 and L4 need a ZFS pool. One decision is James's: whether a locked source's index stays browsable; the default built keeps it browsable, labeled detached. |
| Retire Spacebot, Console, and storefront surfaces | done (2026-09-25)                      | Opus            | The Spacebot UI, voice overlay, `spacebot` app config, window, build aliases, generated types, and design docs are removed; the Alt+Space global shortcut and its plugin went with the voice overlay. Console and Pod design docs are removed. Private copies are in `company/archive/spacedrive-sources-public-cleanup/`. No storefront code remained. |
| CI green on `sources` | landed 2026-10-04 (Fable, SPAC-4); Rust Formatting, Clippy and TypeScript green on #3102 | Fable | The three `ci.yml` jobs were red on `sources` before any PR: the self-hosted runner's preinstalled toolchain had no rustfmt or clippy (`setup-rust` now installs both every run, and `rust-toolchain.toml` names them), `maximize-build-space` left the Blacksmith runner's root disk with 100 MB so no toolchain could install (step removed), the clippy job ran on GitHub's ubuntu-22.04 whose FFmpeg 4.4 cannot build `sd-ffmpeg` (now `blacksmith-4vcpu-ubuntu-2404`), and the server and desktop crates failed to compile without a built web UI (debug builds now get an empty dist with a warning; release builds still refuse), `cargo clippy --workspace -D warnings` failed in seven crates plus the Tauri sidecar check and the two platform-only file-opening crates, and `bun run typecheck` in `apps/tauri` had 30 errors in `packages/interface`. All fixed with no product change: sqlx row aliases in `sd-store`, exact casts and raw FFI borrows in `sd-ffmpeg`, `DaemonRequest.device` in `sd-mobile-core`, target-gated `file-opening-macos`/`-windows`, a Tauri build script that drops `externalBin` when no daemon sidecar exists, unused imports and `SourceInfo.adapter_id` nullability in the interface, the `/redundancy/duplicates` route the dashboard already linked. `core/tests/delete_strategy_test.rs` compiles again (calls `ops::files::trash::trash` directly), the 13 stale doctests compile, `scripts/setup.sh` installs the libav dev packages, bun is pinned to 1.3.4 with `--frozen-lockfile`, workspace `rust-version` is 1.95 (gpui's floor). Still red and out of this scope: 20 integration-test failures on a bare Linux VM (see SPAC-2); the three unit-test bugs have their own row below. Next: confirm the three jobs green on the PR, then decide whether `core_tests.yml` should run on pull requests. |
| Unit-test bugs on `sources` | fixed 2026-10-04 (Fable, SPAC-5); `cargo test -p sd-core --lib` green five runs in a row with default parallelism | Fable | Three bugs the SPAC-2 baseline found. The btrfs parser read the filesystem UUID from a line starting with `uuid:`, but `btrfs filesystem show` prints it on the `Label:` line, so every Btrfs volume failed with "Could not parse Btrfs UUID"; the header line now yields both label and UUID. The plus-code parser rejected its own encoder's output and Google's `WR2C+2C Bibra Lake` form (its alphabet check demanded every character equal every alphabet digit); `PlusCode::is_valid` now follows the Open Location Code validation rules, the encoder is unchanged. The `ops::files` fixture flakes were not an FTS5 or temp-path problem: `LibraryManager`'s directory watcher auto-opens a new `.sdlibrary` folder 500 ms after it appears, and under load `create_library` was still initializing it, so two openers migrated one database at once (`seaql_migrations` UNIQUE failures, `vtable constructor failed: search_index`, `AlreadyInUse`). Paths a create call is building are now registered and the watcher leaves them alone. Next: the integration-test triage (needs a Mac for the pairing suites). |
| Linux integration-test triage | in progress 2026-10-04 (Fable, SPAC-6); 7 of the 13 red `core/tests` targets green on a bare Linux VM | Fable | Each of the 20 failures from SPAC-2 was reproduced on 61dea60 and classed. Stale assertions fixed with no product change: `library_test` expected a `sidecars` dir that ba8efcd stopped creating (now checks `previews` and `exports`), `event_filtering_test` listed the subfolder itself as a "subdirectory only" path (exact mode matches direct children since cfa31cc), `sync_event_log_test` read the newest-first query as oldest-first and raced the startup pruner, `sync_metrics_test` assumed no change arrives before the first explicit action (library open broadcasts the device record), the two volume capacity tests compared ext4 usable bytes to the image size. Environment assumptions made hermetic: `resource_events_test` indexes a seeded temp dir instead of `~/Desktop` and asserts on the file batch, `volume_tracking_test` builds a loop-backed volume when the machine has no user-visible mount (Linux hides `/`), skipping only without sudo. Still red, product bugs rather than tests: pull transfers compare raw blake3 against the sampled content hash (`copy/strategy.rs:705` vs `file_transfer.rs:1252`, every pull fails checksum), the Linux fs-watcher turns a move out of the watched root into a Modify instead of a Remove (`platform/linux.rs:83`, `watcher_test` is right), two cores sharing a hostname collide on `devices.slug` during backfill (`sync_backfill_test`), and after an in-process restart neither peer reconnects (`file_transfer_with_restart_test`, which also needs `core.shutdown()` rather than `drop` to release `secrets.redb`). Next: brief fixes for the four bugs; decide whether `core_tests.yml` runs on PRs (see `reports/SPAC-6.md` in the captain drive). |
| Peer reconnect after restart | in progress 2026-10-04 (Fable, SPAC-8); `file_transfer_with_restart_test` green on a Linux VM | Fable | Bug 4 from the triage. Two causes. `NetworkingService::shutdown` never closed the iroh endpoint, so an in-process restart left the old socket alive with the same node id: the peer never saw a disconnect and the restarted core's dials by node id timed out against its own ghost. Shutdown now aborts the periodic reconnection task and closes the endpoint, which sends CONNECTION_CLOSE to every peer. The reconnection loop was bounded (10 dials, 5 s apart) with no bound on a single dial, the device with the higher EndpointId never dialed at all, and the periodic sweep ignored any device seen within five minutes. `service/network/core/reconnect.rs` now decides the dial role (lower EndpointId initiates, the other side waits a 15 s grace period and then dials anyway) and the backoff (2 s doubling to 30 s, 12 attempts), each dial is bounded by a 30 s timeout, the loop stops once the registry shows the device connected, one loop per device is enforced, and the 30 s sweep re-arms any device still unconnected. Next: live proof between a Mac and the NAS; the suite now runs in `cargo xtask test-core`. |
| Pairing bugs from the integration triage | fixed 2026-10-04 (Fable, SPAC-7); `file_copy_pull_test`, `watcher_test`, `sync_backfill_test` green on Linux | Fable | Three product bugs SPAC-6 located. Pull transfers always failed verification: the receiver hashed every byte with blake3 while the sender sent the 16-hex sampled content hash, and the push receiver never verified at all; `TransferComplete.final_checksum` is now the full blake3 of the file bytes in both directions (`ContentHashGenerator::generate_integrity_hash`, computed while streaming on the sender), compared on both receivers, with a corrupted file deleted and reported as `TransferError`. The Linux watcher turned a move out of the watched root into a Modify of a path that no longer existed: `RawEventKind::Rename` now carries inotify's From/To/Both, a lone MOVED_FROM becomes Remove and a lone MOVED_TO becomes Create after a 200 ms pairing window, and a buffered Modify is dropped when its path is removed. Two devices with the same hostname collided on `devices.slug` during sync apply (collision handling existed on insert only, not on update), so backfill failed every 5 s forever; the slug now gets a suffix from the device id on both paths, and three consecutive backfill failures log an error and pause sync instead of looping. `test_volume_resource_events_on_sync` now accepts the coarse `Refresh` that #3061 made backfill emit. Still red in `sd-fs-watcher --lib` and out of scope: `test_file_deletion_events` reads the write's buffered Modify before the Remove, and `a_shallow_watch_delivers_events_for_immediate_children` is filtered because `tempfile` dirs contain `.tmp`. Next: bug 4 (no reconnect after in-process restart) in its own thread; decide whether `core_tests.yml` runs on pull requests. |
| Nightly CLI channel | landed 2026-10-05 (Fable, SPAC-10); nightly prerelease live, trigger moves to `main` in #3111 | Fable | Every push to `main` builds `sd` and `sd-daemon` (release, default features, macOS ARM64 and Linux x86-64) and republishes them on the rolling `nightly` prerelease with sha256 files, its tag moved to the built commit (`.github/workflows/nightly.yml`; the branch is one line in the trigger). `sd update` on `update.channel = nightly` fetches that release by tag and compares the release's target commit with the commit compiled into the binary (`SD_GIT_SHA` from `apps/cli/build.rs`, shown by `sd --version`); the stable channel still compares the latest release tag with the package version. The asset matcher strips the platform and expects exactly `sd` (or `sd-cli`) and `sd-daemon`, so the two no longer collide; downloads are checked against their `.sha256`; replacement writes a temp file and renames over the running binary; a launchd-managed daemon is restarted with `launchctl kickstart`. `sd update --yes` is the non-interactive path and `sd update install-timer` writes a launchd agent (systemd timer on Linux) that runs it hourly. Default `update.repo` is now `spacedriveapp/spacedrive`. The README now links the nightly release and has an Install a nightly section. Next: land #3111 so the trigger follows `main`, opt both Macs in. |
| `core_tests.yml` on pull requests | landed 2026-10-05 (Fable, SPAC-11); unit and integration jobs green on the first Blacksmith run | Fable | The workflow was dispatch-only with the two traps SPAC-4 removed from `ci.yml` (`maximize-build-space`, runner label in the `target` field). It now runs on push and pull requests against `sources` as two `blacksmith-4vcpu-ubuntu-2404` jobs: unit (`cargo xtask test-core --unit` plus `sd-fs-watcher --lib`, 12 min cold) and integration (`cargo xtask test-core --integration`, 17 suites, 49 min cold of which 46 min is the suites themselves; the six two-core suites take 4 to 6 min each by design). `xtask test-core` gained `--unit` and `--integration`. Each job has its own `Swatinem/rust-cache` key, `scripts/setup.sh` installs libav, bun 1.3.4 with `--frozen-lockfile` serves the TypeScript bridge suites, and the loop-device volume helper now reports a missing `losetup` device as the skip reason (Blacksmith grants passwordless sudo, so the volume suites ran for real). The two `sd-fs-watcher --lib` failures are fixed: `EventFilters::should_skip` matched `.tmp` anywhere in the path and hid every file under a `tempfile` dir, so temp and system filters now look at the file name only; `test_file_deletion_events` drains the file's events until Remove and asserts no Create slipped in, instead of assuming the write's buffered Modify arrives before the delete. Next: cut the integration wall time by splitting the two-core suites across jobs if 50 min proves too slow for the PR loop. |
| Replica fetch: resume, cap, pause, progress | in progress 2026-10-05 (Fable, SPAC-14); four commits in review against `sources` | Fable | Bugs 5 and 6 from SPAC-12: pairing the Studio with the MacBook pulled 2.9 GB of source stores at 200 KB/s over ship wifi, and stopping the daemon threw the partial away. A transfer now lands in `mounts-remote/<device>/<source>.<generation>.<blake3 prefix>.part` and survives failures and restarts; the next attempt resumes from its length over two new byterange requests, `FetchSnapshotFrom` and `FetchDatabaseFrom`, whose header still names the whole artifact, and the finished file is validated against the header hash before it replaces the live replica. A part is removed only when the owner's listing no longer carries its generation. `replication.max_bytes_per_sec` (daemon config, `sd config set`, zero unlimited) caps serving and receiving through one token bucket; `sd sources replication pause` / `resume` stops new fetches and halts in-flight ones with their parts kept, persisted in `replication.paused`. `sources.list` rows carry the in-flight transfer (bytes, total, rate), `core.status` and `mounts.replication_status` summarize, and a typed `ReplicationProgress` event is emitted about once a second. Both Macs must run a nightly with the `From` requests for resume to work; an older owner answers them with an error and the fetch starts from zero. Documented under Source replication in `docs/core/networking.mdx`; related to but distinct from P1 "restart refetch" in the replication plan. Next: live test on the Studio/MacBook pair with a cap set before reconnect. |
| Library join over the network | in progress 2026-10-05 (Fable, SPAC-13); fixes and two new suites green on a Linux VM | Fable | SPAC-12 paired a fresh Mac Studio with the MacBook library and `join-remote` backfilled the wrong way: the member went `Uninitialized -> Backfilling{joiner}` and adopted the empty copy's snapshot (its Default space row was overwritten), the member's catch-up loop ran against the joiner every 6 to 15 s forever (no watermark is ever written for a peer with no device-owned rows), the reverse `RegisterDeviceRequest` timed out so the joiner never got the member's device row and so never found a sync partner, and `library info` on the joiner mixed peer totals with `Sources 0`. Fixes: a `sync_state` marker in `sync.db` says whether a library copy still waits for its first backfill, written only when a copy is created from another device's library, so every library that already holds state starts `Ready` and never requests a snapshot (`SharedChangeRequest.include_snapshot`, catch-up never escalates to a full backfill); per-peer catch-up completion is recorded in `sync.db` even when zero rows arrive and the loop no longer re-polls a peer on a 60 s timer; the registration round trip carries `needs_initial_state` and returns the responder's device record in the response (a copy still waiting for its own backfill refuses to seed a joiner; `sync_setup` fails instead of reporting `devices_registered: true` when the record is missing); connections a protocol dials now get a stream handler too (`TrackOutboundConnection` from `get_or_create_connection`), which is why the peer's request on a connection we opened used to hang 30 s; fleet totals in `library info` add only summaries of devices in the library's device table and include the peers' source counts. Tests: `library_join_test` (two daemons, real pairing and join) and `sync_catchup_test` (in-process). Docs: pairing, devices, CLI overview, library sync setup, library sync. Next: live check on the Macs (delete `My Library 1.sdlibrary` on the Studio and join again: MacBook stays `Ready`, Studio gets both device rows and the MacBook's spaces, no repeating "Never caught up with this peer" lines after the first). |
| CLI and daemon housekeeping from the live runs | in progress 2026-10-05 (Fable, SPAC-16); one PR against `main`, one commit per item | Fable | Nine small fixes the Mac Studio / MacBook sessions (SPAC-9, 10, 12, 15) turned up. `core.status` defaults its `replication` block so a newer `sd --device <peer> status` reads an older daemon (unit test on the pre-#3109 shape); `sd config get/set/show` cover `replication.paused` beside `max_bytes_per_sec` and `--help` lists the keys; a peer syncing a library this device no longer holds gets one WARN per connection and a `SyncMessage::Error` reply so its catch-up fails fast and backs off (a real "leave library" is a separate brief); `core.status` reads paired and connected counts from the network trust registry so `sd status` and `sd network status` agree ("Paired devices"); `sd update` downloads into resumable `<data>/updates/*.part` files with a 60 s idle timeout, verifies sha256 before replacing, appends every attempt with its reason to `logs/update.log`, keeps `update-state.json`, writes `sd-version.txt` beside the binaries, and `sd update status` prints the running build, marker, last attempt and result; the daemon plist asks launchd for 65536 file descriptors; `jobs.list` no longer loads each job's persisted `state` blob (an indexer over millions of files keeps its seen-path set there, which is why a remote `job list` took 45 s); `maximize-build-space` removed from the Blacksmith jobs in `release.yml` and `server.yml`. `main` is the branch of record from 2026-10-05 (fast-forwarded to `sources` at 10dbc18): `nightly.yml` and `core_tests.yml` now trigger on `main`, with `sources` kept in the lists for one release. Next: merge, nightly, re-run the SPAC-15 re-join once the MacBook is on it. |
| Library backup and restore | in progress 2026-10-06 (Fable, SPAC-23); one PR against `main` | Fable | Release gate "Database backup and restore pass from release candidates" and candidate validation step 7. `libraries.backup` copies `library.db`, `sync.db`, every owned source store and its `sidecars.db` with `VACUUM INTO` on a fresh connection while the daemon runs, hashes each file with blake3 and writes `manifest.json` (device, build sha, applied migrations by name, each source's store id, revision and schema hash), into a directory or `.tar.zst`; `libraries.backup.verify` rehashes without restoring; `libraries.restore` verifies every hash first, refuses a backup whose library applied a migration this build does not know, stages under the data dir, renames into place (replace mode closes the library, releases open store and sidecar pools so the swapped `data.db` is what the next open reads, and parks the displaced state in `restore-trash/`), and reopens. Replacing a library other devices are members of needs `force`, because the device rows and sync watermarks in it would diverge from what those peers hold. CLI: `sd library backup --to`, `sd library backup verify --from`, `sd library restore --from [--as-new|--replace] [--force]`. `core/tests/library_backup_test.rs` backs up while a watcher writes, restores into a fresh data dir and proves counts, tag assertions, space items and store revisions equal, replaces over a mutated library and proves the mutation gone, and detects a one-byte tamper. Documented in `docs/core/library.mdx` (Backup and restore). Next: run the gate on the MacBook library before the beta candidate. |
| Extension loading | in progress 2026-10-07 (Fable, SPAC-28); one PR against `main` | Fable | Product sequence step 5, first half. A daemon built with the `wasm` feature discovers every `<data dir>/extensions/<dir>/manifest.json` at startup (before libraries open, so a resumed job finds its extension), instantiates it with wasmer 6 (4.4 stopped linking on Rust 1.89+, wasmer #5610), and keeps the instance for its lifetime. `plugin_init` registers jobs; `extensions.list` reports them and `extensions.run_job` starts one as a `wasm_job` through the normal job manager. The guest's host calls are bridged to the job context: progress, warnings, metrics, an interrupt flag the pause path sets, and checkpoints that reach the checkpoint table before the guest continues, so both a graceful stop (paused row) and a SIGKILL (running row plus checkpoint) resume from the guest's last state on restart; proven live on a Linux daemon and in `core/tests/wasm_extension_test.rs` (acceptance group, with its own `--features wasm` build: carrying the feature across the integration group added minutes to every test binary link, 51 to 78 minutes on the first CI run). The SDK's `#[job]` export now runs the job function (async ones through a poll loop) and `plugin_init` installs a panic hook so a guest panic reaches the host log. Phase B: the photos extension builds to wasm32 without wasm-bindgen imports, loads beside the test extension and registers four jobs; its first job stops at `JobContext::run` (task execution has no host function), and the SDK has no file-kind or preview registration at all, so step 5's second half is a host-function brief (report SPAC-28 lists the gaps: task execution, vdfs entry and sidecar access, AI inference, model registry, tags, entropy, clock, file-kind and preview registration). The `wasm` feature stays opt-in; whether the shipped daemon turns it on is Jamie's call. |
| Repository release gates in CI | in progress 2026-10-07 (Fable, SPAC-26); one PR against `main` | Fable | `docs/core/releases.mdx` lists five repository gates; only formatting, clippy, typecheck and tests had jobs. `ci.yml` gains four more on every PR to `main`: Docs (bun 1.3.4, `docs/` frozen lockfile, `types:check` and `next build`), Generated types (`generate_typescript_types` plus `cargo check -p spacedrive` for the Tauri schemas, fails on a diff under `packages/ts-client/src/generated` or `apps/tauri/src-tauri/gen/schemas`, own rust-cache key), Secrets scan (pinned gitleaks over the PR's commits and the tree, allowlist in `.gitleaks.toml` including the AWS docs example key in `cloud_credentials.rs`), and License and notices (LICENSE is Apache-2.0, NOTICE exists and carries the same AGPL-3.0 cut-over date as README). Found and fixed on the way: the docs build failed on an unescaped `<name>` in `cli/library-sync-setup.mdx`, and the committed Tauri schemas predated the updater plugin. The gates in `releases.mdx` now name their job. Next: confirm the four jobs green on the PR and the warm wall time under 10 min. |

## Product sequence

1. Run Spacedrive on the Mac, pair it with a NAS, and operate its sources,
   jobs, and logs through the same CLI.
2. Package the proven CLI operations as a Spacedrive skill. Build the MCP
   transport after the tool schemas have been exercised.
3. Simplify the web sidebar and inspector around primary filesystem concepts.
   Audit every sidebar row for use of the canonical component.
4. Restore source-scoped persistent sidecars beyond thumbnails, streaming
   previews, and video scrubbing through the byte-provider path. Thumbnail
   sidecars landed 2026-09-21.
5. Open file-kind detection and preview registration to one real extension.
6. Carry the stable operations and component model into the GPUI client.
7. Close the release test matrix around destructive operations, restart safety,
   supported filesystems, providers, and clients.

## Current architectural truth

- Spacedrive is a set of sources. A source has an origin, an ingest, and a
  store. Filesystem and adapter sources differ in ingest, not store shape.
- `crates/store` owns the record table, filesystem facets, content identities,
  edges, overlays, and source-store schema.
- The volume index is the filesystem read map. A source is a retained scope
  over that map and adopts identities assigned by the volume arena.
- A source store has a rebuildable generation and an irreplaceable assertion
  layer. No code path may assume the store can be discarded and rebuilt.
- Content IDs are convergent. Sampled IDs are candidates. Destructive duplicate
  decisions require confirmed integrity hashes.
- The entries world is gone: its schema, row sync, tests, and compatibility
  names were removed by `docs/plans/2026-09-15-entries-final-drop.md`. Do not
  restore them.
- A pin is a Space item for a path. It does not own records, trigger
  indexing, or enable a watcher, and pinning never implies a processing
  policy.
- Places is client vocabulary for navigation destinations computed from known
  system folders, sources, pins, and extension contributions. It is not a
  database model, durable identity, operation namespace, or indexing lifecycle.
- Cross-source truth still belongs to the future content `catalog.db`. Until it
  lands, projections such as global redundancy remain incomplete.
- `files.duplicates` can find repeated content within the stores it queries. A
  content value that occurs once in each of two source stores still needs
  `catalog.db` for a complete cross-source answer.
- The physical-drive design requires identity to be concluded conservatively
  from observations. D0 through D2 remain deferred until after the move. The
  hardware product catalog may change labels and visuals, but it cannot merge
  or split physical drives.

## Safety invariants

These are regression boundaries, not incidental fixes:

- Never index the daemon's own data directory, including SQLite journals and
  frozen source copies beneath it.
- A failed filesystem batch must salvage valid rows individually and report the
  record that failed. One bad path must not create a permanent ledger hole.
- A hashing or verification job is complete only after its final write batch
  has committed.
- Persist a source's volume anchor and resolve its absolute root through the
  registry. A relative root is not an operational path.
- Index walks are not resumable. Their sweep evidence is process-local, so a
  restarted daemon must run a fresh walk.
- Refuse a sweep that would remove more than half the store. A later complete
  walk can clean stale rows safely.
- Startup job reconciliation must classify every queued or running row. Resume
  registered resumable jobs and mark every unrecoverable row failed with a
  reason.
- Job dispatch deduplicates by job name and deduplication key.
- An unfiltered source remains unfiltered for watcher events. Capture policy is
  persisted and applied to both the walk and live changes.
- Watcher-created or modified files must schedule content identification. The
  current implementation coalesces dirty sources and nudges them every thirty
  seconds.
- Linux birth time uses the full 256-byte `statx` ABI. Keep the ABI-size test;
  an undersized structure corrupts the stack.
- Duplicate-space UI uses `indexed_bytes` and `unique_bytes` from the same
  source-store ledger. Filesystem allocation and ZFS compression cannot be
  mixed into that ratio.
- A hot-thumbnail bake completion must survive virtualized cell unmounts. Hide
  a fallback icon only after the current image element has loaded its current
  URL.
- Keep Tauri's asset protocol restricted to the current home and mounted
  volumes. Normalize macOS APFS data-volume aliases before conversion instead
  of widening the protocol scope.
- Resolve source and pin containment on the daemon with path components after
  volume alias normalization. Never use raw string prefixes, and reject stored
  relative paths containing roots, parent components, or platform prefixes.

## Known current limits

- An interrupted index walk is marked failed and does not restart itself.
  Re-track the root to dispatch a fresh complete walk.
- Startup-resumed jobs currently enter the live-job map without their original
  deduplication key. A manual dispatch can therefore stack duplicate work until
  that bookkeeping is fixed.
- Watcher hash nudges currently choose the first open library. This is safe for
  a single-library deployment and ambiguous with multiple open libraries.
- The 50 percent sweep fuse has no force path. A legitimate deletion of most of
  a source leaves stale rows for a later explicit recovery path to address.
- Non-UTF-8 names are retained lossily and logged. Review every warning before
  accepting a final archival index.
- A source whose volume is locked or unmounted, with its mount point directory
  left in place, reads as attached. Its index is kept, but reads fail, and
  after the volume returns its watch misses changes until the daemon restarts.
  See `docs/plans/2026-09-28-locked-volumes.md`.
- `crates/ffmpeg` is a default workspace member and links the system libav
  libraries through `ffmpeg-sys-next`, so `cargo build` needs the FFmpeg
  development headers (`libav*-dev` and `libclang-dev` on Debian and Ubuntu,
  `ffmpeg` from Homebrew) before the first build. `scripts/setup.sh` installs
  them and `xtask setup` warns when pkg-config cannot find them.

## Recent landed

### Interface and desktop

- `4bc442e43`: made ephemeral search case-insensitive (lowercased registry
  keys, folded on snapshot restore too) and removed the exact/prefix/contains
  short-circuit so an exact name no longer hides its substring matches. Also
  stopped the double-indexed macOS home tree: the volume map skips hidden
  volumes (the sealed system volume at `/` was mapping the firmlinked tree a
  second time), and directory browse and scoped search normalize their paths
  through `locate_path` like `sources.track` does. Verified live: duplicates
  gone, mixed-case names found, full result sets on exact queries.
- `2097fc141`, `cf5f3f45a`, `4e53d58db`, `f3a1ede2f`, `c32768f2e`: the sidebar
  simplification, `paths.context` and the locations demotion, the thumbnail
  remount and Quick Preview fixes, host media tool discovery with on-demand
  thumbstrips, and explicit permission-aware startup discovery. Register rows
  above track their remaining live proofs and next steps.
- `f0347a317`, `632d04359`: fixed hot-thumb 404s, silenced browse indexer
  jobs, and kept background hashing passes off the event bus.

### Documentation

- `2d380fd26`: moved docs to Fumadocs and rewrote the core pages around the
  daemon/source-store/library split, added `product-direction.mdx` and the
  drive-catalog plan, and retired the whitepaper page.
- `b653676e8`: README and CONTRIBUTING updates, locations and data-model pages
  reconciled, regenerated Tauri schemas, and the new icon asset.

### Multi-device

- `50fdbad59`, `3ac1f0af7`, `a0180d1b7`: cross-device search over paired
  devices, peer replicas surfaced in `sources.list` with remote listings
  served, and remote job completions delivered.
- `aa589a767`, `9d948817e`: stopped device presence flapping (a connection
  loss only marks a device disconnected when no connections remain),
  classified Tailscale CGNAT paths with their own badge, and made overview
  and sidebar request paired devices explicitly so a fresh page sees them.
  Verified live: 20 consecutive samples online.
- `5f49a0141`: fleet-wide statistics are computed once by the owning device
  and reported identically everywhere; fixed the PATH-dependent ZFS capacity
  correction, stale persisted volume capacity, and double-counted source
  totals.
- `5d822a0ca`: fixed peer snapshot serving to read from the volume index
  directory, restoring source replication between paired devices.
- `1766bd175`, `728403b4f`: wired remote job activity end to end. Paired
  devices are subscribed as they connect, the cache builds rows from started
  and progress events (nothing emits a queued event), `sd job remote` lists
  them, and terminal rows are swept hourly. Verified live in both directions.
- `960febb2b`: added `--device` to the CLI and the remote-ops protocol: any
  Wire action or query can be forwarded to a paired device and executed
  through the same registries, with the target resolving its own open
  library. Verified live: sources listed, jobs listed, and a verify job
  dispatched on the NAS from the Mac.
- `29b9b8abc`: pairing completion broadcasts the connection so peer sync
  starts immediately; `sd --device <x> logs follow` streams a paired
  device's log bus with server-side filtering; `sd job remote --watch`
  follows remote jobs; stale remote job rows are swept by `updated_at`.

### Linux and archival correctness

- `c446235e6`: captured uid, gid, symlink targets, content errors, and invalid
  UTF-8 diagnostics.
- `50ab6038e`: added source freezing and the `sd sources` CLI domain.
- `a33fde636`: enabled self-contained Linux musl builds with vendored OpenSSL.
- `55e962717`: made content hashing wait for committed batches.
- `4aaec8b5c`: excluded the daemon data directory and salvaged failed batches.
- `a1c9919da`: derived volume IDs from fingerprints instead of process-local
  randomness.
- `ac9f266ed`: added full-byte verification for shared sampled content.
- `81070270d`, `63ba30edd`: read Linux birth time through `statx` and corrected
  the kernel ABI buffer size.
- `3dd5f6a3a`: recorded ZFS pool topology in drive groups and tightened the
  pre-export evidence list.

### Restart, registry, and sweep safety

- `c1a6bbfa2`, `1ab9017c6`: registered every job type, made startup
  reconciliation truthful, enabled it, deduplicated dispatch, and removed the
  task-system panic-on-drop source by logging it in release builds.
- `8e5fdfbc3`, `8f50a1538`: resolved verify and freeze roots through the source
  registry and persisted the volume rows those roots depend on.
- `023015552`: made walks restart from zero and added the destructive-sweep
  refusal.

### Reporting and live capture

- `672d472e1`, `db15a2177`, `f545663e1`: implemented store-derived unique and
  indexed byte counts, indexed the content join, and made the duplicate bar
  honest under compressed filesystems.
- `0aa1cdc4e`: made watcher events follow the source's persisted capture policy.
- `4badfe403`: made watcher changes nudge content hashing.
- `7690309d8`: typed source capture policy and added `sources.update` for
  renaming sources or widening their capture policy safely.

### Storage cutover

- `59520ee5e`: demoted locations to six-field pins.
- `9d5465832`, `7d41e07ab`, `ba8efcdda`, `c78cf8f31`: stopped writing entries,
  moved reads to the arena and source stores, deleted the unreachable media
  pipeline, and removed most remaining production entry-era support.
- `f32da4777`: rewrote tag operations around record and content identity.

## Worktree handoff

At this update, the only uncommitted work in the checkout is documentation:

- this file
- `docs/plans/2026-09-15-entries-final-drop.md` (new, referenced by the Final
  entries drop register row; commit it with this update)
- plan updates in `docs/plans/2026-08-20-entries-teardown-execution.md`,
  `docs/plans/2026-08-22-source-convergence.md`, and
  `docs/plans/2026-09-08-locations-demoted.md`

Run `git status --short` at the start of every session. Preserve unrelated
changes and do not infer ownership from this list, which will age quickly.

## Primary documents

- Product boundary and decision rules:
  [`docs/core/product-direction.mdx`](docs/core/product-direction.mdx)
- Source/store convergence:
  [`docs/plans/2026-08-22-source-convergence.md`](docs/plans/2026-08-22-source-convergence.md)
- Source durability:
  [`docs/core/design/source-durability.md`](docs/core/design/source-durability.md)
- Tag and assertion ownership:
  [`docs/core/design/tags-and-assertions.md`](docs/core/design/tags-and-assertions.md)
- Location semantics:
  [`docs/plans/2026-09-08-locations-demoted.md`](docs/plans/2026-09-08-locations-demoted.md)
- Whole-volume map:
  [`docs/plans/2026-08-27-storage-map.md`](docs/plans/2026-08-27-storage-map.md)
- Physical drive identity:
  [`docs/plans/2026-09-13-physical-drives.md`](docs/plans/2026-09-13-physical-drives.md)
- Hardware catalog and visuals:
  [`docs/plans/2026-09-13-drive-catalog.md`](docs/plans/2026-09-13-drive-catalog.md)
- Entries teardown history:
  [`docs/plans/2026-08-20-entries-teardown-execution.md`](docs/plans/2026-08-20-entries-teardown-execution.md)

The `.tasks/` tree and `docs/core/task-tracking.mdx` describe the retired task
system. The entries-teardown status table also lags some September 14 fixes. Prefer the current tree, recent commit
messages, and this dated handoff for present state. Preserve plans as rationale
unless a task explicitly updates them.

## Session start checklist

1. Read `AGENTS.md` and this file.
2. Run `git status --short` and inspect recent commits.
3. Confirm live external state before relying on a dated observation here.
4. Put the task in the Immediate register before making a broad change.
5. Read the linked design document for that workstream.
6. Use generated TypeScript types for frontend changes and regenerate them
   after changing public Rust types.
7. Run focused tests while iterating, then verification proportional to the
   affected data path.
8. Update this file before handing the work to another session.

## How to maintain this file

- Keep it short enough to read at session start. Link to designs instead of
  copying their arguments.
- Replace stale state rather than appending a diary. Recent landed should cover
  only the context needed to understand current risks and priorities.
- Date remote observations and name where they came from.
- Record blockers with a concrete unblock condition.
- Move finished tasks out of the Immediate register in the same commit that
  finishes them.
- Update the code baseline whenever the current state depends on newer commits.
- Never put credentials, tokens, private keys, or recovery material here.
- `.tasks/` remains frozen. This file is the operational handoff until a new
  task system deliberately replaces it.
