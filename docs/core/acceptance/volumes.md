# Locked and unmounted volumes acceptance

The phases of [the locked volumes plan](../../plans/2026-09-28-locked-volumes.md)
each name an exit proof. This file maps the ones that run on Linux to the
test that proves them and states whether that test passes on the CI runner
(Blacksmith Ubuntu 24.04, `core_tests.yml`).

How the suite runs:

- `core/tests/locked_volumes_acceptance_test.rs` runs in the `acceptance`
  job of `core_tests.yml` through `cargo xtask test-core --acceptance`. Each
  test builds a loop-backed ext4 image with the shared test volume helper,
  tracks it as a whole-volume source, and unmounts it with the mount point
  left in place, which is the shape of a locked ZFS dataset or an unplugged
  drive. The helper skips with a reason where there is no passwordless sudo or
  no loop device; the Blacksmith runner has both.
- `core/tests/zfs_locked_volumes_acceptance_test.rs` runs for real in the
  `zfs` job of `core_tests.yml` on GitHub's `ubuntu-24.04` image, whose kernel
  carries the zfs module; the Blacksmith kernel has none, so in the
  `acceptance` job the suite skips with that reason. It builds a file-backed
  pool under `/tmp` mounted under `/mnt` with one encrypted dataset keyed from
  a passphrase file, tracks the dataset, locks it (`zfs unmount` and
  `zfs unload-key`) and unlocks it (`zfs load-key` and `zfs mount`) under the
  running daemon. The suite skips with a reason where there is no passwordless
  sudo, no zfs userland or no zfs kernel module.
- The colocated tests run in the `--lib` suite of
  `cargo xtask test-core --unit`.

Status values follow `source-runtime.md`. `CI` marks a row whose test runs
for real only on the CI runner: the cloud machines the crew builds on have no
ZFS kernel module, so the ZFS suite skips there and its status is read from
the `zfs` job.

## Matrix

| # | Phase | Scenario | Test | Status |
|---|---|---|---|---|
| 1 | L1 | A volume detection cannot see at startup comes up detached | `core/tests/locked_volumes_acceptance_test.rs` `a_source_whose_volume_is_gone_at_startup_comes_up_detached` (row corrected to offline, `sources.list` shows the source offline, map restored read-only, no watch armed, no walk dispatched, reattaches when the volume returns); `core/src/ops/indexing/volume_index.rs` `a_volume_detection_cannot_see_comes_up_detached_whatever_its_row_says`, `without_detection_the_stored_flag_decides_attachment` | passing |
| 2 | L1 | The monitor marks a tracked volume offline when detection stops returning it | `core/tests/locked_volumes_acceptance_test.rs` `the_monitor_marks_a_vanished_volume_offline` | passing |
| 3 | L2 | An empty mount point is never walked, hashed or thumbnailed as a source | `core/tests/locked_volumes_acceptance_test.rs` `an_empty_mount_point_is_reported_unmounted_not_walked` (stored state still says mounted; the mount point check refuses, a forced heal dispatches nothing and sweeps nothing, `sources.track` on the mount point refuses, the listing reports the source unmounted with its counts intact); `core/src/volume/utils.rs` `a_mount_point_differs_from_its_parent_by_device` | passing |
| 4 | L1 | A daemon with two libraries lists every library's sources before and after a restart, whichever order the data directory loads them in | `core/tests/multi_library_acceptance_test.rs` `two_libraries_list_their_own_sources_across_a_restart` (`sources.list` and the volume index answer per library; closing one library leaves the other's sources); `core/src/ops/indexing/volume_index.rs` `two_libraries_each_list_their_own_sources_in_either_load_order` | passing |
| 5 | L3 | A dataset whose key is not loaded reads as locked, not unmounted or empty | `core/tests/zfs_locked_volumes_acceptance_test.rs` `a_locked_dataset_reads_as_locked_and_follows_its_key` (`volumes.list` keeps the tracked dataset with `locked` set and `is_mounted` clear, `sources.list` reports `volume_state` locked and the source detached with its counts intact, dispatch at the mount point refuses, a forced heal and an identity pass dispatch nothing, the listing serves from the snapshot; after `zfs load-key` and `zfs mount` plus one refresh both report mounted); `core/src/volume/fs/zfs.rs` `a_dataset_whose_key_is_not_loaded_is_locked_not_merely_unmounted`, `five_column_output_still_parses_as_mounted` | CI |
| 6 | L4 | A dataset locked and unlocked under the running daemon takes its source with it, watch included, and retries the identifications that failed while it was away | `core/tests/zfs_locked_volumes_acceptance_test.rs` `a_locked_dataset_reads_as_locked_and_follows_its_key` (the watch is dropped on lock and armed on the mounted dataset after unlock, a file created after the unlock reaches the map, a record left with a content error and no identity is identified) | CI |
| 7 | L4 | A volume unmounted and remounted under the running daemon follows through the volume manager's events alone | `core/tests/locked_volumes_acceptance_test.rs` `a_remounted_volume_reattaches_its_source_and_rearms_the_watch` (no reconciliation call; the source detaches and reattaches, the watch is dropped and re-armed, a file created after the remount reaches the map, a failed identification is retried); `core/src/ops/indexing/volume_index.rs` `a_volume_detection_cannot_see_comes_up_detached_whatever_its_row_says` (return announces the root, lock drops the watch) | passing |

## Decisions carried

A locked or unmounted source stays browsable from its snapshot and store,
labeled detached. The plan proposes this and leaves it to James; the
alternative, hiding the index until the volume returns, is not built.
