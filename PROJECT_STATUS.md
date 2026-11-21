| macOS CLI signing in release builds | in progress 2026-11-21 (FAN-12143) | Opus | `cli-build` in `release.yml` signs the macOS `sd` and `sd-daemon` with the Developer ID certificate, hardened runtime and a secure timestamp, submits them to notarization as a zip (a bare binary cannot be stapled), and fails the job unless each passes `codesign --verify --strict`, carries a Developer ID authority, a Team ID, the runtime flag and a timestamp, and passes `spctl -a -t install`. Runs on a tag and in a signed dry run. Checksums are taken after signing. Next: cherry-pick onto `release/beta.1` once merged. |
# Project Status

> Last updated: 2026-11-21
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

Content reads never hydrate cloud placeholders (Capy SPAC-86, November 19,
beta.1 blocker). Every path that reads file bytes stats the file first and
refuses one macOS reports `SF_DATALESS`
(`ops::indexing::metadata::refuse_dataless`): content identification, the
integrity tier, thumbnails and thumbstrips, EXIF, the extension record reads
and the byte plane's `LocalProvider`. Hashing records a placeholder with the
`content_error` "cloud placeholder: contents not downloaded" and keeps the
walk's metadata. Hashing reads run on their own runtime at background QoS on
macOS (nice 10 on Linux), and its progress total grows when records arrive
mid-pass. A source walk applies rules and so never sweeps its store, so the
reconcile now removes what the map holds under a directory today's rules
exclude, outside the removal fuse, and the watcher rejects a path when any
ancestor below the root is excluded (`/Users/*/Library` names only the
directory). Open: a placeholder downloaded later keeps its error until the file
changes or its volume remounts; the grep op held on `capy/search-grep` (#3219)
must call `refuse_dataless` before it opens a file; Quick Preview of a local
file loads through Tauri's asset protocol, which the daemon never sees.

Watcher snapshot flushes are paced by snapshot size (Capy SPAC-83, November
19, beta.1 blocker). #3181 rewrote a drive's whole arena snapshot every 30 s
whenever its arena was dirty, which on a 1.8M-record home source with constant
cache churn came to about 11 GB written per hour. The periodic flush now waits
`size / 512 MiB per hour` between saves of one drive, so a 109 MB snapshot is
written about every 13 minutes; shutdown and walk saves still write at once,
and the save runs on a blocking thread. A crash between saves restores the
older snapshot and the warm-start reconcile applies what it missed
(`a_snapshot_older_than_the_last_changes_is_reconciled_on_restart`).

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
| Release candidate validation (cloud) | run 2026-11-05 on nightly `26b57e1` (SPAC-58); steps 1 to 4 and 7 run, record in `docs/core/acceptance/candidate-validation.md` | Opus | Steps 1 (Linux), 2, 3 and 7 pass. A v2.0.0-alpha.2 library opens but loses its locations, files, tags and collections (table in the record). Windows clean install fails until a nightly with #3169 is published; then rerun the smoke workflow from #3166. Step 4 fails. Fixed and merged: truncated file left by a killed copy (#3170), `--instance` data dir nesting (#3171), store migrations missing from the log (#3168), Windows verbatim paths (#3169), unhealed first walk (#3172), thumbnail scope (#3174), library statistics (#3173, #3182, #3183), `logs show` path (#3175). Nightly builds cancelled by every push (#3186, merged). In review: CLI panic on a closed pipe (#3188). The heal now resolves registered sources directly, so sources on hidden volumes (Linux `/`, `/home`, overlay roots) heal too (#3192, `hidden_volume_heal_acceptance_test`). Changes made while the daemon is stopped are reconciled at startup by the warm-start reconcile from `zero-onboarding-startup.md` (SPAC-63, `stopped_daemon_reconcile_acceptance_test`; 16 s cold on the 1M-file tree against a 125 s walk). Open: a full disk hangs the indexer. Remaining: macOS installs and signing, real prior-version Mac state, a cross-network pair, remote operations, update signatures and rollback, release notes and checksums. |
| Startup reconcile on a large macOS home source | in review 2026-11-19 (SPAC-84, FAN-11141) | Opus | On the beta.1 MacBook the startup reconcile of a 1.8M-entry home source ran past 35 minutes with nothing visible. It read every directory the map held, including `~/Library`, which the map kept from walks before the Nov 5 `Users/*/Library` rule, and iCloud listings in it timed out (os error 60). It also checked each new entry with a ruler built from that entry's own path, which skipped `.gitignore` for files, so every pass added gitignored files to the store (10,000 of them on the generated tree); the watcher shared that check and now builds it for the file's directory too. The pass now builds one ruler per directory as the walk does, leaves held directories the rules now exclude unread, never lists a cloud provider placeholder (`SF_DATALESS`; the walk and its summary count skip them too), bounds each listing at 10 s, and runs as a `reconcile` job with progress and an ETA. Every skip logs at info. Next: SPAC-79 relaunch on the MacBook to confirm the time and the `excluded`/`placeholders` counts. |
| Candidate validation follow-ups | in progress 2026-11-05 (SPAC-64); one PR per finding against `main` | Opus | Four findings from the SPAC-58 run. A full disk hangs the indexer: reproduce on a loop-mounted data dir with a debug build, fail the job with a disk-full error and leave the store consistent, with an acceptance test on the loop-device helper. The receive side of a peer transfer writes in place: stage beside the destination and rename on verified completion as #3170 does for local copies, keeping the #3109 resume rules, with a kill-mid-receive case in the two-core transfer suite. `sd daemon install` without systemd or launchd now says so and prints the command to run instead. The README names Windows as a preview platform, matching the beta.1 fact sheet. |
| Agent grants core (G1) | in review 2026-11-19 (FAN-11130); held until v2.0.0-beta.1 is tagged | Opus | A grant lends one directory of a tracked source to an agent: `grants.create/list/revoke/extend/audit`, `sd grant`, tokens stored as SHA-256 and shown once, 5 min to 12 h lifetimes, revocation on the next request. `ops::grants::authorize` is the one enforcement point; it resolves paths by filesystem identity (`..`, symlinks out, firmlink and case aliases) and keeps writable grants off AGENTS.md, CLAUDE.md, SKILL.md and .git. Bounded audit table. Design in `docs/core/design/agent-grants.md`; `agent_grants_acceptance_test` and the sd-cli `grant_test`. Next: the agent MCP HTTP surface (M1) builds on `authorize`. |
| Track on a full data disk | in review 2026-11-06 (SPAC-68, FAN-5078) | Opus | `sources.track` on a nearly full data disk registered the source, failed to queue its walk, returned success, and the heal never walked it because the root stayed flagged as indexing. The store is now marked walk-unfinished before the walk is queued, a failed dispatch clears the indexing flag, a walk whose store cannot open is refused, and the action returns an error saying the walk will retry at the next start. Proof: `track_full_data_disk_acceptance_test`. |
| macOS Data volume read as unmounted | in review 2026-11-13 (SPAC-73, FAN-6659) | Opus | On macOS 26.1, `/`, `/System/Volumes` and `/System/Volumes/Data` report the same `st_dev`, so the device comparison in `is_mount_point` (from #3151) called the Data volume unmounted and every walk and browse under `/Users` was refused. macOS and iOS now judge a mount point by `statfs` `f_mntonname`; Linux keeps the device comparison. A refused first walk now fails `sources.track` with the reason, and a store kept in the library is marked walk-unfinished so the heal retries. Proof: `statfs_mount_point_tests` (Linux and Mac), `track_refused_walk_acceptance_test` (its macOS case ran on the Mac Studio), and a real daemon on the Studio walking a folder under `/Users`. |
| Case-only renames and firmlink spellings on macOS | in review 2026-11-13 (SPAC-74, FAN-6716) | Opus | SPAC-72 found that on APFS a case-only rename (`Readme.md` to `README.md`, `keep` to `Keep`), offline or live, left the old spelling as a second record, because the reconcile's removal re-check and the macOS watcher decided presence by stat, and APFS answers a stat for any case of a name. Presence is now decided by the name the directory stores (`sd_fs_watcher::present_as_spelled`: `getattrlist` on macOS, the parent listing elsewhere): the reconcile's re-check uses it, the macOS watcher classifies single-path rename events with it and pairs a remove with an earlier create by inode, and the change handler checks it on macOS. Case-sensitive volumes still keep `Readme.md` and `README.md` apart. `file info` resolves a path through the volume's spelling (`VolumeManager::index_spelling`), so `/Users/...` finds the record held under `/System/Volumes/Data/...` and answers in the spelling asked; the CLI makes relative paths absolute, exits non-zero when nothing is found, and prints search results through the firmlink (`short_spelling`). Proof: `spelling` unit tests with case folding and the firmlink table injected, `path_spelling_acceptance_test` (casefold ext4 on GitHub's runner, the temp dir on a Mac), macOS watcher tests, and a daemon on the Studio. |
| Acceptance flakes on main | in PR 2026-11-06 (SPAC-67); one PR against `main` | Opus | Two acceptance tests failed on main without a code change. `wasm_extension_test::extension_job_resumes_after_restart` (2c4da34) read the guest's checkpoint as soon as the job reported Paused, but Paused is reported when the pause is requested and the guest can land one more checkpoint before it stops; resume itself was right. The test now reads the checkpoint after shutdown, which waits for the job's last save. `disk_full_acceptance_test` (bcc2fa9) filled the disk the whole data dir was on, so the walk's job-row insert in `jobs.db` could hit SQLITE_FULL before the walk began; only the source stores now sit on the filled volume. Finding, not fixed: on a real install whose data dir disk is full, `sources.track` can register the source and skip the walk with only a log line, and the heal never walks it once there is room. |
| Disk-full integrity check | in PR 2026-11-20 (SPAC-87, FAN-11245); one PR against `main` | Opus | `disk_full_acceptance_test` failed once on #3219 with `PRAGMA integrity_check` returning "disk I/O error" (code 4874, `SQLITE_IOERR_SHMSIZE`). A connection that opens a WAL store on a full volume has to rebuild the `-shm` index and has no free blocks for it, so it fails before reading a page; the store is not damaged (a copy and an in-place check after freeing space both pass). The test now checks the store as the full disk left it through a copy on the regular disk, and `SQLITE_IOERR_SHMSIZE` counts as disk full, so a salvage stops at the first such row. 100 of 100 runs passed on main and 60 of 60 on the change. |
| Source runtime reliability | R1-R4, R6, R7 and the R8 acceptance matrix landed | in progress | Execute `docs/plans/2026-09-15-source-runtime-reliability.md`. Landed: destructive paths removed, flush made a durability barrier, self-nudge loop stopped and hashing progress labeled (`bb033de1b`, `954c6d980`, `fb0dab3e6`); R1 ancestor synthesis (`9242a14a1`); R2 anchored volume roots (`0aa0d390c`); R3 persisted replica inventory (`59b05a6c8`) plus the fetch-generation fix (`98e8a1485`: header names the generation and blake3 of the bytes delivered, a delivery must parse before it replaces a good artifact, one failed source no longer aborts the device sync, fetch failures back off, an unchanged generation still refreshes owner facts, revocation unloads and quarantines replicas, snapshot decode is bounded); the coverage heal (`5a65d0998`); watchers made unconditional on a bare daemon (`8c8499413`, `847144ba0`, `3ee5449bf`); dirty-driven refetches paced to five minutes (`4a690c84a`). R6 (results in the plan): `9e29e7c46` opened stores read-only with listings, lookups and Unicode-folded name search; `4cf25c17c` routes one backend per source (arena when restored or walked, SQLite otherwise, no empty-result retry, `total_is_exact` on the wire, detached directories list from their stores); `5f1fdd287` delivers a nested source's replica as its own database with the snapshot header contract; the 100-store/1M-record cold fan-out measures 1.7 s (`33bd1ce5d`). R7 (results in the plan): stores validated by direct SQL with zero dangling ancestry, a cold restart with the owner down serves and searches every replica, byte read, remote op and remote log stream verified, and reconvergence transfers zero bytes. Since then: musl builds use mimalloc as the global allocator, since musl's allocator gives each allocation group its own mapping and exhausted the kernel's map limit within an hour of a restart; a nested source's generation is the store's revision, a count of committed row changes ... (line truncated to 2000 chars)
| Drive catalog H0                                  | plans committed, code missing          | unconfirmed     | The drive-catalog and physical-drives plans landed in `2d380fd26`, but the uncommitted `packages/drives` package earlier handoffs said to preserve is no longer in the checkout. Confirm whether that code survives elsewhere before restarting the package and archetype contract from the plan. The physical-identity firewall still applies.                                                                                                                                                                                                                                                                                                                       |
| Final entries drop                                | FD0–FD5 done; FDA matrix landed, all rows pass on Linux | Opus            | Execute `docs/plans/2026-09-15-entries-final-drop.md`. FD0 through FD4 are done (2026-09-18): no production code reads the entry tables, legacy row sync is gone, and a transactional migration drops 29 retired tables so every library converges on a 14-table schema. Both live libraries upgraded after backups. Locations are gone end to end: pins are Space items, Places come from `paths.system_folders`, and Add Storage tracks sources. FD5 is done: the ephemeral qualifier is retired (`VolumeIndex`, `Partition`, `Arena`, `core.index_status`), entry-era tests and examples are deleted or repointed, and the persistent-era search, event, status, and error surface is gone. FDA landed as `docs/core/acceptance/entries-drop-and-file-operations.md`: every row of the Acceptance section mapped to a CI test (`core/tests/entries_drop_acceptance_test.rs`: fourteen-table schema on a fresh and on a pre-drop library, no production reference to the retired substrate, registered sync models name current tables, pins are Space items with no indexing side effect and survive a restart, tags survive a reindex, a frozen store is self-describing and unwritten by a reader, an incompatible library is refused intact, non-UTF-8 names retained lossily and warned). The two rows that failed when the matrix landed are fixed: Linux trash restore records the item under `Trash/files` and takes the `.trashinfo` with it on restore and purge, and a rename over an existing file removes the overwritten row in the batch that lands the move (the matrix's F-a, F-b). `KeyManager::close` no longer creates a redb file named `:memory:` in the working directory, so the pin restart row runs in process. Not automatable on the runner: a pin following a volume remount, remote job watch and log stream (live in SPAC-12), the `wasm` and `sd-native` check lines. Follow-ups are in its record: redundan... (line truncated to 2000 chars)
| Tags on source stores (FD2) | stretch 1 complete (Fable, 2026-09-18) | Fable | Landed in five commits through the outbox delivery fix. Verified against a paired NAS: queued outbox batches delivered on reconnect, acked and retired, the owner's stores answer `files.by_tag`, and both libraries' legacy tag tables verified empty, clearing FD4's preconditions. Search phase 4 landed: store-resolved include/exclude proven exact by CLI, replica hits removed under a tag filter, Tags advertised, and a picker pill in the refinement bar. P6 done across `tagging.mdx`, `library.mdx`, the design doc, and the stats tag count. |
| Search refinement                                 | phases 1 and part of 2 committed       | in progress     | Execute `docs/plans/2026-09-15-search-refinement.md`. Landed: `af3470373` made sort, pagination, and filters honest (pipeline module with the R6 backend-neutral candidate stage, true `total_found` past the old 200 cap, case-folded extensions, hidden excluded by default, date filters fail closed; verified live with `--limit`, sorts, and offsets), and `f34fc3c8a` fixed the debounce, preserved the picked scope across keystrokes, unified the length gate, and wired Cmd+F. `7733fe8a9` replaced the Location scope with a working Source scope resolved through `paths.context`, and `81accc3ad` plus `3f4b67100` built the refinement bar: kind, extension, size, modified-date, and hidden filters with facet counts folded over the full match set and a true result count, recomposed as one row of `@spacedrive/primitives` (ToggleGroup scope, SelectPill popovers, Switch) where each pill carries its own state. Phase 4 tag filtering landed 2026-09-18 (store-resolved include/exclude, replica honesty, a Tags pill; results in the plan). Remaining: per-tab search state and search as a navigation target (deferrable no longer now that the shell work landed), and phase 5 redundancy-view verification. Inspect the live panel, then consider consolidating onto ts-client `useSearchFiles`. |
| Agent writes (W1) | in review 2026-11-21 (SPAC-88, FAN-11136); core ops first, MCP write tools after #3223 | Opus | `agent.write_file` (whole file and byte range), `agent.make_dir`, `agent.move`, `agent.delete` and `agent.stat`, each checked through `ops::grants::authorize` with a write scope, so instruction files and `.git` stay refused. Single-owner write-through: a change names the store revision or the BLAKE3 content hash it read and is refused as stale with the current state; a create never replaces and keeps both as `name (agent conflict N).ext`; no merging. Bytes stage beside the target and rename with no-replace; replaced, moved and deleted items go to the trash, and each write is journaled under an id `files.undo` reverses. Spec and acceptance matrix in `docs/core/design/agent-writes.md`; proof `agent_writes_acceptance_test`. Next: `write_file`, `make_dir`, `move` and `delete` MCP tools on the #3223 endpoint, with `content_hash` on its read results. |
| Live content search (`search.grep`) | in review 2026-11-19 (FAN-11132, agents X1) | Opus | `search.grep` searches file contents under a path inside a source tracked on this device, live, with the ripgrep crates and no index: regex or literal, case, include and exclude globs, `.gitignore`, hidden files, context lines, a per-file byte cap (1 MiB default), and cursor pages over a sorted walk. Binary files and files over the cap are skipped, symlinks are never followed, a scope that resolves outside its source is refused, and another device's path says it is not available on this device yet. `sd search grep <pattern> [path]`. The scan takes a plain path so the grant layer can pass its subtree. Proof: `grep_acceptance_test`. |
| Sidebar simplification                            | first slice committed (`2097fc141`); proven on Linux web 2026-11-05 (SPAC-60); macOS pending | unowned         | The sidebar has one library scope, flat daily destinations, Places computed from system folders without persisting them as locations, canonical rows, and an Activity/Settings footer. Topology groups retired; their pins survive seed convergence, and replica sources render under their owning device. Next: inspect the live library, then refine Home and Storage ownership from real use. Linux web live 2026-11-05 (SPAC-60): one library scope, Places, Storage and Trash render on fixture data, and a detached drive shows Unmounted. |
| Explorer path status                              | proven on Linux web 2026-11-05 (SPAC-60); macOS pending | unowned    | `paths.context` distinguishes source-database existence from a committed record for the exact path. The PathBar uses neutral surfaces, floating Spacedrive source and volume assets, unfilled status chips, and large 36px circle actions for pinning, full-source reindex, navigation, and technical details. All explicit small CircleButton usages in Spacedrive are removed; the shared variant definition lives in the separate SpaceUI package. Pins rebuild from the registry's mounted source root. Earlier generated-client, focused Rust, scoped PathBar, and Fumadocs checks passed. The desktop frontend production build and scoped diff check pass after the styling change. Inspect the panel live. Linux web proof (SPAC-60): the PathBar shows source and volume status for a tracked folder and for a loop-mounted ext4 volume. |
| Virtualized thumbnail remounts                    | committed (`4e53d58db`); Linux web Media view fails (SPAC-60); macOS pending | unowned    | Hot-tier bake completion epochs now survive cell unmounts, and each thumbnail hides its fallback only after its current DOM image loads. Media rows now keep stable React identity as the virtual window advances, width is measured before paint, content is no longer deliberately invalidated for every render, and live row-count changes no longer snap the camera roll to the bottom. The desktop production build passes. Scroll and trigger selection or inspector rerenders in a large media view to confirm the grid remains stable. SPAC-60 on Linux web: Grid keeps every visible tile loaded across scroll and inspector rerenders, but the Media view over 2,400 photos requests `/hot-thumb/` with the drive's volume index id as the source id and gets 404 for every cell. |
| Quick Preview originals                           | Linux web and Tauri images proven with #3176 and #3180 (SPAC-60); Tauri video on Linux and macOS pending | unowned    | The Tauri asset boundary now maps canonical macOS APFS data-volume paths back to the scoped `/Users` and `/Volumes` aliases. Image, video, audio, text, mesh, and local thumbnail-original renderers share the fix. Open a home-directory image and an external-volume file to confirm originals load without a 403. SPAC-60 on Linux: the web client had no original URL at all (#3176 proxies the mounts share through sd-server), and the Tauri asset scope refused `/mnt`, `/media`, `/run/media` and hidden directories (#3180). With both, a home video plays on web and an external-volume image opens in Tauri. Video in Tauri under WebKitGTK stays at 0:00 with no scope error, even with `gstreamer1.0-libav` installed. |
| Permission-aware automatic startup                | first slice committed (`c32768f2e`)    | unowned         | Library loading no longer dispatches a new filesystem discovery pass. The desktop requests one idempotent pass after the connected shell renders; a bare daemon does not initiate discovery, and `sd index start --defaults` gives CLI and server operators the same explicit path. Restored volume snapshots now prevent repeat full walks. Next: model denied coverage and add the macOS permission explanation and Full Disk Access affordance.                                                                                                                                                                                                                   |
| Host media tools                                  | Linux: host FFmpeg and video posters proven (SPAC-60); scrub sheet and package-manager install pending | unowned    | The machine-scoped registry discovers host FFmpeg outside GUI PATHs, reports its version and media capabilities, and exposes explicit Homebrew/WinGet installation. Services settings now shows the resolved version, path, capability coverage, missing FFprobe state, refresh, and a confirmed package-manager install action. Default builds use QuickLook or host FFmpeg for hot video thumbnails and generate immutable 5 by 5 thumbstrips on hover. Core, linked-feature, server, Tauri shell, generated-client, and desktop production builds pass; real FFmpeg output is 384×216 and 800×450 in the focused fixtures. Restart the daemon, inspect Services settings, then open a video with no old sidecars and confirm its poster and hover scrub sheet. SPAC-60 on Linux web: Services shows `/usr/bin/ffmpeg` 6.1.1 with video thumbnails, media metadata and timeline previews, and video posters render. Hovering a video requested no thumbstrip. |