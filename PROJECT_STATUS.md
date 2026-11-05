| Release candidate validation (cloud) | run 2026-11-05 on nightly `26b57e1` (SPAC-58); steps 1 to 4 and 7 run, record in `docs/core/acceptance/candidate-validation.md` | Opus | Steps 1 (Linux), 2, 3 and 7 pass. A v2.0.0-alpha.2 library opens but loses its locations, files, tags and collections (table in the record). Windows clean install fails until a nightly with #3169 is published; then rerun the smoke workflow from #3166. Step 4 fails. Fixed and merged: truncated file left by a killed copy (#3170), `--instance` data dir nesting (#3171), store migrations missing from the log (#3168), Windows verbatim paths (#3169), unhealed first walk (#3172), thumbnail scope (#3174), library statistics (#3173, #3182, #3183), `logs show` path (#3175). In review: nightly builds cancelled by every push (#3186), CLI panic on a closed pipe (#3188). Open: on Linux the heal never runs for sources on `/` or `/home` because those volumes are hidden; a full disk hangs the indexer; changes made while the daemon is stopped are not reconciled (the warm-start reconcile in `zero-onboarding-startup.md` is not built). Remaining: macOS installs and signing, real prior-version Mac state, a cross-network pair, remote operations, update signatures and rollback, release notes and checksums. |
# Project Status

> Last updated: 2026-10-09
> Code baseline: `b1a58be`
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
on 2026-10-05 (Jamie), and again to November 15, 2026 on 2026-10-31 (Jamie).
This is a release goal, not a claim that the gates have passed. No package
versions, tags, builds or published releases changed.

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
| Source runtime reliability | R1-R4, R6, R7 and the R8 acceptance matrix landed | in progress | Execute `docs/plans/2026-09-15-source-runtime-reliability.md`. Landed: destructive paths removed, flush made a durability barrier, self-nudge loop stopped and hashing progress labeled (`bb033de1b`, `954c6d980`, `fb0dab3e6`); R1 ancestor synthesis (`9242a14a1`); R2 anchored volume roots (`0aa0d390c`); R3 persisted replica inventory (`59b05a6c8`) plus the fetch-generation fix (`98e8a1485`: header names the generation and blake3 of the bytes delivered, a delivery must parse before it replaces a good artifact, one failed source no longer aborts the device sync, fetch failures back off, an unchanged generation still refreshes owner facts, revocation unloads and quarantines replicas, snapshot decode is bounded); the coverage heal (`5a65d0998`); watchers made unconditional on a bare daemon (`8c8499413`, `847144ba0`, `3ee5449bf`); dirty-driven refetches paced to five minutes (`4a690c84a`). R6 (results in the plan): `9e29e7c46` opened stores read-only with listings, lookups and Unicode-folded name search; `4cf25c17c` routes one backend per source (arena when restored or walked, SQLite otherwise, no empty-result retry, `total_is_exact` on the wire, detached directories list from their stores); `5f1fdd287` delivers a nested source's replica as its own database with the snapshot header contract; the 100-store/1M-record cold fan-out measures 1.7 s (`33bd1ce5d`). R7 (results in the plan): stores validated by direct SQL with zero dangling ancestry, a cold restart with the owner down serves and searches every replica, byte read, remote op and remote log stream verified, and reconvergence transfers zero bytes. Since then: musl builds use mimalloc as the global allocator, since musl's allocator gives each allocation group its own mapping and exhausted the kernel's map limit within an hour of a restart; a nested source's generation is the store's revision, a count of committed row changes ... (line truncated to 2000 chars)
| Drive catalog H0                                  | plans committed, code missing          | unconfirmed     | The drive-catalog and physical-drives plans landed in `2d380fd26`, but the uncommitted `packages/drives` package earlier handoffs said to preserve is no longer in the checkout. Confirm whether that code survives elsewhere before restarting the package and archetype contract from the plan. The physical-identity firewall still applies.                                                                                                                                                                                                                                                                                                                       |
| Final entries drop                                | FD0–FD5 done; FDA matrix landed, all rows pass on Linux | Opus            | Execute `docs/plans/2026-09-15-entries-final-drop.md`. FD0 through FD4 are done (2026-09-18): no production code reads the entry tables, legacy row sync is gone, and a transactional migration drops 29 retired tables so every library converges on a 14-table schema. Both live libraries upgraded after backups. Locations are gone end to end: pins are Space items, Places come from `paths.system_folders`, and Add Storage tracks sources. FD5 is done: the ephemeral qualifier is retired (`VolumeIndex`, `Partition`, `Arena`, `core.index_status`), entry-era tests and examples are deleted or repointed, and the persistent-era search, event, status, and error surface is gone. FDA landed as `docs/core/acceptance/entries-drop-and-file-operations.md`: every row of the Acceptance section mapped to a CI test (`core/tests/entries_drop_acceptance_test.rs`: fourteen-table schema on a fresh and on a pre-drop library, no production reference to the retired substrate, registered sync models name current tables, pins are Space items with no indexing side effect and survive a restart, tags survive a reindex, a frozen store is self-describing and unwritten by a reader, an incompatible library is refused intact, non-UTF-8 names retained lossily and warned). The two rows that failed when the matrix landed are fixed: Linux trash restore records the item under `Trash/files` and takes the `.trashinfo` with it on restore and purge, and a rename over an existing file removes the overwritten row in the batch that lands the move (the matrix's F-a, F-b). `KeyManager::close` no longer creates a redb file named `:memory:` in the working directory, so the pin restart row runs in process. Not automatable on the runner: a pin following a volume remount, remote job watch and log stream (live in SPAC-12), the `wasm` and `sd-native` check lines. Follow-ups are in its record: redundan... (line truncated to 2000 chars)
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
| Action validation, preview, and folder merge      | V1 through V9 and dedupe landed; CI acceptance landed, NAS run still open | Opus            | Plan: `docs/plans/2026-09-22-action-previews.md`. Preflight (`validate:` and `preview:`), `FsPlan`, copy, move, merge, and delete on both methods, the merge job, dedupe as a delete target (first copy in walk order or chosen copies, read in full before removal) with `sd file dedupe` and a Duplicates screen, `sd file copy`, `merge`, and `delete` rendering preflight, the merge dialog, the modifier drop, the preflight-driven operation modal, plan handles, and overlay listings with the explorer's preview mode; the old validation hook is gone. Remote execution rides the existing `--device` forwarding. Two opt-in preflight methods on actions, both taking the action's exact input: `validate:` answers whether and how an operation runs (findings with stable codes plus execution facts, re-run server-side at dispatch, errors refuse) and `preview:` answers what will exist afterward (`FsPlan` from the index, advisory, browsable through plan handles as an overlay on `files.directory_listing`). The plan catalogs both methods across copy, move, merge, delete, batch rename, and dedupe. Folder merge is the first action on them: recursive merge with integrity-confirmed duplicate skipping, per-leaf conflict policy, optional source consumption, remote execution via `--device`. Also fixes copy's silent blind overwrite inside directory collisions, deletes the CLI's local conflict logic per the operations-are-the-unit principle, and removes the dead `ValidationResult`/`ConfirmationRequest` hook. Six decisions are James's. Landed ahead of preflight: `files.delete` takes a comparison as its targets (`sd file delete A --against B --show both`); the job derives the set from the index as it runs, reads both copies in full before removing one, checkpoints the compare cursor, and reports skips. The plan's acceptance clauses that a fixture can prove are... (line truncated to 2000 chars)
| Addressing                                        | P0 and P1a landed 2026-10-06; P1b, P1c and D1 to D6 open | unowned | Plan: `docs/plans/2026-09-25-addressing.md`. P0 rewrote `docs/core/addressing.mdx` to match the code. P1a landed: source stores carry a schema version in `PRAGMA user_version`, migration 1 rebuilt `content` so a row's integrity hash belongs to every record on it (candidate rows by sampled hash, confirmed rows by integrity hash, `candidate_uuid` beside `uuid`), and compare-delete and dedupe now read a hash a file's own bytes produced. Upgrade note: an existing store migrates to `user_version` 1 on its first writer open (daemon start or source attach); the rebuild is one transaction and measured 11 s for 1M content rows and 32 s for 3M in release on a cloud machine, so a 3M-record MacBook pauses that source for well under a minute once. Every row that held both hashes loses its integrity hash, so dedupe and compare-delete re-read each shared file once afterwards. Replicas keep version 0 until their owner upgrades and are read in their own shape. Next: P1b, network admission for fully paired devices only; P1c, refusals for addresses destructive operations can't honor (cross-device moves wait on D6); then P2 to P6 behind D1 to D6. |
| Locked and unmounted volumes                      | L1 and L2 landed 2026-10-07 (SPAC-25); L3 and L4 landed 2026-10-08 (SPAC-40, `306f97f`); L5 built 2026-10-08 (SPAC-43), PR open | Fable           | Plan: `docs/plans/2026-09-28-locked-volumes.md`; acceptance: `docs/core/acceptance/volumes.md`, run on a loop-mounted image and a file-backed ZFS pool in the acceptance CI job. L1: the monitor marks a tracked volume offline when detection stops returning it, `attach_library` resolves against live detection and corrects a stale row, the volume index carries each mapped drive's mounted state, and a source whose volume is away comes up detached with its map restored read-only and no watch armed, reattaching when the volume returns. L2: attached means mounted; a mount point check (device differs from the parent's) runs before any walk, hash or thumbnail job and inside the jobs themselves, so the directory an unmounted drive leaves behind is never walked, a forced heal dispatches nothing, and `sources.track` on it refuses. L3: Linux detection reads `mounted`, `encryption` and `keystatus` from the one `zfs list` per refresh and lists a dataset that is unmounted or whose key is not loaded as a volume with `locked` set; `VolumeState::{Mounted, Unmounted, Locked}` reaches `sources.list` as `volume_state` and `volumes.list` through the volume's flags; a locked dataset matches its tracked row by mount point, since nothing inside it can be read. L4: the volume index follows the volume manager's events (`follow_volume_events`) instead of the monitor calling it; an unmount or lock drops the watches the OS dropped, a mount or key load resolves the roots, restores the map if needed, re-arms the watches on the mounted filesystem, puts identifications that failed while the volume was away back in the pending set and dispatches a background identity pass. The ZFS suite (`zfs_locked_volumes_acceptance_test`) builds a file-backed pool with an encrypted dataset and runs only on the CI runn... (line truncated to 2000 chars)
| Add to Library, core half                         | core landed 2026-10-07 (Fable, SPAC-27); modal landed 2026-10-07 (Fable, SPAC-32); offline copy (step 4) in review 2026-10-08 (Fable, SPAC-42) | Fable           | Plan: `docs/plans/2026-09-16-add-to-library.md`; acceptance: `docs/core/acceptance/add-to-library.md`. Library Settings > Adding content holds the defaults (`LibrarySettings.adding`: store in library, offline copy kept, filtered capture, content identified) and `sources.track` and `volumes.track` take the same `overrides` on top; `config.library.update` changes the defaults for later adds only. Store placement is in each source's config and resolved per library by `VolumeIndex::store_dir`: in library under `<data>/sources/<id>`, on source under `<root>/.spacedrive/sources/<id>`; every open, read, freeze and listing goes through it, and on source is refused for network and cloud volumes. Adding any path registers its volume row (synced, event emitted) and maps it in the index, so a source always has its volume. Every store carries `source.json`; re-adding a scope the same library wrote adopts that identity and reopens the catalog. `crate::config::is_managed` keeps the data directory, every `.spacedrive` directory and the volume marker out of walks, watcher events and the writer, whatever the rules say. `sources.delete` removes the registration, retires the handles and keeps the catalog and the volume row unless `delete_catalog` is set; `sd source untrack` wraps it. The modal (`packages/interface/.../AddToLibraryModal.tsx`, replacing `AddStorageModal`) is the one entry for folders and whole drives from the Explorer, path bar, Home and the volume menu: it shows the library defaults resolved for the scope, takes per-add overrides, explains an unavailable placement, offers Use these settings as defaults, says how the path is already included (exact, enclosing, nested) and reports the saved `TrackSourceOutput` with a jobs link; `volumes.track` now returns its... (line truncated to 2000 chars)
| Retire Spacebot, Console, and storefront surfaces | done (2026-09-25)                      | Opus            | The Spacebot UI, voice overlay, `spacebot` app config, window, build aliases, generated types, and design docs are removed; the Alt+Space global shortcut and its plugin went with the voice overlay. Console and Pod design docs are removed. Private copies are in `company/archive/spacedrive-sources-public-cleanup/`. No storefront code remained. |
| CI green on `sources` | landed 2026-10-04 (Fable, SPAC-4); Rust Formatting, Clippy and TypeScript green on #3102 | Fable | The three `ci.yml` jobs were red on `sources` before any PR: the self-hosted runner's preinstalled toolchain had no rustfmt or clippy (`setup-rust` now installs both every run, and `rust-toolchain.toml` names them), `maximize-build-space` left the Blacksmith runner's root disk with 100 MB so no toolchain could install (step removed), the clippy job ran on GitHub's ubuntu-22.04 whose FFmpeg 4.4 cannot build `sd-ffmpeg` (now `blacksmith-4vcpu-ubuntu-2404`), and the server and desktop crates failed to compile without a built web UI (debug builds now get an empty dist with a warning; release builds still refuse), `cargo clippy --workspace -D warnings` failed in seven crates plus the Tauri sidecar check and the two platform-only file-opening crates, and `bun run typecheck` in `apps/tauri` had 30 errors in `packages/interface`. All fixed with no product change: sqlx row aliases in `sd-store`, exact casts and raw FFI borrows in `sd-ffmpeg`, `DaemonRequest.device` in `sd-mobile-core`, target-gated `file-opening-macos`/`-windows`, a Tauri build script that drops `externalBin` when no daemon sidecar exists, unused imports and `SourceInfo.adapter_id` nullability in the interface, the `/redundancy/duplicates` route the dashboard already linked. `core/tests/delete_strategy_test.rs` compiles again (calls `ops::files::trash::trash` directly), the 13 stale doctests compile, `scripts/setup.sh` installs the libav dev packages, bun is pinned to 1.3.4 with `--frozen-lockfile`, workspace `rust-version` is 1.95 (gpui's floor). Still red and out of this scope: 20 integration-test failures on a bare Linux VM (see SPAC-2); the three unit-test bugs have their own row below. Next: confirm the three jobs green on the PR, then decide whether `core_tests.yml` should run on pull requests. |
| Unit-test bugs on `sources` | fixed 2026-10-04 (Fable, SPAC-5); `cargo test -p sd-core --lib` green five runs in a row with default parallelism | Fable | Three bugs the SPAC-2 baseline found. The btrfs parser read the filesystem UUID from a line starting with `uuid:`, but `btrfs filesystem show` prints it on the `Label:` line, so every Btrfs volume failed with "Could not parse Btrfs UUID"; the header line now yields both label and UUID. The plus-code parser rejected its own encoder's output and Google's `WR2C+2C Bibra Lake` form (its alphabet check demanded every character equal every alphabet digit); `PlusCode::is_valid` now follows the Open Location Code validation rules, the encoder is unchanged. The `ops::files` fixture flakes were not an FTS5 or temp-path problem: `LibraryManager`'s directory watcher auto-opens a new `.sdlibrary` folder 500 ms after it appears, and under load `create_library` was still initializing it, so two openers migrated one database at once (`seaql_migrations` UNIQUE failures, `vtable constructor failed: search_index`, `AlreadyInUse`). Paths a create call is building are now registered and the watcher leaves them alone. Next: the integration-test triage (needs a Mac for the pairing suites). |