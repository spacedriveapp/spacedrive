# Release candidate validation

This file records a run of the nine candidate validation steps in
[`releases.mdx`](../releases.mdx#candidate-validation) against a published
nightly build. Run it again for each candidate. Each row says what was tested,
how, and the result. A step that needs hardware or signing keys this run did
not have is listed as remaining.

## Candidate

| Field | Value |
|---|---|
| Build | Nightly `26b57e1be3332f6781d1d6e4d29a10a7599e990a` (`nightly.sha`), published 2026-10-31 |
| Binaries | `sd-linux-x86_64`, `sd-daemon-linux-x86_64`, `sd-windows-x86_64.exe`, `sd-daemon-windows-x86_64.exe`, each checked against its `.sha256` |
| `sd --version` | `spacedrive 2.0.0-alpha.2 (26b57e1be333...)` |
| Run date | 2026-11-05 |
| Machines | Linux x86-64 cloud machines (4 vCPU, 16 GB RAM, no swap), fresh Docker containers per run; GitHub `windows-latest` and `ubuntu-latest` runners |

Every run used the published binaries, not a local build, except where a row
names a fix branch.

## Summary

| Step | Result |
|---|---|
| 1. Clean install | Linux passes on Ubuntu 24.04, Ubuntu 22.04 and Debian 12. Windows fails on `26b57e1` (`file list` returns nothing under a tracked folder) and passes on nightly `fc630d1`, which includes the fix (#3169). |
| 2. Upgrade path | Passes from three earlier nightlies. From v2.0.0-alpha.2, the library opens, but its locations, files, tags and collections are dropped. See the table in step 2. |
| 3. Large source | Passes at 1,000,000 files. Five thumbnail, statistics and logging bugs found and fixed (#3173, #3174, #3175, #3182, #3183). |
| 4. Interrupts | Fails. A killed copy left a truncated file (fixed, #3170). A killed or disk-full first walk is never healed (fixed in #3172, `68dc227`, and on Linux root and home volumes since). A full disk hangs the indexer. Files changed while the daemon is stopped are not picked up. |
| 5. Pairing | Covered by CI on every pull request. One real cross-network pair remains. |
| 6. Remote operations | Remaining. Needs two paired devices. |
| 7. Backup and restore | Passes: backup, verify, `--replace` (also under SIGKILL), and `--as-new` into a fresh data directory. |
| 8. Update signatures and rollback | Remaining. Needs signed release artifacts. |
| 9. Release notes and checksums | Remaining. Needs a tagged release. |

## 1. Clean install

### Linux x86-64

The README's "Install a nightly" steps ran in fresh `ubuntu:24.04`,
`ubuntu:22.04` and `debian:12` containers with only `curl` and CA
certificates added. The script downloads both binaries and their `.sha256`
files from the release download URLs, verifies them, installs them as
`~/.local/bin/sd` and `sd-daemon`, and then:

1. starts the daemon with `--no-default-sources`,
2. creates a library and tracks a 210-file tree,
3. checks `sources list`, `library info`, `file list` and `search files`,
4. creates a tag, applies it, and reads it back,
5. stops the daemon, confirms the process exited, restarts it, and checks
   the counts and the tag again.

All three images pass. On `debian:12` the first run exited 101 at `sd status | head`, because the CLI panics on a closed pipe (see findings). With the pipes replaced by `awk`, every check passes. The binaries link only `libc`, `libm` and
`libgcc_s`, so no other runtime packages are needed.

The nightly smoke workflow in #3166 runs the same checks on
`ubuntu-latest`. It passes in 16 s: the daemon is ready in 2 to 3 s, the walk
of 200 files takes about 1 s, and `file list` summed over every directory
finds 200 files and 24 directories.

### Windows x86-64

#3166 adds `.github/workflows/nightly-smoke.yml`. It runs the published
`sd-windows-x86_64.exe` and `sd-daemon-windows-x86_64.exe` on
`windows-latest` through the same smoke. The daemon starts in 5 s, and
`sources list` and `library info` match (224 records, 200 files). Then
`file list` finds 0 files.

The cause is the `\\?\` verbatim prefix. `sd sources track` sends the
path as `canonicalize()` returns it, `\\?\D:\...`.
`VolumeManager::locate_path` compared that path with the drive's mount
point without stripping the prefix, so the source registered under the
verbatim spelling and with no volume. `file list` sends the plain spelling,
which matches no source. Tracking the plain path through `sd op
sources.track` made every check pass, restart included. #3169 strips the
prefix in `locate_path` and in the CLI (merged as `b6cd7fb`).

The smoke passes on nightly `fc630d1`, the first published build with #3169
(run 41970748320). Windows takes 40 s: the daemon is ready in 5 s, the source
has a volume, `file list` finds 200 files and 24 directories, and the counts
and the tag hold after a restart. Linux takes 28 s. `sources list` on Windows
prints the root with mixed separators (`D:\a/_temp/smoke-src`), while
`sources track` returns `D:\a\_temp\smoke-src`. Listing and counts work
either way.

### Findings

- `sd daemon install` on a host without systemd writes the unit file and
  then fails with `No such file or directory (os error 2)`. The error does
  not say that `systemctl` is missing.
- `sd status | head` and `sd search files ... | head` exit 101 with a
  panic (`failed printing to stdout: Broken pipe`) when `head` closes the
  pipe first. Under `set -o pipefail` that fails the script. #3188 restores the default `SIGPIPE` action, so the CLI ends quietly
  (exit 141, like other Unix tools).
- The README's "Install a nightly" lists macOS and Linux only. The nightly
  release also ships Windows binaries.
- `--version` reports `2.0.0-alpha.2` for the beta candidate. Bump the
  package version before tagging the beta.
- `--data-dir X --instance N` nested the daemon's state at
  `X/instances/N/instances/N` while the CLI read `X/instances/N`. The
  CLI then sent an empty device slug, and `file list` returned nothing.
  Fixed in #3171 (`f07010c`).
- The Nightly workflow cancelled its running build on every push to
  `main`. A build takes about 45 minutes, so on a day with frequent merges
  nothing was published: eleven runs on 2026-11-05 were all cancelled, and
  `nightly` stayed on `26b57e1`. #3186 queues builds instead.
- `start` creates a "My Library" before any `library create`, so a
  scripted install ends with two libraries. With two libraries, `job list
  --format json` prints one document per library, and `index start` needs
  `--library`.

## 2. Upgrade path

Two origins were tested: earlier nightlies, and a library created by the
v2.0.0-alpha.2 release.

### From earlier nightlies

The rolling `nightly` tag keeps only the latest binaries. Earlier nightly
builds were downloaded from the artifacts of the `nightly.yml` runs, which
are kept for 90 days:

| Origin build | Why it matters |
|---|---|
| `df385a9` (2026-10-04) | Oldest nightly with artifacts left |
| `5bc474e` (2026-10-05) | Last before `c81e3d6`, store schema version 0 |
| `26b67ff` (2026-10-08) | Last before `f9d89c1`, snapshot format v3 |

For each origin, a fresh container:

1. installs the old binaries,
2. creates a library with two sources (2,061 files in 64 directories,
   including identical files),
3. applies three tags to three files,
4. records `library info`, `sources list`, the tag list, `file info` for
   the tagged files, a search, and a directory listing,
5. stops the daemon,
6. upgrades with the old CLI's own `sd update --yes` on the nightly
   channel,
7. starts the daemon and records the same outputs,
8. writes a new file into a source to check that the watcher works after
   the upgrade.

Results, the same for all three origins:

- The old updater downloads and verifies both binaries and installs
  `26b57e1`. Its message names the release's frozen `target_commitish`
  (`0e765514630a`), not the installed commit. The docs describe this.
- File counts, sizes, source counts, the tags, file ids, tag assignments,
  search results and listings are identical before and after.
- Each source's snapshot is moved aside (`snapshot undecodable ... moved
  aside for diagnosis`). The map is then rebuilt from the store (`no usable
  snapshot; map rebuilt from the source store ... loaded=1920 took=62ms`),
  and the watcher is armed (`Restored 2 registered source arena(s) at
  startup`). No walk runs.
- The store migration runs on the first write after the upgrade. A store
  from `5bc474e` went from `PRAGMA user_version` 0 to 2, with
  `candidate_uuid` set on every content row (1,802 of 1,802 and 201 of 201).
  Before the first write the store stays at version 0 and readers read the
  old shape, as `crates/store/src/migrate.rs` describes.
- A file created after the upgrade appears in the listing within 8 s.


### Findings

- The store migration never appeared in `daemon.log`: the default filter
  dropped the `sd_store` target. Fixed in #3168 (`74da11c`), which also
  makes config migrations finish in one pass.
- A v3 snapshot is reported as "undecodable" (`Found a varint that didn't
  terminate`), not as a version mismatch. The outcome is right, but the
  message reads like corruption.
- `library info` shows the wrong tag count both before and after the
  upgrade (0, then 1, with 3 tags). See step 3.

### From v2.0.0-alpha.2

The alpha.2 release ships the desktop app and the daemon but no CLI. The
`sd-daemon` from `Spacedrive-linux-x86_64.deb` (tag `v2.0.0-alpha.2`,
`f1843bf`) ran in a fresh `ubuntu:24.04` container. It was driven through
the nightly CLI's `sd op` passthrough, with inputs in alpha.2's own shapes.
In that container:

1. alpha.2 created the library `AlphaLib`,
2. added two locations in `Content` mode (40 files and 22 files plus a
   subfolder), and waited for both indexer jobs to finish (61 content
   identities),
3. created the tag `Vacation` and applied it to two files, and created the
   tag `Work` in the namespace `Projects`,
4. created the Space `Alpha Space`,
5. inserted a collection `Best of 2025` with 5 files directly into
   `library.db`, because alpha.2 registers no collection operation,
6. stopped, and the nightly `sd-daemon` started on the same data
   directory.

alpha.2 refuses a location on a volume it cannot detect (`No volume found
for location path`), so the test data sat on its own mount. Library
creation in alpha.2 sometimes fails with `UNIQUE constraint failed:
seaql_migrations.version` because it races its own auto-open. The test
retried until creation succeeded.

On first open the nightly runs 9 library migrations, from
`m20260125_000001_unique_user_metadata_tag` to
`m20260918_000001_drop_entries_world`. Each table was compared before and
after:

| Item | After the upgrade |
|---|---|
| Library id, name, folder | Kept. It opens with no error and is listed by `library list`. |
| Device identity (`device.json`, `device_key`) | Kept unchanged. |
| Library config (`library.json`) | Kept, with new defaults added (`adding`: `identify_content`, `keep_offline_copy`). |
| Volumes | Kept (1 row). |
| User-created Spaces | Kept, with name and color (`Alpha Space`). |
| Default Space and sidebar | Replaced. alpha.2's `All Devices` Space, its 4 items (Overview, Recents, Favorites, FileKinds) and its 4 groups (Devices, Locations, Volumes, Tags) give way to the current `Default` Space with the items Overview, Recents, a Screenshots collection, Sources and Redundancy, and no groups. |
| Locations | Dropped (2 rows). `sources list` shows no sources. |
| Indexed files (`entries`, `directory_paths`, `entry_closure`) | Dropped (66 entries). `file list` on a former location returns nothing. |
| Content identities and kinds | Dropped (61 rows). |
| Tags and tag applications | Dropped (2 tags, 2 applications). `tag search` finds nothing. |
| Collections | Dropped (1 collection, 5 members). |
| Search index | Dropped. |
| Sidecars | Dropped. alpha.2 generated none in this run. |

`library info` right after the upgrade still shows alpha.2's cached
totals (62 files, 61 unique contents) while the library holds no sources.
It corrects itself once statistics are recalculated.

Tracking the two old folders as sources (`sd sources track`) indexes them
again: 41 and 23 records, and `search files img1` finds the file. The tags
and collection do not come back. The data directory also gains a second,
empty library, `My Library` (see step 1).

What the release notes need to say: a v2.0.0-alpha.2 library opens in the
beta and keeps its name, id, device identity and custom Spaces. Its
locations, indexed files, tags, tag applications and collections are
deleted. The sidebar resets to the beta defaults. Users must track their
folders again as sources. To keep tags, they must write them down before
upgrading.

## 3. Large source

The tree was generated on the machine: 1,000,000 files in 2,110
directories, 9.22 GB. Leaf directories hold 500 files each. File sizes are
log-normal (median 2 KiB). The tree includes 4,000 real JPEG and PNG
images and 2,000 groups of identical files covering 21,805 files, which
makes 980,195 distinct contents.

| Measure | Value |
|---|---|
| Walk | 200 s (discovery 32 s, then the arena and the store) |
| Content identification | 655 s, 1,000,000 files, 0 unreadable |
| Thumbnails | 202 s; 4,000 generated |
| Peak daemon RSS | 1.89 GB for walk and hash, 2.41 GB with thumbnails, 1.26 GB after a restart |
| Source store | 623 MB (`data.db` 518 MB, `sidecars.db` 105 MB) |
| Data directory | 3.10 GB, mostly the thumbnail cache |
| Restart | stop 1.5 s; daemon responsive 2.4 s after start; snapshot of 1,002,117 entries loaded in 4.7 s |
| Search | 0.04 to 0.10 s per query; one query with no hits took 1.67 s |

Checks:

- `library info` shows 1,000,000 files, and `sources list` shows
  1,002,110 records (files plus directories). Both match the disk.
- The store holds 980,195 content rows, as generated. Each of the 2,000
  duplicate groups is found, and `files.alternate_instances` returns all
  copies of a file.
- An exact file name returns one hit. A name shared by 8 copies returns all
  8. `--file-type jpg` returns 2,039, the number of JPEG files on disk.
  `--content-type image` returns 4,000.
- After a restart, the counts and total bytes are unchanged. Searches return
  the same results, and the content identification pass finds nothing left
  to do.

### Findings

- The thumbnail job submits every file in scope, not only images (fixed in #3174). It reports
  "4000 generated, 0 skipped, 996000 failed", and most of its 202 s goes to
  files that cannot have a thumbnail. Fixed in #3174 (`e15fe24`).
- `library info` reports `unique_content_count` 0 after hashing. The
  number is read from the registry row, and that row is written by the
  snapshot save at the end of the walk, before hashing starts. Nothing
  refreshes it later (fixed in #3183). Fixed in #3183 (`69108a0`).
- `library info` reports `tag_count` 0 after a tag is created and applied
  (Linux and Windows, and after a restart). Fixed in #3182. The recount read only the tag
  staging table. Fixed in #3182 (`d3926fb`).
- `total_size` includes the sizes of directories (25 MB over the file bytes
  here), although "Total files" leaves directories out. Fixed in #3173. Fixed in #3173
  (`25fe4d4`).
- `sd logs show` reads a fixed macOS path (`~/Library/Application
  Support/spacedrive`) on every platform. Fixed in #3175. Fixed in #3175 (`990b85f`).
- `file info` returns `content_identity: null` for hashed files.
- The thumbnail cache stores raw RGBA tiles: about 435 KB on disk per
  thumbnail.

## 4. Interrupts

Every case ran in a fresh container against the published binaries. A
script compares the source store with the disk: it rebuilds every record's
path from `record.parent_uuid` and `record.title`, compares the result
with `find`, and counts missing and extra paths, duplicate paths, orphaned
records and files without content.

The test tree has 240,000 files (240,630 entries) of 0 to 20 KB.

| Case | Result |
|---|---|
| SIGKILL during content identification, after a completed walk | Pass. Identification resumes, and the store matches the disk exactly: 240,630 records, 0 missing, 0 extra, 0 orphans, 0 unhashed. |
| SIGKILL during the first walk of a source | Fail. The indexer job is marked failed ("interrupted by restart; not resumable"), and the source stays at 0 records across restarts and `index start --defaults`. |
| SIGKILL during a copy of 12 × 150 MB files | Fail. One file is left truncated under its final name. The resumed job skips it as existing and reports completed, and a second `file copy` keeps it "by policy". |
| SIGKILL during `library restore --replace` (110 MB backup), at 0.3, 0.8 and 1.5 s | Pass. After each restart the library opens with 240,000 files, the tag on the test file is intact, and the store matches the disk. A clean restore afterwards succeeds. |
| Disk full under the first walk (data directory on a 400 MB loop device) | Fail. The store reports `database or disk is full` and the job stops at 23% but stays "running" with no CPU use for 30 minutes. It does not continue after space is freed. After a restart the source keeps 19,000 of 240,630 records for good. |
| Files changed while the daemon is stopped | Fail. 3,001 entries created while the daemon was stopped are missing from the store after two restarts and a startup pass. |

### Findings and status

- Killed copy leaves a truncated file. Local copies now write to a
  hidden, exclusively created `.sd-<uuid>.sdpart` sibling and rename it
  into place when complete. Fixed in #3170 (`a313420`). The receiving side
  of a remote transfer still writes in place.
- A first walk that never finished is never healed.
  `heal_uncovered_sources` skipped every source without a record count,
  and only a completed walk writes one. #3172 (`68dc227`) heals such a source. A debug
  build of #3172 healed a killed first walk on a loop-mounted volume, and
  the store then matched the disk exactly.
- The heal never ran on Linux root and home volumes.
  `map_attached_volumes` skipped volumes that are not user-visible, and
  `is_system_mount_point` hides `/` and `/home`. On a typical Linux
  install a home folder is on one of those, so no coverage heal reached it.
  Fixed: the heal now resolves registered sources directly, whatever their
  volume's visibility, and never walks the hidden volume itself. Visibility
  is a display rule, not an indexing rule. The same change covers sources
  under `/Users` on macOS. `hidden_volume_heal_acceptance_test` stops a
  first walk of 30,300 entries on a hidden volume and checks that the launch
  pass heals it; on the old code the store stays at 0 records.
- Disk full hangs the indexer. After the store's batch write fails, the
  salvage writes rows one by one. Five rows log `row failed to land`, and
  then the job blocks with no further log lines or CPU use. The nightly
  binary is stripped, so no stack was captured. Reproduce it with a debug
  build: put the data directory on a nearly full loop device, track a
  240,000-file tree, and attach a debugger to the stalled daemon.
- Changes made while the daemon is stopped are not reconciled. At
  startup the map is restored from the snapshot and the watcher is armed,
  but nothing compares the map with the disk.
  `docs/core/design/zero-onboarding-startup.md` plans a background mtime
  reconcile per source on warm start. It is not built. Until it is, a file
  created, changed or deleted while the daemon was not running is not seen
  until something walks that folder again.
- Writes before the watcher is armed are missed. A copy job that
  resumed 2 s after a restart wrote 9 files into a tracked folder. The
  folder's store held 4 of its 13 entries afterwards.
- The daemon logs `DEBUG: JobExecutor ...` lines at WARN level on every
  job start.

## 7. Backup and restore

The `--replace` path is covered under step 4. For `--as-new`, a library with
one source (501 files) and a tagged file was backed up from one data
directory, verified, and restored into a second, fresh data directory with
its own daemon. `library info` showed the same file count, size and source
count. `sources list` showed the same 502 records. The map was rebuilt from
the restored store 5 s after the restore (`no usable snapshot; map rebuilt
from the source store ... loaded=502`), after which `file info` returned the
file with its tag. A second `--as-new` with the same backup refused because
the library id already exists, as documented.

## 5. Pairing

Covered by CI on every pull request (`device_pairing_test` over mDNS,
`relay_only_pairing_test` through an in-process relay). Not repeated here.
One real device pair across networks remains, as `releases.mdx` says.

## Fixes from this run

| PR | Fix | Merge |
|---|---|---|
| #3168 | Log store migrations by default | `74da11c` |
| #3169 | Strip the Windows verbatim prefix before locating a volume | `b6cd7fb` |
| #3170 | Stage local copies and rename them into place | `a313420` |
| #3171 | Pass the base data directory with `--instance` | `f07010c` |
| #3172 | Heal a source whose first walk never finished | `68dc227` |
| #3173 | Leave directories out of `total_size` | `25fe4d4` |
| #3174 | Skip files no thumbnail producer can draw | `e15fe24` |
| #3175 | Read logs from the resolved data directory | `990b85f` |
| #3182 | Count tags from source stores | `d3926fb` |
| #3183 | Refresh source counts after content identification | `69108a0` |
| #3186 | Queue nightly builds instead of cancelling them | `79e38ea` |
| #3188 | End the CLI quietly when its output pipe closes | `b31c2ab` |
| #3166 | Nightly smoke workflow for Linux and Windows | `d3892f1` |

## Remaining

These steps need hardware, accounts or keys a cloud machine does not have:

- Step 1 on macOS (Apple Silicon): install from a browser download
  (quarantine attribute), `sd daemon install` under launchd, and the
  update timer.
- Step 2 on real prior-version state: copies of the Macs' libraries,
  including the 3-million-file library.
- Step 5: one device pair across networks.
- Step 6: remote queries, ranges, mounts and mutations between two paired
  devices.
- Step 8: update signatures and rollback, which need signed artifacts.
- Step 9: release notes and checksums, which need a tagged release.
