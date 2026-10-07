# Source runtime acceptance matrix

The R8 table in
[the source runtime reliability plan](../../plans/2026-09-15-source-runtime-reliability.md)
lists 28 scenarios. This file maps each one to the automated test that proves
it, states whether that test passes on the CI runner (Blacksmith Ubuntu
24.04, `core_tests.yml`), and records the observed behavior where it does not.
Nothing here depends on a personal machine or a NAS.

Status values:

- `passing`: the named test runs in CI and passes at the commit that landed
  this file.
- `failing`: the named test exists, is marked
  `#[ignore = "R8: <row> fails: <reason>"]`, and fails when run with
  `--ignored`. Each one is a fix brief.
- `not automatable`: no Linux test can prove the row; the reason says why.

How the suites run:

- Colocated tests (`core/src/...`) run in the `--lib` suite of
  `cargo xtask test-core --unit`.
- `crates/store/tests` run as the "Store crate tests" suite of
  `cargo xtask test-core --integration`.
- `core/tests/source_runtime_acceptance_test.rs` is one `Core` over
  temporary directories (the single-daemon rows).
- `core/tests/source_replication_test.rs` is two daemons in separate
  processes paired over loopback (the peer rows).

Run an ignored row by name, for example:

```sh
cargo test -p sd-core --lib a_repeat_observation_repairs_a_failed_write -- --ignored
```

## Matrix

| # | Scenario | Test | Status |
|---|---|---|---|
| 1 | Child before parent across batches | `core/src/ops/indexing/store.rs` `a_child_arriving_before_its_parent_batch_gets_real_ancestry`, `a_batch_builds_the_tree_whatever_order_it_arrives_in`; `crates/store/tests/files.rs` `a_child_ahead_of_its_parent_still_commits` | passing |
| 2 | Shallow browse inside an empty retained source | `core/src/ops/indexing/store.rs` `a_shallow_browse_in_an_empty_store_persists_its_ancestry` | passing |
| 3 | One failed row and successful siblings | `core/src/ops/indexing/store.rs` `one_failed_row_keeps_its_siblings_and_fails_the_flush` (siblings retained, flush refuses and counts the loss); `a_repeat_observation_repairs_a_failed_write` (the R1 proof that a repeat observation repairs the row) | passing / failing (see F1) |
| 4 | Writer dies or pending query fails | `core/src/ops/indexing/store.rs` `a_dead_writer_is_a_typed_failure_not_an_empty_queue`, `a_failed_commit_surfaces_at_the_flush_barrier` | passing |
| 5 | Restart between identity assignment and commit | `core/src/ops/indexing/store.rs` `a_restart_after_assignment_keeps_the_identity_and_lands_the_write` | passing |
| 6 | Interrupted or partially denied scan | `crates/store/tests/files.rs` `an_interrupted_sweep_does_not_delete_the_source`, `a_subtree_the_walk_could_not_read_survives_the_sweep`, `a_sweep_that_saw_almost_nothing_is_refused`; `core/src/ops/indexing/store.rs` `a_locked_folder_costs_its_subtree_not_the_walk`; `core/src/ops/indexing/job.rs` `only_a_whole_unfiltered_walk_may_sweep`, `a_browse_may_not_sweep`, `rules_bar_a_sweep`, `a_depth_limit_bars_a_sweep` | passing |
| 7 | Old schema with origin offline | `crates/store/tests/files.rs` `an_index_that_predates_parent_addressing_is_refused_intact` (the compatibility-status branch: the old table and its assertions stay intact; no data-preserving migration exists yet) | passing |
| 8 | Same volume, nested roots, reversed registration order | `core/src/ops/indexing/sources.rs` `a_nested_source_never_redefines_its_volume_root`, `reversed_registration_order_keeps_one_volume_boundary`; `core/src/ops/indexing/volume_index.rs` `a_nested_source_shares_the_drive_it_sits_on`, `nested_sources_registered_inner_first_share_one_map_and_identity` | passing |
| 9 | Remount and APFS alias changes | Remount: `core/src/ops/indexing/sources.rs` `a_remount_keeps_the_source`, `folders_on_one_drive_are_distinct_sources`, `a_different_drive_at_the_same_mount_point_is_a_different_source`; `core/src/ops/indexing/volume_index.rs` `a_root_mismatched_snapshot_moves_aside_instead_of_deleting`. APFS alias: none | passing (remount) / not automatable (APFS: firmlink spellings such as `/System/Volumes/Data/Users/x` only exist on macOS; `VolumeManager::locate_path` resolves them through the live volume list, which a Linux runner cannot produce) |
| 10 | Missing or invalid restart snapshot | `core/src/ops/indexing/volume_index.rs` `an_invalid_snapshot_leaves_the_source_visible_and_its_store_readable`, `test_snapshot_roundtrip_and_detached_restore` (missing); `an_invalid_snapshot_is_retained_for_diagnosis` | passing / failing (see F2) |
| 11 | Failed watcher subscription | `core/tests/source_runtime_acceptance_test.rs` `a_refused_watch_is_not_reported_active` | failing (see F3) |
| 12 | Offline client restart | `core/src/service/mounts/peer.rs` `a_cold_restore_rebuilds_the_inventory_without_the_owner`, `published_facts_outlive_the_owners_connection`, `a_manifest_without_facts_still_restores` | passing |
| 13 | One failing source among nine | `core/src/service/mounts/peer.rs` `one_failing_source_among_nine_stays_listed_as_unavailable` | passing |
| 14 | Listing/fetch generation race | `core/src/service/mounts/peer.rs` `the_generation_recorded_names_the_delivered_bytes`, `a_corrupt_delivery_never_replaces_a_good_artifact` | passing |
| 15 | Unchanged owner for ten intervals | `core/tests/source_replication_test.rs` `test_source_replication` (Bob's ten passes after convergence: same generation, same sync time, same loaded arena); `core/src/service/mounts/peer.rs` `dirtiness_alone_paces_while_a_moved_generation_transfers`, `an_unchanged_store_exports_identical_bytes`; `crates/store/tests/revision.rs` `reopening_an_unchanged_store_rewrites_no_triggers` | passing |
| 16 | Continuous real writes | `core/tests/source_replication_test.rs` `test_source_replication` (Alice writes and re-walks; Bob lands a newer generation holding every record); `crates/store/tests/revision.rs` `file_changes_move_the_revision_and_nothing_else_does` | passing |
| 17 | Repeated subtree clear and refill | `core/src/ops/indexing/arena.rs` `repeated_clear_and_refill_keeps_allocation_bounded` | failing (see F4) |
| 18 | Cold search across 100 stores | `crates/store/tests/scale.rs` `a_hundred_cold_stores_answer_without_writers` (CI size: 100 stores x 200 records); `a_cold_fan_out_across_many_stores` (the measurement, ignored, sized by `SD_SCALE_*`) | passing |
| 19 | Same capture read through arena and SQLite | `core/src/ops/search/arena_search.rs` `the_store_backend_matches_the_arena_for_the_same_capture` (identities, matching, filters, scores); `core/src/ops/search/pipeline.rs` `every_sort_field_orders_and_reverses`, `equal_scores_tiebreak_deterministically`, `a_page_is_a_window_over_the_sorted_whole` (ordering and pagination shared by both backends) | passing |
| 20 | Suitable loaded arena returns no matches | `core/tests/source_runtime_acceptance_test.rs` `an_empty_answer_from_a_suitable_arena_is_final` | passing |
| 21 | Loaded arena has insufficient coverage or query support | `core/tests/source_runtime_acceptance_test.rs` `an_unwalked_source_answers_from_its_store` | passing |
| 22 | Five suitable arenas and 95 stores | `core/tests/source_runtime_acceptance_test.rs` `five_arenas_and_ninety_five_stores_page_the_same` | passing |
| 23 | Arena candidates need store-only filter or sort fields | `core/tests/source_runtime_acceptance_test.rs` `a_store_only_filter_narrows_before_pagination` (tags are the store-only field) | passing |
| 24 | Search with requested limit five | `core/tests/source_runtime_acceptance_test.rs` `a_limit_of_five_returns_five_with_an_honest_total` | passing |
| 25 | Same record in multiple representations | `core/tests/source_runtime_acceptance_test.rs` `a_file_under_nested_sources_is_one_hit_from_the_arena` (loaded); `a_file_under_nested_sources_is_one_hit_from_the_stores` (store-backed) | passing / failing (see F5) |
| 26 | Replica replacement with local assertions | none | not automatable today: replicas open read-only and carry no receiver-owned assertions (R6 results, "receiver-owned assertions on replica databases" is a registered follow-on gated on FD2), so there is no local assertion to preserve. `crates/store/tests/files.rs` `a_moved_file_carries_its_assertions_with_it` and `a_removal_takes_the_facet_and_leaves_the_assertion` cover the owner-side contract the replica path will have to reuse |
| 27 | Mapped volume with no source | `core/src/ops/indexing/volume_index.rs` `a_tracked_drive_maps_without_appearing_as_a_source` (browse and snapshot), `a_mapped_drive_without_a_source_is_watchable` (watcher); `core/src/ops/indexing/store.rs` `a_partition_with_no_store_still_browses` | passing |
| 28 | Status query and resource event | `core/tests/source_runtime_acceptance_test.rs` `listing_status_and_store_agree_on_a_sources_count` (`sources.list`, `core.index_status` and the store report one count and one observation time for one source) | passing |

Totals: 21 rows fully passing; 3 rows with a passing half and a failing
half (3, 10, 25); 2 rows failing outright (11, 17); row 9 passing except
its APFS half, which no Linux runner can produce; row 26 not automatable
because the feature it names does not exist yet. Five ignored tests in all
(F1 to F5).

## Failing rows

Each is an ignored test that fails at the commit that landed this file. Run
it with `--ignored` to see the behavior.

### F1. Row 3: a repeat observation does not repair a failed write

`core/src/ops/indexing/store.rs` `a_repeat_observation_repairs_a_failed_write`.
After a batch loses one row to a database failure, the ledger keeps the
binding it made before the commit. The next walk observes the same file
unchanged, the ledger answers `Unchanged`, and nothing is written: the row is
missing for as long as the file's size and mtime hold. Observed: the store
holds `["docs"]` where `["docs", "docs/bad.txt"]` was expected. The commit
salvage in `commit` (store.rs) drops the failed write without unbinding it.

### F2. Row 10: an unreadable snapshot is deleted on load

`core/src/ops/indexing/volume_index.rs` `an_invalid_snapshot_is_retained_for_diagnosis`.
`snapshot.rs` removes the file when it cannot decode or parse it
(`Unreadable snapshot ... removing`), so the artifact is gone before anyone
can inspect it. The source stays visible and its store still answers (the
passing half of the row); only retention fails. R2 asks for the artifact to
stay until a validated replacement lands.

### F3. Row 11: a refused watch is reported active

`core/tests/source_runtime_acceptance_test.rs` `a_refused_watch_is_not_reported_active`.
`FsWatcherService::watch_root` calls `VolumeIndex::register_for_watching`
before `watcher.watch_path`. When the OS refuses the subscription (the
directory was removed), the error propagates but the volume index keeps the
root in `watched_paths`, so `is_watched` and `core.index_status` report an
active watch that does not exist. R2 asks for registration only after the OS
accepts.

### F4. Row 17: arena allocation grows with history

`core/src/ops/indexing/arena.rs` `repeated_clear_and_refill_keeps_allocation_bounded`.
One hundred clear-and-refill cycles over a 50-file branch leave 54 live
paths and 5,154 allocated slots. `NodeArena::vacate` keeps the slot, nothing
compacts, and the snapshot serializes every slot. This is R4, which has not
landed.

### F5. Row 25: store-backed search returns one hit per nested store

`core/tests/source_runtime_acceptance_test.rs` `a_file_under_nested_sources_is_one_hit_from_the_stores`.
With an outer and an inner source both walked, the file under the inner root
is committed to both stores. After a restart with no snapshot, every source
answers from its store and `search_every_index` queries each store in turn,
so the file surfaces twice (`total_found` 2 where 1 was expected). The loaded
case is one hit because both registrations share one arena. R6 asks for
deduplication of identities shared by nested sources with provenance kept.

## Not in the plan's table but covered on the way

- Unchanged refresh: `an_unchanged_generation_still_refreshes_the_owner_facts` (peer.rs).
- Resumable replica transfer: `an_interrupted_transfer_resumes_from_the_part_length`,
  `a_part_from_another_generation_starts_over` (peer.rs).
- Database replica restore: `a_database_artifact_restores_into_a_browsable_replica` (peer.rs).
