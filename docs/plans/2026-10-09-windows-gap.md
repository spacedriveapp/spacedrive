# Windows: distance from the beta gate

> Status: measured 2026-10-09 on GitHub-hosted `windows-latest` from
> `03efcad` plus the fixes in #3148; the `Core Tests (windows unit)` job in
> `core_tests.yml` is the standing measurement
> Captured: 2026-10-09
> Owns: what builds, what passes and what fails on Windows at `main`, the size
> of each gap, what the FDA and R8 acceptance matrices would need to run
> there, and the beta.1 recommendation
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/releases.mdx` (the cross-platform gate),
> `docs/core/acceptance/entries-drop-and-file-operations.md` (FDA),
> `docs/core/acceptance/source-runtime.md` (R8),
> `docs/core/acceptance/volumes.md` (locked volumes)

## Outcome

Windows had not been built since the rewrite. The first build on a real
Windows runner compiled `sd-core`, `sd-cli` and `sd-daemon` with no source
change, and the `sd-core` lib suite ran 605 of 613 tests green at `main`.
Three of the eight failures were Windows bugs in the product and five were
tests written in Unix shapes; all eight are fixed in #3148 and the second run
is green: 615 passed, 4 ignored (the two FDA trash rows and two Linux-only
tests). The CLI's own tests, including the MCP test that
spawns `sd-cli.exe` against an in-process core, also pass. Nightly now
publishes `sd-windows-x86_64.exe` and `sd-daemon-windows-x86_64.exe`, and
`sd update` installs them.

What has not run on Windows is everything above the unit layer: the
integration suites, the FDA and R8 acceptance matrices, and the locked
volumes suite. Those are the cross-platform gate's evidence, and the section
below sizes what each needs.

## Measurements

Runner: `windows-latest` (Windows Server 2025, 4 vCPU, 16 GB). Toolchain
installed by `setup-rust` from `rust-toolchain.toml` (1.97.1).

| Step | Cold | Notes |
|---|---|---|
| `cargo build -p sd-cli -p sd-core --bin sd-cli --bin sd-daemon` (debug, no debuginfo) | 32 min | 769 crates. No C dependency failed; `libsqlite3-sys` bundled, `openssl-sys` not compiled on this target (reqwest uses schannel) |
| `cargo test -p sd-core --lib` build | 9 min | on top of the build above |
| `cargo test -p sd-core --lib` run | 3 min | 613 tests |
| `cargo test -p sd-cli` | 7 min | 16 lib tests plus the MCP stdio test, which spawns `sd-cli.exe` against an in-process core |
| Whole job | 53 min cold | `continue-on-error`; cache saved on red runs too (`cache-on-failure`) so the next run warms |

A warm run has not been measured yet: the first run failed before
`rust-cache` saved, and the cache now saves on failure. Expect the
dependency half of the 32 minutes to drop; the workspace crates rebuild every
run regardless (`rust-cache` keys out workspace members), so the floor is the
`sd-core` compile on a 4 vCPU Windows host, likely 15 to 20 minutes. The
brief's 40 minute budget needs that warm measurement. Blacksmith has no
Windows runners; a self-hosted Windows machine would be the other lever.

## What passed at `main` (no change needed)

- Compiling. `cargo check -p sd-core -p sd-cli --lib --bins --tests` for
  `x86_64-pc-windows-gnu` from Linux with mingw is also clean, and is a
  nine minute way to catch cfg gaps without a Windows runner.
- 605 `sd-core` lib tests, including the file operation fixtures (copy,
  merge, delete, trash the OS way, rename, archive), the indexing arena,
  stores, search, sync, the volume index and the daemon bootstrap.
- Daemon and CLI talk over TCP loopback (`127.0.0.1:6969`), so nothing
  needed a Unix socket.

## What failed at `main` and the fixes (#3148)

| # | Test | Cause | Kind | Fix |
|---|---|---|---|---|
| 1 | `ops::files::acceptance::preview_rows_match_execution_for_copy_move_merge_and_delete` | A move inside one volume chose the streaming copy, then the delete of the source was gated on `VolumeManager::same_volume`, which answered "same volume" and skipped it, so the file ended in both places and the job reported success | product bug | The job checks whether the source still exists after the strategy ran and removes it then |
| 2 | `ops::indexing::writer::tests::a_walk_lands_as_one_event` | Hidden is a file attribute on Windows, read with `GetFileAttributesW`. The arena rebuilt `is_hidden` from the path on every read instead of keeping what the walk saw, so anything not on disk at that moment (a test fixture, a snapshot, a detached drive) read as not hidden, and every arena read was a syscall | product bug | `PackedMetadata` carries a hidden bit (size field drops to 59 bits); the arena records what the walk saw; the writer judges a change by the metadata it carries; snapshot restore recomputes the bit from names where names decide |
| 3 | `ops::search::arena_search::tests::the_store_backend_matches_the_arena_for_the_same_capture` | Same as 2: the arena returned `.secret.mov` from a hidden-excluded search | product bug | Same fix |
| 4 | `ops::indexing::volume_index::tests::a_mapped_drive_that_is_away_keeps_its_partition` | `volume::utils::is_mount_point` was `path.exists()` on Windows, so the empty directory an unplugged mapped drive leaves behind passed as mounted and a detached volume could be dispatched at it | product bug (the `get_inode`-style stub the brief named) | `GetVolumePathNameW` names the mount point of the volume holding a path; a directory that is its own volume root is a mount point |
| 5 | `ops::search::media::tests::a_replica_pages_its_media_in_path_order` | Hidden (as 2) plus a displayed path compared against a slash-written expectation | test | Normalise separators in the test helper |
| 6, 7 | `ops::paths::compare::tests::by_path_sorts_each_file_into_its_set`, `by_content_finds_bytes_wherever_they_sit` | Displayed paths compared against slash-written expectations | test | Same |
| 8 | `ops::sources::track::action::tests::an_absolute_root_is_accepted` | `/Volumes/Archive` is not absolute on Windows | test | A platform-absolute root |

Also fixed in the PR without a failing test: the CLI built the daemon's path
as `sd-daemon` everywhere (now `sd_client::daemon_binary_name()`, which
appends `.exe` on Windows), and `sd update` renamed the staged file over the
running binary, which Windows refuses for a loaded executable (now the
current binary moves to `.bak` first; `sd.exe.bak` stays until the next
update removes it, since a running executable cannot be deleted).

## Gap list: what has not run on Windows

Sizes are for one person with a Windows machine or the CI job as the loop
(each CI iteration is 50 minutes cold, so a machine is the faster loop).

| Gap | What it is | Size | Blocks |
|---|---|---|---|
| Store relative paths joined with `/` | Store rows keep `rel_path` with `/`. Twenty call sites do `root.join(&entry.relative_path)`, which on Windows yields `C:\root\a/b.txt`. `Path` compares by components so lookups work, but every displayed or serialised path carries mixed separators, and any string comparison on the result is wrong | half a day: one `join_rel` helper split on `/`, the twenty sites, a test | cosmetic in the UI, wrong for string-keyed sets |
| Trash restore (FDA F3, F5, F10) | `trash` crate's Windows item id is a shell parsing name, not a path, so the trash view cannot stat an item and restore goes through the crate blind. Two acceptance tests are `#[ignore]` on Windows for this | one day: record the recycle bin item through `IShellItem`/`SHGetKnownFolderPath` or the crate's `os_limited` API, then un-ignore | the FDA matrix |
| Integration suites (`xtask test-core --integration`) | 40 targets, never run on Windows. They assume `scripts/setup.sh` system packages (bun for the TypeScript bridge tests), a Desktop directory, user volumes, and two daemons pairing over loopback. On Linux 20 of them failed on a bare VM until SPAC-6 fixed environment assumptions | one to two days: a second Windows job on the same cache, triage per suite the way SPAC-6 did on Linux | the gate's "mount, watcher, permission" rows |
| FDA matrix on Windows | `entries_drop_acceptance_test.rs` and `core/src/ops/files/acceptance.rs` (the latter now runs green in the lib suite minus the two trash rows). The integration half needs the job above | included in the integration triage | cross-platform gate |
| R8 matrix on Windows | `source_runtime_acceptance_test.rs` and the two-process `source_replication_test.rs`; `crates/store/tests`. Rows 9 (APFS) and 26 are not automatable anywhere but macOS; the rest should run. Watcher rows (11) go through `crates/fs-watcher/src/platform/windows.rs` (ReadDirectoryChangesW), which has never been exercised in CI | one day after the integration job exists | cross-platform gate |
| `sd-fs-watcher --lib` on Windows | The unit job on Linux runs it; the Windows job does not yet. Add it once the first run shows what the Windows handler does with the shared tests | an hour to add, unknown to fix | watcher row |
| Locked volumes (`volumes.md` rows 5, 6) | Linux uses loop mounts and ZFS; Windows has `diskpart` VHDs (`test_volumes.rs` has a `create_vhd` helper that nothing calls). Mapped drives and BitLocker are the real cases | two days | gate's removable drive row |
| Update timer | `sd update install-timer` is launchd and systemd only; Windows returns a clear error. Task Scheduler (`schtasks`) is the equivalent | half a day | nightly users on Windows, not the gate |
| Signing | Nightly Windows binaries are unsigned; SmartScreen warns on a browser download. Release needs an Authenticode certificate, which nobody has started | Jamie's decision; days of paperwork | release, not nightly |
| Desktop (Tauri) on Windows | Not built in this cycle at all; `release.yml` has a Windows desktop row that has never run on this tree | unknown | desktop beta on Windows |
| Warm CI time | Unmeasured, see above | one run | keeping the job under 40 minutes |

Not gaps: filesystem metadata (`EntryMetadata` reads `created`, inode as the
NTFS file id, no `uid`/`gid`), hidden files, mount points, device identity
(`domain/device.rs` has Windows branches throughout), the volume manager's
`\\?\` prefix handling, path spelling through `locate_path`. These are what
the 605 green tests cover.

## Recommendation

Scope Windows out of beta.1 for the desktop and the acceptance claims, and
keep the CLI and daemon on the nightly channel for Windows as a preview.

The reasoning: the compile and unit layer is in better shape than the
roadmap feared (eight failures, three real, all fixed in a day), so this is
weeks of unbuilt work rather than months, but the gate asks for the FDA and
R8 matrices and the mount, watcher, permission and removable drive rows to
pass on Windows, and none of that has run once. The integration triage alone
took the Linux side two threads (SPAC-2, SPAC-6) with a fast machine in the
loop; on Windows the loop is a 50 minute hosted runner unless someone
provides a Windows machine. With the beta on November 1 and Windows first on
the cut list in `roadmap-to-beta.md`, the honest claim for beta.1 is what the
CI job proves: it builds, the unit layer passes, the binaries install and
update themselves.

Proposed release notes text:

> Spacedrive 2.0 beta.1 supports macOS (Apple silicon) and Linux (x86-64).
> Windows is not part of this beta. The daemon and CLI build and pass their
> unit tests on Windows, and nightly builds for Windows x86-64 are published
> for people who want to try them, but the file operation, source runtime and
> drive acceptance suites have not been run there, so we do not yet make the
> data safety claims this release makes for macOS and Linux. Windows support
> is planned for a later beta.

If Jamie wants Windows in beta.1 instead: the path is a Windows machine with
rustup (or a self-hosted runner), the integration job, then the trash and
relative-path fixes, then the matrices; three weeks of one thread's time
with the loop on a real machine, longer on the hosted runner. That does not
fit before November 1 alongside the other gates.

## Keeping the job honest

- `Core Tests (windows unit)` runs on every push to `main` and every PR and
  gates the merge since it was green twice on `main` (15d7df1, e4b4cb7). It
  takes 40 min cold and 21 min warm (build 6, lib tests 8, sd-cli 5).
- A failing Windows test is a bug or a Unix-shaped test. Fix the test's shape
  only when what it asserts is platform-neutral; `cfg(windows)`-ignore only
  with the reason in the attribute, the way the FDA trash rows are.
- Before a Windows runner, `cargo check --target x86_64-pc-windows-gnu` with
  mingw on Linux catches cfg gaps in nine minutes.
