# Project Status

> Last updated: 2026-09-21
> Code baseline: `e8febd196`
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

The one-shot NAS index is locked: all nine sources verified and frozen on
2026-09-15 (dated copies under each store's `freezes/`). The NAS stays online
for roughly one more week, so re-freezing after late changes remains possible.
The remaining NAS work is manual evidence capture and the off-NAS backup, both
owned by James.

The September 15 evening live audit reopened source-runtime acceptance: titan
still registers all nine sources, but the Mac currently serves eight replicas,
and the home store has malformed parent relationships. The repair plan is a
draft for review in `docs/plans/2026-09-15-source-runtime-reliability.md`. This
does not invalidate or replace the dated NAS freezes.

Development focus moves to the product sequence below, starting with running
Spacedrive on the Mac and operating titan's sources from it.

The drive-catalog and physical-drives plans are committed (`07fc633c2`), but
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
and draft launch wording. This is a release goal, not a claim that the gates
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
September 16), through `3d8c44394` plus identified uncommitted database-export
work. Reuse the new read-only store path and exports. Remaining foundations:
consistent volume registration/defaults, library-scoped store resolution,
portable source identity, handle retirement, unconditional managed-directory
exclusions, and removal that preserves portable catalogs. Full offline store
copies are distinct from existing saved arena maps. On-source WAL databases
need supported local storage on their serving daemon. No runtime code or fleet
state changed; placement defaults and handoff/consolidation remain for review.

Library sync and membership research is ready for review (Codex), recorded in
`docs/plans/2026-09-15-library-sync-research.md`. The live Mac and titan are paired
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
`2c7e058ce` removed the overview's Setup Sync button. Pairing still closes
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
cold daemon and a detached source both answer. Writes against titan-owned
replicas queue durably, overlay the replica listing immediately, and deliver
on the owner's ack; titan's deployed daemon predates the merge action, so
the roundtrip completes at its next redeploy. No legacy migration: zero tag
rows verified on the Mac 2026-09-16; check titan the same way before FD4.
Execution record and remaining P6 items are in
`docs/plans/2026-09-17-tags-on-source-stores.md`. This is source-store
assertion sync; library sync stays deferred.

Path-scoped thumbnail generation is implemented and verified (Codex, 2026-09-15, uncommitted). `thumbs.generate` accepts a file or indexed folder with missing/stale/force modes, uses resumable batches, and versions video tiles against host FFmpeg availability. File and folder context menus plus `sd thumbs generate` call the same operation. Ten focused tests, core/CLI/server/Tauri checks, generated types, CLI help, and the desktop frontend build pass. Changed hooks have no TypeScript errors; the full interface typecheck still fails in existing dependency and SVG declarations. Restart the daemon and reload the app, then regenerate a folder containing an old video icon to verify the live repaint. See `docs/core/thumbnail-generation.mdx` for coverage and resume semantics.

Update this table whenever work starts, lands, becomes blocked, or changes
owner. Keep only active and near-term work here. Move completed context to
Recent landed.

| Work                                              | Status                                 | Owner           | Next proof or action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------- | -------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source runtime reliability | R1-R3, R6 and R7 landed and verified live | in progress | Execute `docs/plans/2026-09-15-source-runtime-reliability.md`. Landed: destructive paths removed, flush made a durability barrier, self-nudge loop stopped and hashing progress labeled (`c63359a9e`, `adf729cd9`, `92bb4412e`); R1 ancestor synthesis (`3a425c629`); R2 anchored volume roots (`4b3eb7389`); R3 persisted replica inventory (`18304e041`) plus the fetch-generation fix (`0650e0acb`: header names the generation and blake3 of the bytes delivered, a delivery must parse before it replaces a good artifact, one failed source no longer aborts the device sync, fetch failures back off, an unchanged generation still refreshes owner facts, revocation unloads and quarantines replicas, snapshot decode is bounded); the coverage heal (`fd40a5b87`); watchers made unconditional on a bare daemon (`88f496891`, `1d8d23962`, `0e605105b`); dirty-driven refetches paced to five minutes (`b57cb2857`). R6 executed 2026-09-16, results in the plan: `51e4873d4` opened stores read-only with listings, lookups and Unicode-folded name search; `3d8c44394` routes one backend per source (arena when restored or walked, SQLite otherwise, no empty-result retry, `total_is_exact` on the wire, detached directories list from their stores); `475a6b0b3` delivers a nested source's replica as its own database with the snapshot header contract, verified live — dev-tools ships as an 11.3 MB database, the sibling leak is gone, and the replica lists and searches identically; the 100-store/1M-record cold fan-out measures 1.7 s (`b88bfe4cf`). R7 executed 2026-09-16, results in the plan: all ten stores validated by direct SQL with zero dangling ancestry, the home store needed no repair, cold-restart with titan down serves and searches all nine replicas, byte read, remote op and remote log stream verified, reconvergence transferred zero bytes for titan's nine sources. Titan runs this build (deployed 2026-09-18). Its daemon had aborted at 03:27 that morning with `memory allocation of 53520 bytes failed`: musl's allocator gives each allocation group its own memory mapping, and the same code reached 45,641 of the kernel's 65,530 maps and 10.5 GB RSS within 43 minutes of a restart. With mimalloc as the musl global allocator it holds about 260 maps and a flat 1.7 GB. The runbook's start command also sent stdout to `~/spacedrive/daemon.log`, inside the jamie-nas source, so titan re-hashed its own log every 30 seconds; stdout now goes to `data/logs/stdout.log` in the excluded data dir. The deployed `sd-server` matches, but its relaunch stays with James and needs `DATA_DIR` plus `SD_AUTH`. On 2026-09-18 titan re-copied the Mac's unchanged home store in full four times: a nested source's generation came from `data.db` and WAL size and mtime, which move whenever the store's pool drains, and each round cost a 736 MB transfer and a 57-minute arena rebuild. The generation is now the store's revision, a count of committed row changes kept by triggers (`crates/store/src/revision.rs`); after one switch-over copy titan transferred nothing for 19.5 hours, where the old scheme would have made about 18 more copies. Titan runs this build since 2026-09-19 21:34, with `bin/ffmpeg` and `bin/ffprobe` (n8.1.2, copied from the CCTV remux tools) wired in through `SPACEDRIVE_FFMPEG_PATH` and `SPACEDRIVE_FFPROBE_PATH`: a forced tile on a CCTV clip spawned that ffmpeg and baked a 384px frame, so video tiles work on the NAS. That restart exposed the next defect: a replica restart re-copies what it already holds, because a share publishes only after its arena rebuild, so the first sync pass finds no share and refetches 736 MB it already had. It is P1 in `docs/plans/2026-09-19-incremental-replication.md`. The hour-long rebuild behind that share is gone: `Arena::add_entry` built a file type registry per row, parsing 92 KB of built-in TOML each time, and the registry is now built once for the process. Inserts went from 3.00 ms to 7.2 µs, putting a 1.76M-entry rebuild near 13 s in a debug build, with the SQL under it measured at 3 s. Remaining: R4 compaction and memory honesty (Mac 3.1 GB / titan 3.4 GB RSS as its baseline), R5 typed health and coverage, R8 acceptance matrix and the docs sweep, delta replication and transfer cleanup per `docs/plans/2026-09-19-incremental-replication.md` (its `temp_store` decision is James's), plus R6's other registered follow-ons (request cancellation, the deferred title index).. Deployed to titan 2026-09-20: the replica restore that took 56m 2s now takes 19.3 s, and the refetch after a restart 8m 10s against 60m 15s, most of it now the download. The restart refetch is still unfixed; the restore publishes its share ten seconds after the first sync pass asks, so speed alone will not close it. Thumbnails then ran over titan's three real sources, leaving 99,137 tiles (jamie-nas 52,503, cctv 45,851, footage 783) in 21 GB of pvcache, after two defects. The `thumbnail` filter ran ahead of the scale, so it buffered thirty frames at source resolution: one 4264x2408 ProRes frame peaked at 1,290 MB and eight parallel decodes got the daemon SIGKILLed mid-job, against 233 MB with the scale first and one decode thread (`7847728c1`). The decode pool also sized itself from cores alone and is now bounded by free memory as well (`3b1bb2ba3`). Behind both, `NodeArena` mapped a `NamedTempFile`, which only `Drop` removes, so every daemon exit left about ten arena files behind; on titan `/tmp` is a tmpfs with no swap and 1,949 of them held 9.4 GB, leaving the daemon under 2 GB free. The backing file is now unnamed, so the kernel reclaims it however the process ends (`19b62033b`), and clearing the orphans took available memory from 1.9 GB to 11.4 GB. Tiles now cross to replicas (P6 of the replication plan, landed 2026-09-21): `thumbs.request` accepts a path on a paired device, answers at once with the version held locally, and fetches the rest from the owner over a new `FetchTiles` request, whose owner answers each ask as soon as it is ready with `Current`, a WebP tile (5 to 15 KB against 324 to 384 KB raw), or `Missing`, baking on demand what it lacks. Tiles land in `volumes/<owner volume>/thumbs.pvcache` beside local ones, so no client changed. Live between titan and the Mac, photos, CCTV clips and on-demand calvin-nas bakes landed within a second and survived a restart. P7 landed the same day as L5's first sidecar kind: every tile baked is also kept as WebP in its source's `sidecars.db`, keyed by record, the hot cache refills from it instead of rebaking, and replicas copy new sidecars page by page after each replication pass (`FetchSidecars`, by a per-store cursor and id). Titan converted its cache without decoding an original (97,728 sidecars in 1.05 GB against 21 GB of cache), the Mac copied all of them, and with titan's daemon stopped the Mac drew titan tiles it had never fetched from its own copies. |
| Mac client operating titan | live acceptance reopened | unowned | Pairing, remote CLI operations, jobs and logs remain functional. The September 15 evening audit found only eight titan replicas active on the Mac: dev-tools remains registered on titan with 28,079 records, but its snapshot is unavailable and its arena/watcher are absent. Preserve the earlier NAS freezes. Recovery and renewed acceptance are specified in `docs/plans/2026-09-15-source-runtime-reliability.md`; the earlier nine-source proof is historical. |
| Peer device facts | live on both machines (2026-09-18) | unowned | Titan runs `30d15bfa4` and publishes its hardware and nine volumes (eight datasets and the pool); on the Mac, `devices.list` carries titan's Ryzen 5 5500, 6 cores and 31 GB, `volumes.list` lists the nine volumes under titan, and all nine replicas fold into them, dev-tools onto `pool`. Next: look at titan's card in the app. The periodic reconnect now also dials peers still in the `Paired` state, so a peer that was down when this device started is reached once it returns; verified live, the Mac reconnected to titan 20 seconds after titan came back, with no restart. Open follow-up: a rename over an existing file inside a source fails to land in the store on `UNIQUE(parent_uuid, title)`; replacing `sd-server` on titan failed one content_identity job and left `sd-server.new` and the old `sd-server` as separate rows in jamie-nas. |
| Preserve NAS and pool evidence                    | pending                                | James           | Save the TrueNAS config, encryption result, pool topology, GUID, and SMART output described by the runbook.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Track the Expansion drive                         | pending                                | James           | Run one `sources track` pass over the Expansion drive before its contents merge into `jamie-nas`, then freeze it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Back up and export                                | pending                                | James           | Copy and verify `~/spacedrive/data` off titan (it now contains the freezes), then cleanly export the pool before the drives move.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Drive catalog H0                                  | plans committed, code missing          | unconfirmed     | The drive-catalog and physical-drives plans landed in `07fc633c2`, but the uncommitted `packages/drives` package earlier handoffs said to preserve is no longer in the checkout. Confirm whether that code survives elsewhere before restarting the package and archetype contract from the plan. The physical-identity firewall still applies.                                                                                                                                                                                                                                                                                                                       |
| Final entries drop                                | FD0–FD5 done; FDA ready                | Opus            | Execute `docs/plans/2026-09-15-entries-final-drop.md`. FD0 through FD4 are done (2026-09-18): no production code reads the entry tables, legacy row sync is gone, and a transactional migration drops 29 retired tables so every library converges on a 14-table schema. The Mac library upgraded live after a backup (`~/.spacedrive/backup/pre-entries-drop/library.db`); titan's upgraded on 2026-09-18 after a backup to `~/spacedrive/data/backup/pre-entries-drop/`. Locations are gone end to end: pins are Space items, Places come from `paths.system_folders`, and Add Storage tracks sources. FD5 is done: the ephemeral qualifier is retired (`VolumeIndex`, `Partition`, `Arena`, `core.index_status`), entry-era tests and examples are deleted or repointed, and the persistent-era search, event, status, and error surface is gone. Follow-ups are in its record: redundancy over source stores, and `indexing.start` walking the caller's path spelling. Next: FDA runs the acceptance matrix. Preserve source stores, paired-source replication, remote byte reads, remote operations, jobs, and logs. Remove locations without data migration or API compatibility.                                                                                                                                                                                                                                                                                                                     |
| Tags on source stores (FD2) | stretch 1 complete (Fable, 2026-09-18) | Fable | Landed in five commits through the outbox delivery fix. Titan redeployed per the runbook: its wedged `sd-server` (no library writes since Sep 16 05:21) was replaced by this build's `sd-daemon`; all ten outbox batches delivered on reconnect, acked and retired, titan's stores answer `files.by_tag`, and both libraries' legacy tag tables verified empty, clearing FD4's preconditions. Search phase 4 landed: store-resolved include/exclude proven exact by CLI, replica hits removed under a tag filter, Tags advertised, and a picker pill in the refinement bar. P6 done across `tagging.mdx`, `library.mdx`, the design doc, and the stats tag count. For James: relaunch `sd-server` on titan with `DATA_DIR=/mnt/pool/jamie-nas/spacedrive/data` and his `SD_AUTH`; it attaches to the running daemon. |
| Search refinement                                 | phases 1 and part of 2 committed       | in progress     | Execute `docs/plans/2026-09-15-search-refinement.md`. Landed: `fcf7f85d1` made sort, pagination, and filters honest (pipeline module with the R6 backend-neutral candidate stage, true `total_found` past the old 200 cap, case-folded extensions, hidden excluded by default, date filters fail closed; verified live with `--limit`, sorts, and offsets), and `fdf4902aa` fixed the debounce, preserved the picked scope across keystrokes, unified the length gate, and wired Cmd+F. `26fdf65c2` replaced the Location scope with a working Source scope resolved through `paths.context`, and `0c0226df3` plus `68bd52676` built the refinement bar: kind, extension, size, modified-date, and hidden filters with facet counts folded over the full match set and a true result count, recomposed as one row of `@spacedrive/primitives` (ToggleGroup scope, SelectPill popovers, Switch) where each pill carries its own state. Phase 4 tag filtering landed 2026-09-18 (store-resolved include/exclude, replica honesty, a Tags pill; results in the plan). Remaining: per-tab search state and search as a navigation target (deferrable no longer now that the shell work landed), and phase 5 redundancy-view verification. Inspect the live panel, then consider consolidating onto ts-client `useSearchFiles`. |
| Sidebar simplification                            | first slice committed (`321818b55`)    | unowned         | The sidebar has one library scope, flat daily destinations, Places computed from system folders without persisting them as locations, canonical rows, and an Activity/Settings footer. Topology groups retired; their pins survive seed convergence, and replica sources render under their owning device. Next: inspect the live library, then refine Home and Storage ownership from real use.                                                                                                                                                                                                                                                                    |
| Explorer path status                              | visual polish built, uncommitted; live proof pending | unowned    | `paths.context` distinguishes source-database existence from a committed record for the exact path. The PathBar uses neutral surfaces, floating Spacedrive source and volume assets, unfilled status chips, and large 36px circle actions for pinning, full-source reindex, navigation, and technical details. All explicit small CircleButton usages in Spacedrive are removed; the shared variant definition lives in the separate SpaceUI package. Pins rebuild from the registry's mounted source root. Earlier generated-client, focused Rust, scoped PathBar, and Fumadocs checks passed. The desktop frontend production build and scoped diff check pass after the styling change. Inspect the panel live. |
| Virtualized thumbnail remounts                    | committed (`d806ba7f1`); live proof pending | unowned    | Hot-tier bake completion epochs now survive cell unmounts, and each thumbnail hides its fallback only after its current DOM image loads. Media rows now keep stable React identity as the virtual window advances, width is measured before paint, content is no longer deliberately invalidated for every render, and live row-count changes no longer snap the camera roll to the bottom. The desktop production build passes. Scroll and trigger selection or inspector rerenders in a large media view to confirm the grid remains stable.                                                                                                  |
| Quick Preview originals                           | committed (`d806ba7f1`); live proof pending | unowned    | The Tauri asset boundary now maps canonical macOS APFS data-volume paths back to the scoped `/Users` and `/Volumes` aliases. Image, video, audio, text, mesh, and local thumbnail-original renderers share the fix. Open a home-directory image and an external-volume file to confirm originals load without a 403.                                                                                                                                                                                                                                                                                                                                                 |
| Permission-aware automatic startup                | first slice committed (`4b84e2c05`)    | unowned         | Library loading no longer dispatches a new filesystem discovery pass. The desktop requests one idempotent pass after the connected shell renders; a bare daemon does not initiate discovery, and `sd index start --defaults` gives CLI and server operators the same explicit path. Restored volume snapshots now prevent repeat full walks. Next: model denied coverage and add the macOS permission explanation and Full Disk Access affordance.                                                                                                                                                                                                                   |
| Host media tools                                  | committed (`bbc53866e`); live proof pending | unowned    | The machine-scoped registry discovers host FFmpeg outside GUI PATHs, reports its version and media capabilities, and exposes explicit Homebrew/WinGet installation. Services settings now shows the resolved version, path, capability coverage, missing FFprobe state, refresh, and a confirmed package-manager install action. Default builds use QuickLook or host FFmpeg for hot video thumbnails and generate immutable 5 by 5 thumbstrips on hover. Core, linked-feature, server, Tauri shell, generated-client, and desktop production builds pass; real FFmpeg output is 384×216 and 800×450 in the focused fixtures. Restart the daemon, inspect Services settings, then open a video with no old sidecars and confirm its poster and hover scrub sheet. |
| Opening remote files                              | O1, O2 (desktop), O4 (macOS) and O6 (Photos) landed 2026-09-24, uncommitted; live proof on titan next | Opus            | Plan: `docs/plans/2026-09-21-opening-remote-files.md`. Quick Preview streams another device's originals from `files.stream_url`, the mounts share's loopback URL; Open and double-click open them through `files.local_path`, which mounts the SMB share at `<data-dir>/mount` with NetFS and returns the file's path inside it; Photos follows a folder on another device (`search.media` pages replicas from their index) and shows a cell in Quick Look through the mount. The mount unmounts at shutdown, a killed daemon's mount is removed at the next start, and volume detection skips it. The SMB frontend served one file's bytes under another's name after a listing (every file id was 0 and search patterns were ignored); both are fixed and covered by tests. The HTTP share refuses non-loopback Host headers and answers CORS only for Tauri and loopback origins; no share key, since RPC on 6969 is open to local processes anyway. Verified end to end on two paired throwaway daemons. Titan runs this build since 2026-09-24 19:52 PDT (musl release, daemon sha256 `6c8a945d`); its previous binaries are `bin/sd-daemon.prev17` and `bin/sd-cli.prev5`, and its library, jobs and sync databases were backed up to `data/backup/pre-open-remote-files/` first. All nine local sources came back with the same item counts; the Mac's `Vault` replica did not, since titan's replica manifest held only the home source and no Vault artifact was on disk. Next, on the Mac against titan after restarting the Mac's daemon on this build: preview and scrub a titan video and read `mounts.cache_status`, Space on a titan clip in Photos, and open a titan clip larger than free disk in QuickTime. Open: O3 open by copy, O5 settings, the web client's `sd-server` route, Open With for remote files (needs a submenu that loads when the menu opens), and Show in Finder on sidebar items, which still takes a path unchecked. |
| File operations on preflight                      | F1 through F7 landed 2026-09-23; live acceptance next | unowned | Plan: `docs/plans/2026-09-23-file-operations.md`. Delete and duplicate in the explorer on preflight; rename and batch rename on preflight with filesystem name rules; the operation journal every mutating job writes, the trash with recorded locations and a Spacedrive trash directory where a volume has none, stashing on replace, and `files.undo` on preflight reached from the job list, ⌘Z and the trash view; mirror as `remove_extras` on merge; organize and flatten; zip and tar.zst archive and extract; attributes and links. CLI commands and dialogs for each, 30 tests over the fixture. Next: the acceptance case on the Expansion drive and a run of trash restore on Windows and Linux. |
| Action validation, preview, and folder merge      | V1 through V9 and dedupe landed; live acceptance on titan next | Opus            | Plan: `docs/plans/2026-09-22-action-previews.md`. Preflight (`validate:` and `preview:`), `FsPlan`, copy, move, merge, and delete on both methods, the merge job, dedupe as a delete target (first copy in walk order or chosen copies, read in full before removal) with `sd file dedupe` and a Duplicates screen, `sd file copy`, `merge`, and `delete` rendering preflight, the merge dialog, the modifier drop, the preflight-driven operation modal, plan handles, and overlay listings with the explorer's preview mode; the old validation hook is gone. Remote execution rides the existing `--device` forwarding. Two opt-in preflight methods on actions, both taking the action's exact input: `validate:` answers whether and how an operation runs (findings with stable codes plus execution facts, re-run server-side at dispatch, errors refuse) and `preview:` answers what will exist afterward (`FsPlan` from the index, advisory, browsable through plan handles as an overlay on `files.directory_listing`). The plan catalogs both methods across copy, move, merge, delete, batch rename, and dedupe. Folder merge is the first action on them: recursive merge with integrity-confirmed duplicate skipping, per-leaf conflict policy, optional source consumption, remote execution via `--device`. Also fixes copy's silent blind overwrite inside directory collisions, deletes the CLI's local conflict logic per the operations-are-the-unit principle, and removes the dead `ValidationResult`/`ConfirmationRequest` hook. Six decisions are James's. Landed ahead of preflight: `files.delete` takes a comparison as its targets (`sd file delete A --against B --show both`); the job derives the set from the index as it runs, reads both copies in full before removing one, checkpoints the compare cursor, and reports skips. |
| Retire Spacebot, Console, and storefront surfaces | blocked on exact private backup        | unowned         | The company tree contains product and research documents, but no exact copy of the current Spacebot UI, configuration, and design files was found. Back those up, then remove product-specific routes, configuration, UI, build aliases, generated types, and docs. Preserve generic filesystem operations, permissions, remote access, jobs, and logs.                                                                                                                                                                                                                                                                                                              |

## Product sequence after the NAS lock

1. Run Spacedrive on the Mac, pair it with titan, and operate NAS sources,
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

## Production run on titan

Verified live on 2026-09-15:

- All nine sources are indexed, hashed, verified, and frozen. Record counts:
  `jamie-nas` 217,884, `cctv` 49,263, `dev-tools` 28,079, `calvin-nas` 1,525,
  `footage` 1,380, plus the four small datasets. The September 14 resumed-walk
  incident was recovered by rebuilding the four affected stores before these
  counts were taken.
- Pending verification is zero in every store, checked directly against the
  store databases, not job status.
- Root-owned files in `cctv` (748 `content_error` rows) are accepted as
  metadata-only.
- The dated freeze copies live under each store's `freezes/` directory on the
  pool, timestamped `2026-09-15T0523Z`. They leave the building with the
  `~/spacedrive/data` backup.
- The cctv cameras write continuously, so the thirty-second watcher hash nudge
  dispatches steadily. This is expected behavior, not a stuck loop.
- The deployed daemon predates `48377348b`; that commit only adds the
  `sources.update` preference op, so redeploying can ride the next binary.
- The final index intentionally omits xattrs, Finder tags, and resource forks.

Do not treat this section as a live query. Re-check titan before acting and
replace the dated observations when its state changes.

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

## Safety invariants earned by the NAS run

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
  titan's single-library deployment and ambiguous with multiple open libraries.
- The 50 percent sweep fuse has no force path. A legitimate deletion of most of
  a source leaves stale rows for a later explicit recovery path to address.
- Non-UTF-8 names are retained lossily and logged. Review every warning before
  accepting a final archival index.
- The recurring permission warning for `/mnt/pool/ix-applications/docker` is
  known operational noise.

## Recent landed

### Interface and desktop

- `fe4a100b3`: made ephemeral search case-insensitive (lowercased registry
  keys, folded on snapshot restore too) and removed the exact/prefix/contains
  short-circuit so an exact name no longer hides its substring matches. Also
  stopped the double-indexed macOS home tree: the volume map skips hidden
  volumes (the sealed system volume at `/` was mapping the firmlinked tree a
  second time), and directory browse and scoped search normalize their paths
  through `locate_path` like `sources.track` does. Verified live: duplicates
  gone, mixed-case names found, full result sets on exact queries.
- `321818b55`, `dd5a0f1aa`, `d806ba7f1`, `bbc53866e`, `4b84e2c05`: the sidebar
  simplification, `paths.context` and the locations demotion, the thumbnail
  remount and Quick Preview fixes, host media tool discovery with on-demand
  thumbstrips, and explicit permission-aware startup discovery. Register rows
  above track their remaining live proofs and next steps.
- `38ff5c059`, `912f01274`: fixed hot-thumb 404s, silenced browse indexer
  jobs, and kept background hashing passes off the event bus.

### Documentation

- `07fc633c2`: moved docs to Fumadocs and rewrote the core pages around the
  daemon/source-store/library split, added `product-direction.mdx` and the
  drive-catalog plan, and retired the whitepaper page.
- `2c7e058ce`: README and CONTRIBUTING updates, locations and data-model pages
  reconciled, regenerated Tauri schemas, and the new icon asset.

### Multi-device

- `1044e0977`, `38a78bafc`, `171a80ecf`: cross-device search over paired
  devices, peer replicas surfaced in `sources.list` with remote listings
  served, and remote job completions delivered.
- `29703a5db`, `dedaba5c9`: stopped device presence flapping (a connection
  loss only marks a device disconnected when no connections remain),
  classified Tailscale CGNAT paths with their own badge, and made overview
  and sidebar request paired devices explicitly so a fresh page sees them.
  Verified live from titan: 20 consecutive samples online.
- `f2dbbac0d`: fleet-wide statistics are computed once by the owning device
  and reported identically everywhere; fixed the PATH-dependent ZFS capacity
  correction, stale persisted volume capacity, and double-counted source
  totals.
- `1d4afd204`: fixed peer snapshot serving to read from the volume index
  directory, restoring source replication between paired devices. Titan runs
  this build; the launch command must pass `--data-dir ~/spacedrive/data` or
  the daemon mints a fresh library at `$HOME/.spacedrive` on the pool.
- `bab963d8d`, `777dc327f`: wired remote job activity end to end. Paired
  devices are subscribed as they connect, the cache builds rows from started
  and progress events (nothing emits a queued event), `sd job remote` lists
  them, and terminal rows are swept hourly. Verified live in both directions.
- `996ec6422`: added `--device` to the CLI and the remote-ops protocol: any
  Wire action or query can be forwarded to a paired device and executed
  through the same registries, with the target resolving its own open
  library. Verified live: sources listed, jobs listed, and a verify job
  dispatched on titan from the Mac.
- `0f9307b62`: pairing completion broadcasts the connection so peer sync
  starts immediately; `sd --device <x> logs follow` streams a paired
  device's log bus with server-side filtering; `sd job remote --watch`
  follows remote jobs; stale remote job rows are swept by `updated_at`.

### NAS correctness and preservation

- `04ed3f3e7`: captured uid, gid, symlink targets, content errors, and invalid
  UTF-8 diagnostics.
- `93c3e515a`: added source freezing and the `sd sources` CLI domain.
- `301026ae1`: enabled self-contained Linux musl builds with vendored OpenSSL.
- `7a91f0034`: made content hashing wait for committed batches.
- `33437d3cd`: excluded the daemon data directory and salvaged failed batches.
- `b8c915db7`: derived volume IDs from fingerprints instead of process-local
  randomness.
- `09eaaa2b3`: added full-byte verification for shared sampled content.
- `166038bd5`, `cee729c22`: read Linux birth time through `statx` and corrected
  the kernel ABI buffer size.
- `a3fe7e0ec`: recorded ZFS pool topology in drive groups and tightened the
  pre-export evidence list.

### Restart, registry, and sweep safety

- `4feb6448a`, `72fe40684`: registered every job type, made startup
  reconciliation truthful, enabled it, deduplicated dispatch, and removed the
  task-system panic-on-drop source by logging it in release builds.
- `359521db7`, `50d712d08`: resolved verify and freeze roots through the source
  registry and persisted the volume rows those roots depend on.
- `023015552`: made walks restart from zero and added the destructive-sweep
  refusal.

### Reporting and live capture

- `d5beaf0c0`, `1a0539a91`, `38a455167`: implemented store-derived unique and
  indexed byte counts, indexed the content join, and made the duplicate bar
  honest under compressed filesystems.
- `fdc6e77f9`: made watcher events follow the source's persisted capture policy.
- `bec3c2ac6`: made watcher changes nudge content hashing.
- `48377348b`: typed source capture policy and added `sources.update` for
  renaming sources or widening their capture policy safely.

### Storage cutover

- `c30aa2365`: demoted locations to six-field pins.
- `554942ce8`, `d94f59f86`, `74f294625`, `79f0c385f`: stopped writing entries,
  moved reads to the arena and source stores, deleted the unreachable media
  pipeline, and removed most remaining production entry-era support.
- `0f5e4a1ef`: rewrote tag operations around record and content identity.

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
- NAS procedure and gates:
  [`docs/plans/2026-09-13-nas-index-runbook.md`](docs/plans/2026-09-13-nas-index-runbook.md)
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
system. The entries-teardown status table and the NAS runbook's known-issues
section also lag some September 14 fixes. Prefer the current tree, recent commit
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
