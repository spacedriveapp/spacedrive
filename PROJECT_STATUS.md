# Project Status

> Last updated: 2026-09-15
> Code baseline: `7c2d27bed`
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

Development focus moves to the product sequence below, starting with running
Spacedrive on the Mac and operating titan's sources from it.

In parallel, the worktree contains early drive-catalog and hardware-
representation work. Preserve it. Physical-drive identity remains separate
from product recognition and visuals.

## Immediate register

Update this table whenever work starts, lands, becomes blocked, or changes
owner. Keep only active and near-term work here. Move completed context to
Recent landed.

| Work                                              | Status                                 | Owner           | Next proof or action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------- | -------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mac client operating titan                        | complete, verified live                | done 2026-09-15 | Product-sequence step one is done: the Mac pairs with titan, replicates and streams all nine sources through the WebDAV peer mount (`1d4afd204`), operates it directly with `sd --device titan <any command>` (`996ec6422`), tails its logs with `sd --device titan logs follow`, and watches dispatched work with `sd job remote --watch` (`0f9307b62`). Pairing now triggers the initial peer sync immediately. Every leg verified live in both directions. Next per the sequence: package the proven CLI operations as a Spacedrive skill, then the MCP transport.                                                                                                |
| Preserve NAS and pool evidence                    | pending                                | James           | Save the TrueNAS config, encryption result, pool topology, GUID, and SMART output described by the runbook.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Track the Expansion drive                         | pending                                | James           | Run one `sources track` pass over the Expansion drive before its contents merge into `jamie-nas`, then freeze it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Back up and export                                | pending                                | James           | Copy and verify `~/spacedrive/data` off titan (it now contains the freezes), then cleanly export the pool before the drives move.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Drive catalog H0                                  | in progress, uncommitted worktree      | unconfirmed     | Finish the package and archetype contract without crossing the physical-identity firewall.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Sidebar simplification                            | first slice built, uncommitted         | current session | The active sidebar now has one library scope, flat daily destinations, Places, canonical rows, and an Activity/Settings footer. Places includes computed folders from the current device without persisting them as locations and renders them with Spacedrive's Folder and Home assets. Explorer view preferences are route-scoped again, so Photos and visual collections can default to Media without leaking that mode into folders or persisted tabs. The library menu now renders above sidebar rows while remaining below dialogs. The desktop production build passes. Next: inspect the live library, then refine Home and Storage ownership from real use. |
| Explorer path status                              | built, uncommitted; live proof pending | current session | `paths.context` now resolves APFS aliases and reports source, volume map, watcher, memory, restart-cache, source-store, availability, system Place, and nearest pin state. The PathBar visualizes containment instead of a false processing pipeline: the current folder sits inside its source, watching and persistence appear as outcome labels, and the physical volume sits outside that boundary. Runtime map terminology is hidden in collapsed technical details. Pins now rebuild from the registry's mounted source root, so they follow remounts. Focused Rust tests, the desktop production build, and the Fumadocs type and production builds pass. Restart the daemon and inspect Home, `/Applications`, a pinned folder, and a denied path. |
| Virtualized thumbnail remounts                    | fixed, uncommitted; live proof pending | current session | Hot-tier bake completion epochs now survive cell unmounts, and each thumbnail hides its fallback only after its current DOM image loads. Media rows now keep stable React identity as the virtual window advances, width is measured before paint, content is no longer deliberately invalidated for every render, and live row-count changes no longer snap the camera roll to the bottom. The desktop production build passes. Scroll and trigger selection or inspector rerenders in a large media view to confirm the grid remains stable.                                                                                                  |
| Quick Preview originals                           | fixed, uncommitted; live proof pending | current session | The Tauri asset boundary now maps canonical macOS APFS data-volume paths back to the scoped `/Users` and `/Volumes` aliases. Image, video, audio, text, mesh, and local thumbnail-original renderers share the fix. Open a home-directory image and an external-volume file to confirm originals load without a 403.                                                                                                                                                                                                                                                                                                                                                 |
| Permission-aware automatic startup                | in progress, uncommitted               | current session | Library loading no longer dispatches a new filesystem discovery pass. The desktop requests one idempotent pass after the connected shell renders; a bare daemon does not initiate discovery, and `sd index start --defaults` gives CLI and server operators the same explicit path. Restored volume snapshots now prevent repeat full walks. Next: model denied coverage and add the macOS permission explanation and Full Disk Access affordance.                                                                                                                                                                                                                   |
| Host media tools                                  | built, uncommitted; live proof pending | current session | The machine-scoped registry discovers host FFmpeg outside GUI PATHs, reports its version and media capabilities, and exposes explicit Homebrew/WinGet installation. Services settings now shows the resolved version, path, capability coverage, missing FFprobe state, refresh, and a confirmed package-manager install action. Default builds use QuickLook or host FFmpeg for hot video thumbnails and generate immutable 5 by 5 thumbstrips on hover. Core, linked-feature, server, Tauri shell, generated-client, and desktop production builds pass; real FFmpeg output is 384×216 and 800×450 in the focused fixtures. Restart the daemon, inspect Services settings, then open a video with no old sidecars and confirm its poster and hover scrub sheet. |
| Product documentation and Fumadocs conversion     | complete, uncommitted worktree         | James           | The README now states the finished product contract. The docs type check and production build pass. Review and commit the migration, product doctrine, README, and offline build fix without absorbing unrelated drive or generated-schema work.                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Retire Spacebot, Console, and storefront surfaces | blocked on exact private backup        | unowned         | The company tree contains product and research documents, but no exact copy of the current Spacebot UI, configuration, and design files was found. Back those up, then remove product-specific routes, configuration, UI, build aliases, generated types, and docs. Preserve generic filesystem operations, permissions, remote access, jobs, and logs.                                                                                                                                                                                                                                                                                                              |
| Reconcile published documentation                 | complete, uncommitted worktree         | current session | The overview, core architecture, storage, networking, sync, CLI, and extension pages now present the October release and source-store model. Fumadocs type-checks, all root-relative doc links resolve, and the production build generates 185 routes. Review and commit with the rest of the docs migration.                                                                                                                                                                                                                                                                                                                                                        |

## Product sequence after the NAS lock

1. Run Spacedrive on the Mac, pair it with titan, and operate NAS sources,
   jobs, and logs through the same CLI.
2. Package the proven CLI operations as a Spacedrive skill. Build the MCP
   transport after the tool schemas have been exercised.
3. Simplify the web sidebar and inspector around primary filesystem concepts.
   Audit every sidebar row for use of the canonical component.
4. Restore source-scoped persistent sidecars, streaming previews, and video
   scrubbing through the byte-provider path.
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
- The persistent `entry` indexing and read paths have been removed. Do not
  restore or deepen them.
- A location is a pin over a source-relative path. It does not own records,
  trigger indexing, or enable a watcher. A future policy may target its stable
  ID, but the existence of the pin must not imply that policy.
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

### Multi-device

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

At this update, the checkout already contained user-owned uncommitted work in:

- `docs/plans/2026-09-13-drive-catalog.md`
- `docs/plans/2026-09-13-physical-drives.md`
- `packages/drives/`
- `CLAUDE.md`
- `justfile`
- generated Tauri capability schemas
- the completed Fumadocs conversion under `docs/`

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
