# Locked and unmounted volumes acceptance

The phases of [the locked volumes plan](../../plans/2026-09-28-locked-volumes.md)
each name an exit proof. This file maps the ones that run on Linux to the
test that proves them and states whether that test passes on the CI runner
(Blacksmith Ubuntu 24.04, `core_tests.yml`). L3 and L4 need a ZFS pool and
are not here yet.

How the suite runs:

- `core/tests/locked_volumes_acceptance_test.rs` runs in the `acceptance`
  job of `core_tests.yml` through `cargo xtask test-core --acceptance`. Each
  test builds a loop-backed ext4 image with the shared test volume helper,
  tracks it as a whole-volume source, and unmounts it with the mount point
  left in place, which is the shape of a locked ZFS dataset or an unplugged
  drive. The helper skips with a reason where there is no passwordless sudo or
  no loop device; the Blacksmith runner has both.
- The colocated tests run in the `--lib` suite of
  `cargo xtask test-core --unit`.

Status values follow `source-runtime.md`.

## Matrix

| # | Phase | Scenario | Test | Status |
|---|---|---|---|---|
| 1 | L1 | A volume detection cannot see at startup comes up detached | `core/tests/locked_volumes_acceptance_test.rs` `a_source_whose_volume_is_gone_at_startup_comes_up_detached` (row corrected to offline, `sources.list` shows the source offline, map restored read-only, no watch armed, no walk dispatched, reattaches when the volume returns); `core/src/ops/indexing/volume_index.rs` `a_volume_detection_cannot_see_comes_up_detached_whatever_its_row_says`, `without_detection_the_stored_flag_decides_attachment` | passing |
| 2 | L1 | The monitor marks a tracked volume offline when detection stops returning it | `core/tests/locked_volumes_acceptance_test.rs` `the_monitor_marks_a_vanished_volume_offline` | passing |
| 3 | L2 | An empty mount point is never walked, hashed or thumbnailed as a source | `core/tests/locked_volumes_acceptance_test.rs` `an_empty_mount_point_is_reported_unmounted_not_walked` (stored state still says mounted; the mount point check refuses, a forced heal dispatches nothing and sweeps nothing, `sources.track` on the mount point refuses, the listing reports the source unmounted with its counts intact); `core/src/volume/utils.rs` `a_mount_point_differs_from_its_parent_by_device` | passing |
| 4 | L1 | A daemon with two libraries lists every library's sources before and after a restart, whichever order the data directory loads them in | `core/tests/multi_library_acceptance_test.rs` `two_libraries_list_their_own_sources_across_a_restart` (`sources.list` and the volume index answer per library; closing one library leaves the other's sources); `core/src/ops/indexing/volume_index.rs` `two_libraries_each_list_their_own_sources_in_either_load_order` | passing |

## Decisions carried

A locked or unmounted source stays browsable from its snapshot and store,
labeled detached. The plan proposes this and leaves it to James; the
alternative, hiding the index until the volume returns, is not built.
