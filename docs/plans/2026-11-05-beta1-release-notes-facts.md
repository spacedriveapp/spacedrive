# Spacedrive 2.0 beta.1: release notes fact sheet

> Status: facts gathered 2026-11-05 from `main` at `26b57e1`; updated
> 2026-11-14 for `main` at `3e41e4a` (#3203 to #3213)
> Owns: the facts the beta.1 release notes state. The prose is
> `docs/core/release-notes/beta-1.mdx`.
> Companions: `docs/core/releases.mdx` (gates and what release notes must state),
> `docs/plans/2026-09-17-beta-launch.md` (narrative),
> `docs/plans/2026-10-09-windows-gap.md` (Windows scope)

Every line below names where it comes from. A line marked "open" needs a
decision before the notes are written.

## Version

- Every product package is `2.0.0-beta.1`, the files `cargo xtask bump`
  edits: `core`, `apps/cli`, `apps/server`, `apps/tauri/src-tauri`,
  `apps/tauri/sd-tauri-core`, `tauri.conf.json` and `apps/tauri/package.json`.
  `sd --version` prints `2.0.0-beta.1 (<sha>)`. The tag is `v2.0.0-beta.1`;
  the release workflow's updater manifest job fails if the tag and
  `tauri.conf.json` disagree.
- The previous 2.0 release is `v2.0.0-alpha.2` (prerelease, 2026-02-07). Its
  tag is not an ancestor of `main`: `main` was fast-forwarded to the
  rewritten `sources` branch on 2026-10-05, so "changes since alpha.2" is the
  whole data model, not a list of commits.
- GitHub's "Latest" release is still `0.4.3` (v1 desktop, 2025-03-24).
  Decided (Jamie, 2026-11-13): beta.1 is published as a full release marked
  Latest, not as a prerelease, and replaces `0.4.3` as Latest. The notes link
  https://github.com/spacedriveapp/spacedrive/releases/tag/0.4.3 for anyone
  who needs the v1 app.

## Supported platforms

Decided (Jamie, 2026-11-13): beta.1 supports Linux and macOS only. Windows is
out of beta.1; the release workflow builds no Windows artifacts, and Windows
builds of the CLI and daemon stay in the nightly.

| Platform | beta.1 status | Evidence |
|---|---|---|
| macOS, Apple silicon | supported | macOS cargo check gates `main` (#3154); live runs on a Mac |
| Linux x86-64 | supported | unit, integration and acceptance suites in `core_tests.yml` run here |
| Windows x86-64 | not in beta.1; CLI and daemon in the nightly only | builds and passes the unit layer (`Core Tests (windows unit)`, #3148, #3153); nightly binaries published; no desktop app; FDA, R8 and volume acceptance suites have not run on Windows |
| macOS x86-64, Linux ARM64 | built, not tested | `release.yml` builds them; no test suite runs on either |

Proposed Windows wording (verbatim from the gap doc):

> Spacedrive 2.0 beta.1 supports macOS (Apple silicon) and Linux (x86-64).
> Windows is not part of this beta. The daemon and CLI build and pass their
> unit tests on Windows, and nightly builds for Windows x86-64 are published
> for people who want to try them, but the file operation, source runtime and
> drive acceptance suites have not been run there, so we do not yet make the
> data safety claims this release makes for macOS and Linux. Windows support
> is planned for a later beta.

### Artifacts the release workflow produces

From the `release.yml` dry run (see "Release workflow dry run" below):

| Product | Platforms | Signed |
|---|---|---|
| Desktop `.deb` | Linux x86-64, ARM64 | no (Debian packages carry no signature in this pipeline) |
| Desktop `.dmg` / `.app` | macOS ARM64, x86-64 | Developer ID and notarized in a release (the job fails otherwise); ad-hoc signed in a dry run |
| `sd` and `sd-daemon` | macOS ARM64 and x86-64, Linux x86-64 and ARM64 | no Developer ID; a `.sha256` beside each, which `sd update` verifies |
| `sd-server` and `sd-cli` tarball | Linux x86-64, ARM64 | no; `.sha256` inside the tarball |

- Linux desktop packages are built on Ubuntu 24.04 runners, so the `.deb`
  needs glibc 2.39 or newer. The CLI, daemon and server are built on Ubuntu
  22.04 (glibc 2.35).
- The CLI and daemon carry sd-core's default features: no bundled FFmpeg,
  libheif or Pdfium. macOS reaches those formats through system codecs. The
  server tarball is built with `heif`, `ffmpeg` and `ai`
  (`release.yml`, Server job).
- The container image comes from `server.yml`, a separate workflow not
  covered by the dry run.

### Filesystems

From `docs/core/filesystems.mdx`, Support Matrix: APFS and HFS+ (macOS);
ext2/3/4, XFS, Btrfs and ZFS (Linux); FAT32 and exFAT (all). Copy-on-write clones
on APFS, Btrfs and ZFS. NTFS and ReFS are Windows-only and so fall under the
preview. Known limits from that page: leaf-only ZFS dataset tracking, Btrfs
nested subvolumes from Docker or snapper are not hidden, NFS and SMB mounts
have no protocol-aware capacity or CoW handling, and encrypted volumes are
seen as whatever filesystem sits on top once unlocked. Locked or unmounted ZFS
datasets show as Locked or Unmounted (#3140, #3141).

## Schema and on-disk format changes since alpha.2

### library.db

Opening an alpha.2 library runs these migrations, which come after alpha.2's
last one (`m20260123_000001_remove_legacy_sync_columns`), in order. Since
#3147 all pending migrations run in one transaction, so a failure leaves the
library as it was.

| Migration | Effect |
|---|---|
| `m20260125_000001_unique_user_metadata_tag` | unique index on user metadata tags (table later dropped) |
| `m20260414_000001_add_redundancy_indexes` | index on `entries` (table later dropped) |
| `m20260417_000001_add_entries_sync_cursor_index` | index on `entries` (table later dropped) |
| `m20260825_000001_create_sources` | the source registry becomes a table in `library.db` |
| `m20260909_000001_add_content_count_to_sources` | distinct content count per source |
| `m20260910_000001_locations_are_pins` | the old `locations` table is dropped; a location becomes a pin (name, path inside a source). No rows carry over |
| `m20260917_000001_create_tag_staging` | tags created but not yet applied to anything |
| `m20260917_000002_create_assertion_outbox` | tag writes against another device's source, held until that device acknowledges them |
| `m20260918_000001_drop_entries_world` | drops entries, content identities, media data, sidecars, collections, tags, user metadata, indexer rules and the FTS search index; deletes Location-typed Space items and Locations groups. No transfer step; no down migration |

Migration behaviour for an alpha.2 library, as the code reads:

- Library identity, devices, volumes, spaces and non-location Space items
  survive.
- Locations, the file index, tags, collections, user metadata and indexer
  rules from alpha.2 do not. Folders and drives have to be added again with
  Add to Library, which reindexes them into source stores. This is the agreed
  breaking transition (`docs/core/releases.mdx`, What beta means).
- Candidate validation step 2 opened a real alpha.2 library with a nightly
  and recorded the result table by table
  (`docs/core/acceptance/candidate-validation.md`, From v2.0.0-alpha.2). It
  matches the two lines above.
- beta.1 does not find an alpha.2 library on its own. alpha.2 resolved its
  data directory with `dirs::data_dir()` on macOS
  (`~/Library/Application Support/spacedrive`) and `dirs::data_local_dir()`
  on Linux (`~/.local/share/spacedrive`) (`core/src/config/mod.rs` at
  `v2.0.0-alpha.2`). `main` uses `~/.spacedrive` on both
  (`core/src/config/mod.rs` and `apps/tauri/sd-tauri-core/src/lib.rs`,
  `default_data_dir`), and nothing reads the old path. An alpha.2 user who
  installs beta.1 gets a new, empty data directory, and the alpha.2 data stays
  where it was. The migration above runs only when beta.1 is pointed at the
  old directory with `--data-dir`. The notes tell alpha.2 users to start a new
  library and add their folders again.

### Source stores (new since alpha.2)

- File state lives in one SQLite store per source: `sources/<id>/data.db`
  plus `sources/<id>/sidecars.db` for thumbnails (`docs/core/library.mdx`).
- A store's shape is versioned in `PRAGMA user_version`; this build writes
  version 2 (`crates/store/src/migrate.rs`). Version 1 rebuilt `content` so a
  row's integrity hash belongs to every record on it (#3112); version 2 adds
  `content.kind_name` and a lowercase extension index (#3135). Only the
  store's owner migrates, one transaction per step. A build refuses a store
  newer than it knows ("store schema version N is newer than this build's").
  The migration is covered by `crates/store/tests/migrate.rs`.
- A replica of another device's source is a copy of the owner's store and
  keeps the owner's version until the owner upgrades.

### Arena snapshots

- Snapshot format version 4 writes only live nodes
  (`core/src/ops/indexing/snapshot.rs`, #3138). A missing, quarantined or
  older-format snapshot is rebuilt from the source store in the background
  instead of rewalking the drive (#3139). An unreadable snapshot is moved to
  `<name>.corrupt-<timestamp>`, not deleted.

### Other files

- `library.json` is written to a temporary file and renamed over the old
  one (#3134).
- Backups write `manifest.json` (below).

### Rollback

- Binary rollback to alpha.2 is not possible: `drop_entries_world` has no
  down migration. Rollback means restoring the pre-upgrade backup.
- No earlier release reads a beta.1 library, so the notes name no
  rollback-compatible version and point to the backup instead. Nightly builds
  since #3135 (store version 2) are the earliest builds that read a beta.1
  store without refusing it; they are not releases.

## Known limits

- Inference is not available. The core has no inference provider, so
  extension calls to `ai.infer` answer `not_available`: no scene
  classification, face detection or language-model place and moment names
  (PROJECT_STATUS, Extension host functions phases 1 and 2).
- Extensions: the WASM extension host is behind the `wasm` cargo feature,
  which the release builds do not enable (`core/Cargo.toml`; the desktop
  build runs `cargo build --release --bin sd-daemon` with default features).
  Decided (Jamie, 2026-11-13): the `wasm` feature stays off in nightly and
  release builds until after beta, so beta.1 ships no extension runtime.
- Live-hardware rows still pending (CI covers the automatable half of each):
  - remote job watch and remote log stream (FDA P4: live only);
  - merge-preview time on a large source and the consuming merge of a real
    drive (action previews V6 and the live clauses);
  - pairing across networks over public relays and pkarr discovery
    (`docs/core/releases.mdx`, Candidate validation step 5);
  - replica replacement with local assertions (R8 row 26, feature not
    built);
  - a pin's path does not follow a drive that remounts at another path (FDA P3).
  - Proved live on a Mac since 2026-11-13: offline-then-online reconcile and
    `/Users` versus `/System/Volumes/Data` spellings (APFS firmlinks, R8 row
    9), after #3209 and #3212. The desktop app on a real library is still
    pending.
  - Several desktop UI fixes are committed with live proof pending:
    virtualized thumbnail remounts, Quick Preview originals, host media tools
    (PROJECT_STATUS register).
- Windows: everything above the unit layer has not run (gap doc): integration
  suites, FDA and R8 matrices, locked volumes, trash restore (two FDA rows
  ignored on Windows), store paths shown with mixed separators, no update
  timer (`sd update install-timer` is launchd and systemd only), unsigned
  binaries (SmartScreen warns on a browser download), no desktop app.
- Locked ZFS sources stay browsable from their index, labelled detached
  (Jamie's decision pending, PROJECT_STATUS Locked volumes).
- A library other devices are members of is only restored over itself with
  `--force`, and sync setup must be run again afterwards
  (`docs/core/library.mdx`).

## Backup steps (before upgrading)

From `docs/core/library.mdx`, Backup and restore (#3115). Run with the daemon
running:

~~~bash
sd library backup --to ~/backups/spacedrive-before-beta1
sd library backup verify --from ~/backups/spacedrive-before-beta1
~~~

- `--to <path>.tar.zst` writes one archive instead of a directory.
- A backup holds `library.json`, `library.db`, `sync.db`, every source store
  this device holds for the library (`data.db`, `sidecars.db`) and
  `manifest.json` with a blake3 hash of every file. It holds no original file
  bytes. `--no-sidecars` leaves thumbnails out; `--include-replicas` adds
  copies of other devices' sources.
- Restore: `sd library restore --from <dir> --as-new` (new library, overwrites
  nothing) or `--replace` (moves the current state to `restore-trash/`). A
  restore refuses a backup whose library applied a migration the running build
  does not know, so a beta.1 backup does not restore into an older build.
- Backup applies to 2.0 libraries on a build that has the command (nightly
  since 2026-10-06). An alpha.2 install has no `library backup` and needs
  none: beta.1 uses a different data directory and leaves the alpha.2 one
  untouched (see library.db above). Data directories: alpha.2 used
  `~/Library/Application Support/spacedrive` (macOS) and
  `~/.local/share/spacedrive` (Linux); nightly and beta.1 use `~/.spacedrive`.

## Upgrade steps

### From nightly (CLI and daemon)

A nightly built before the version bump reports `2.0.0-alpha.2 (<sha>)`, and
the stable channel sees beta.1 as newer. A nightly built after the bump
reports `2.0.0-beta.1 (<sha>)`; the stable channel treats beta.1 as the same
version and reports it is up to date, since that nightly already runs beta.1
or newer code.

~~~bash
sd library backup --to ~/backups/spacedrive-before-beta1
sd config set update.channel stable
sd update
sd --version
~~~

- `sd update` stops the daemon, swaps `sd` and `sd-daemon` through a
  `.bak` copy after checking the `.sha256`, and starts the daemon again
  (`docs/core/releases.mdx`, Opting in). Libraries migrate on first open.
- Decided (Jamie, 2026-11-13): beta.1 is published as Latest, so this path
  works. The stable channel reads
  `/repos/spacedriveapp/spacedrive/releases/latest`
  (`apps/cli/src/domains/update/mod.rs`, `fetch_release(.., "latest")`).
  GitHub's latest-release endpoint never returns a prerelease or a draft, so
  `sd update` finds beta.1 only if it is published as a full release marked
  Latest, replacing `0.4.3`. If beta.1 ships as a prerelease, nightly users
  stay on nightly (or download the binaries by hand) and the notes must say
  so. The desktop updater endpoint (`releases/latest/download/latest.json`
  in `tauri.conf.json`) has the same property.
- The desktop updater is on (#3210). The release uploads the macOS
  `.app.tar.gz` and the Linux `.deb` updater bundles with their `.sig` files
  and a `latest.json` listing `darwin-aarch64`, `darwin-x86_64`,
  `linux-x86_64` and `linux-aarch64`. The `Updater manifest` job verifies every
  signature with minisign against `plugins.updater.pubkey` before it writes
  the manifest, and fails on a tag that does not match `tauri.conf.json`.
  The trusted key is now `FA1DB2E94E74028C`, the key `TAURI_PRIVATE_KEY`
  holds. alpha.2 trusted `053A7DF33374C68B`, so an alpha.2 desktop app that
  reads beta.1's `latest.json` rejects the signature: alpha.2 desktop users
  install beta.1 by hand. No in-app update has run yet; the first one is
  beta.1 to a later release.
- Staying on nightly after beta.1 is valid: the nightly channel keeps
  following `main`.
- Update timers installed with `sd update install-timer` keep running and
  follow whichever channel is configured.

### From alpha.2 (desktop)

- Install beta.1 by hand (the alpha.2 updater rejects the new key), create a
  library, and add folders and drives again with Add to Library. beta.1 does
  not read the alpha.2 data directory, so nothing is migrated and the alpha.2
  data stays in place. Tags and collections from alpha.2 are not carried over.

## User-visible changes merged since October 4

From `git log --since=2026-10-04 main` (50 commits). CI, test and doc-only
commits are left out.

Library and sources
- Add to Library: one setup modal for folders and drives, with defaults and
  overrides, store placement, volume registration and portable identity;
  removing a source keeps its catalog (#3121, #3126).
- Keep an offline copy of a catalog that lives on the source's own drive
  (#3142).
- Library backup, verify and restore with manifest verification (#3115).
- Capture time and place in listings and search, read from EXIF (#3146,
  #3151).
- Library config writes are atomic (#3134); library migrations run in one
  transaction (#3147).

Drives and volumes
- Vanished volumes go offline; attached means mounted, so an unmounted drive's
  empty mount directory is never walked (#3119).
- ZFS dataset state (mounted, unmounted, locked) is detected and the index
  follows volume events (#3140); the state shows in the app, the CLI and on
  paired devices (#3141).
- A drive's index is rebuilt from its source stores when the snapshot is
  missing or quarantined, without a rewalk (#3139); arena compaction bounds
  memory under churn (#3138).

Devices and sync
- Replica fetch: resumable transfers, bandwidth cap, pause and progress
  (#3109).
- Library join fixes: backfill direction, catch-up loop, reverse device
  registration, member totals (#3110); changes buffered during an incremental
  catch-up are applied (#3160).
- Paired peers reconnect after a restart (#3106); pull transfer checksum, Linux
  watcher move-out and device name collision fixes (#3105).

File operations
- Linux trash restore, rename over an existing file (#3117); acceptance matrix
  for file operations runs in CI (#3116).

CLI, daemon and integrations
- `sd mcp`: an MCP server over the operation registry (#3131).
- Nightly channel with commit-aware `sd update` and an hourly update timer
  (#3107); the nightly release is edited in place, so watchers are not
  notified on every push (#3137).
- Windows: daemon and CLI build and pass unit tests; Windows nightlies
  published; `sd update` works on Windows (#3148).
- The daemon raises its open-file limit at start (#3153).
- Housekeeping: removed the `libraries.export` stub; restore-trash is pruned
  (#3143); CLI and daemon fixes from live Mac runs (#3111).

Extensions (not in release builds unless the `wasm` feature is turned on)
- The daemon loads WASM extensions and runs their jobs with checkpoint and
  resume (#3122); host functions for records, sidecars, models, tags, custom
  fields and job dispatch (#3124, #3144); extension file kinds and a photos
  viewer (#3135, #3136).

Fixes without a feature
- The library watcher no longer reopens a library closed right after creation
  (#3145, #3161); macOS build fix (#3154).

## User-visible changes merged November 6 to 13

From `git log 26b57e1..3e41e4a` (#3203 to #3213). #3205 (two test flakes)
and #3156 (a disabled CI workflow) change nothing a user sees.

- Startup reconcile and the watcher key non-UTF-8 names the walk's way, so
  such a name no longer breaks `file list` or search (#3203).
- A source is reconciled with its disk when its volume returns, including a
  drive that was detached when the daemon started (#3204).
- `sources.track` and `volumes.track` fail with "<root> is tracked, but its
  first walk could not start: <cause>" when the walk cannot start, for
  example on a full data disk; the store is marked walk-unfinished and the
  heal finishes the walk once the cause clears (#3206, #3209).
- macOS mount points are judged by `statfs` `f_mntonname`. On macOS 26.1 the
  device comparison judged the Data volume not mounted and refused every walk
  under `/Users` on nightly `11523b2` (#3209).
- Case-only renames on case-insensitive volumes (APFS, ext4 casefold) leave
  one record, live and offline. A chain of renames applied in one batch keeps
  its record on every Linux filesystem. `sd file info` resolves `/Users` and
  `/System/Volumes/Data` spellings and relative paths, and exits 1 when
  nothing is found (#3212).
- `sd file list` types rows by `kind`, so folders no longer print as files.
  `sd start` and `sd restart` wait for the old daemon's port to close, write
  the new daemon's stderr to `logs/daemon-stderr.log`, and report success
  only after a ping answers (#3211).
- Release: Linux and macOS only, draft with `make_latest: true`, a signing
  preflight, and a macOS notarization check (#3208); desktop updater feed
  (#3210); every product package at `2.0.0-beta.1` (#3213). The signed
  macOS path has not run with real secrets yet; the first tag push is its
  first run.

## Security-relevant changes

- The daemon's RPC listens on loopback TCP (`127.0.0.1:6969`) on every
  platform (Windows gap doc).
- `ci.yml` runs a gitleaks secrets scan and a license and notices check on
  every PR (#3118).
- Extensions run under a manifest permission check and are off by default
  (`wasm` feature).
- A release fails before building when a signing secret is missing, and a
  macOS job fails unless the app has a Developer ID signature and a stapled
  notarization ticket (#3208).
- Desktop update signatures are verified against the shipped public key
  before `latest.json` is written (#3210).

## Release workflow dry run

Run [41954878839](https://github.com/spacedriveapp/spacedrive/actions/runs/41954878839) at `818edfa`, dispatched with `dry_run: true` on the branch that adds the
input. It builds every artifact above except the signed macOS and Windows
desktop bundles, creates no tag or release, uploads only workflow artifacts,
and its token is read-only.

Green end to end in 46 minutes of wall time; `Create Release` was skipped.
No tag or release was created (`gh release list` and `git ls-remote --tags`
unchanged).

| Job | Runner | Time | Artifacts |
|---|---|---|---|
| CLI - linux-x86_64 | Blacksmith 4 vCPU, Ubuntu 22.04 | 8m41s | `sd-linux-x86_64` (42.8 MB), `sd-daemon-linux-x86_64` (50.9 MB), `.sha256` each |
| CLI - linux-aarch64 | Blacksmith 4 vCPU ARM, Ubuntu 22.04 | 24m01s | `sd-linux-aarch64`, `sd-daemon-linux-aarch64`, `.sha256` each |
| CLI - windows-x86_64 | GitHub `windows-latest` | 17m46s | `sd-windows-x86_64.exe`, `sd-daemon-windows-x86_64.exe`, `.sha256` each |
| CLI - macos-aarch64 | GitHub `macos-15` | 41m45s | `sd-macos-aarch64`, `sd-daemon-macos-aarch64`, `.sha256` each |
| CLI - macos-x86_64 | GitHub `macos-15-intel` | 44m28s | `sd-macos-x86_64`, `sd-daemon-macos-x86_64`, `.sha256` each |
| Server - linux-x86_64 | Blacksmith 4 vCPU, Ubuntu 22.04 | 18m48s | `sd-server-linux-x86_64.tar.gz` (`sd-server`, `sd-cli`, `.sha256` each) |
| Server - linux-aarch64 | Blacksmith 4 vCPU ARM, Ubuntu 22.04 | 45m56s | `sd-server-linux-aarch64.tar.gz` |
| Desktop - Main x86_64-unknown-linux-gnu | Blacksmith 16 vCPU, Ubuntu 24.04 | 19m30s | `Spacedrive-linux-x86_64.deb` (115 MB), `Spacedrive-frontend-linux-x86_64.tar.xz` |
| Desktop - Main aarch64-unknown-linux-gnu | Blacksmith 8 vCPU ARM, Ubuntu 24.04 | 44m35s | `Spacedrive-linux-aarch64.deb` (102 MB), `Spacedrive-frontend-linux-aarch64.tar.xz` |
| Create Release | | skipped | none |

Checked by download: `sd-linux-x86_64 --version` prints
`spacedrive 2.0.0-alpha.2 (818edfa0...)`; the x86-64 `.deb` is package
`spacedrive` 2.0.0-alpha.2, amd64, with `/usr/bin/Spacedrive`,
`/usr/bin/sd-daemon` and the codec bundle under `/usr/lib/spacedrive/`. Every
CLI job runs `sd-<platform> --version` on its runner before uploading.
