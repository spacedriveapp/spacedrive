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
- `fixed`: the row failed at the commit that landed this file and a later
  commit made its test pass; the row's section below records the root cause
  and the fix.
- `not automatable`: no Linux test can prove the row; the reason says why.

How the suites run:

- Colocated tests (`core/src/...`) run in the `--lib` suite of
  `cargo xtask test-core --unit`.
- The other three run as `cargo xtask test-core --acceptance`, the
  `acceptance` job of `core_tests.yml`:
  `crates/store/tests` ("Store crate tests");
  `core/tests/source_runtime_acceptance_test.rs`, one `Core` over temporary
  directories (the single-daemon rows);
  `core/tests/source_replication_test.rs`, two daemons in separate processes
  paired over loopback (the peer rows).

Run an ignored row by name, for example:

```sh
cargo test -p sd-core --lib repeated_clear_and_refill_keeps_allocation_bounded -- --ignored
```

## Matrix

| # | Scenario | Test | Status |
|---|---|---|---|
| 1 | Child before parent across batches | `core/src/ops/indexing/store.rs` `a_child_arriving_before_its_parent_batch_gets_real_ancestry`, `a_batch_builds_the_tree_whatever_order_it_arrives_in`; `crates/store/tests/files.rs` `a_child_ahead_of_its_parent_still_commits` | passing |
| 2 | Shallow browse inside an empty retained source | `core/src/ops/indexing/store.rs` `a_shallow_browse_in_an_empty_store_persists_its_ancestry` | passing |
| 3 | One failed row and successful siblings | `core/src/ops/indexing/store.rs` `one_failed_row_keeps_its_siblings_and_fails_the_flush` (siblings retained, flush refuses and counts the loss); `a_repeat_observation_repairs_a_failed_write` (the R1 proof that a repeat observation repairs the row) | passing / fixed (see F1) |
| 4 | Writer dies or pending query fails | `core/src/ops/indexing/store.rs` `a_dead_writer_is_a_typed_failure_not_an_empty_queue`, `a_failed_commit_surfaces_at_the_flush_barrier` | passing |
| 5 | Restart between identity assignment and commit | `core/src/ops/indexing/store.rs` `a_restart_after_assignment_keeps_the_identity_and_lands_the_write` | passing |
| 6 | Interrupted or partially denied scan | `crates/store/tests/files.rs` `an_interrupted_sweep_does_not_delete_the_source`, `a_subtree_the_walk_could_not_read_survives_the_sweep`, `a_sweep_that_saw_almost_nothing_is_refused`; `core/src/ops/indexing/store.rs` `a_locked_folder_costs_its_subtree_not_the_walk`; `core/src/ops/indexing/job.rs` `only_a_whole_unfiltered_walk_may_sweep`, `a_browse_may_not_sweep`, `rules_bar_a_sweep`, `a_depth_limit_bars_a_sweep` | passing |
| 7 | Old schema with origin offline | `crates/store/tests/files.rs` `an_index_that_predates_parent_addressing_is_refused_intact` (the compatibility-status branch: the old table and its assertions stay intact; no data-preserving migration exists yet) | passing |
| 8 | Same volume, nested roots, reversed registration order | `core/src/ops/indexing/sources.rs` `a_nested_source_never_redefines_its_volume_root`, `reversed_registration_order_keeps_one_volume_boundary`; `core/src/ops/indexing/volume_index.rs` `a_nested_source_shares_the_drive_it_sits_on`, `nested_sources_registered_inner_first_share_one_map_and_identity` | passing |
| 9 | Remount and APFS alias changes | Remount: `core/src/ops/indexing/sources.rs` `a_remount_keeps_the_source`, `folders_on_one_drive_are_distinct_sources`, `a_different_drive_at_the_same_mount_point_is_a_different_source`; `core/src/ops/indexing/volume_index.rs` `a_root_mismatched_snapshot_moves_aside_instead_of_deleting`. APFS alias: none | passing (remount) / not automatable (APFS: firmlink spellings such as `/System/Volumes/Data/Users/x` only exist on macOS; `VolumeManager::locate_path` resolves them through the live volume list, which a Linux runner cannot produce) |
| 10 | Missing or invalid restart snapshot | `core/tests/snapshot_rebuild_acceptance_test.rs` `a_missing_snapshot_rebuilds_the_map_from_the_store`, `a_corrupt_snapshot_is_quarantined_and_the_map_rebuilt_from_the_store` (real restart); `core/src/ops/indexing/volume_index.rs` `an_invalid_snapshot_leaves_the_source_visible_and_rebuilt_from_its_store`, `an_older_format_snapshot_is_quarantined_and_the_map_rebuilt_from_the_store`, `an_invalid_snapshot_is_retained_for_diagnosis`, `test_snapshot_roundtrip_and_detached_restore` | passing / fixed (see F2) |
| 11 | Failed watcher subscription | `core/tests/source_runtime_acceptance_test.rs` `a_refused_watch_is_not_reported_active` (refusal reported with its reason on `core.index_status`, then a retry flips the root to active) | fixed (see F3) |
| 12 | Offline client restart | `core/src/service/mounts/peer.rs` `a_cold_restore_rebuilds_the_inventory_without_the_owner`, `published_facts_outlive_the_owners_connection`, `a_manifest_without_facts_still_restores` | passing |
| 13 | One failing source among nine | `core/src/service/mounts/peer.rs` `one_failing_source_among_nine_stays_listed_as_unavailable` | passing |
| 14 | Listing/fetch generation race | `core/src/service/mounts/peer.rs` `the_generation_recorded_names_the_delivered_bytes`, `a_corrupt_delivery_never_replaces_a_good_artifact` | passing |
| 15 | Unchanged owner for ten intervals | `core/tests/source_replication_test.rs` `test_source_replication` (Bob's ten passes after convergence: same generation, same sync time, same loaded arena); `core/src/service/mounts/peer.rs` `dirtiness_alone_paces_while_a_moved_generation_transfers`, `an_unchanged_store_exports_identical_bytes`; `crates/store/tests/revision.rs` `reopening_an_unchanged_store_rewrites_no_triggers` | passing |
| 16 | Continuous real writes | `core/tests/source_replication_test.rs` `test_source_replication` (Alice writes and re-walks; Bob lands a newer generation holding every record); `crates/store/tests/revision.rs` `file_changes_move_the_revision_and_nothing_else_does` | passing |
| 17 | Repeated subtree clear and refill | `core/src/ops/indexing/arena.rs` `repeated_clear_and_refill_keeps_allocation_bounded` (slots bounded by the live tree across 100 cycles, snapshot carries live nodes), `a_shrunken_tree_compacts_once_vacancy_outgrows_it` (the compaction trigger), `a_snapshot_after_churn_carries_only_live_nodes` (round-trip after churn), `a_listing_cursor_taken_before_compaction_still_pages` (path cursors survive renumbering); `core/src/ops/indexing/nodes.rs` `a_vacated_slot_is_reused_and_unreadable_until_then` | fixed (see F4) |
| 18 | Cold search across 100 stores | `crates/store/tests/scale.rs` `a_hundred_cold_stores_answer_without_writers` (CI size: 100 stores x 200 records); `a_cold_fan_out_across_many_stores` (the measurement, ignored, sized by `SD_SCALE_*`) | passing |
| 19 | Same capture read through arena and SQLite | `core/src/ops/search/arena_search.rs` `the_store_backend_matches_the_arena_for_the_same_capture` (identities, matching, filters, scores); `core/src/ops/search/pipeline.rs` `every_sort_field_orders_and_reverses`, `equal_scores_tiebreak_deterministically`, `a_page_is_a_window_over_the_sorted_whole` (ordering and pagination shared by both backends) | passing |
| 20 | Suitable loaded arena returns no matches | `core/tests/source_runtime_acceptance_test.rs` `an_empty_answer_from_a_suitable_arena_is_final` | passing |
| 21 | Loaded arena has insufficient coverage or query support | `core/tests/source_runtime_acceptance_test.rs` `an_unwalked_source_answers_from_its_store` | passing |
| 22 | Five suitable arenas and 95 stores | `core/tests/source_runtime_acceptance_test.rs` `five_arenas_and_ninety_five_stores_page_the_same` | passing |
| 23 | Arena candidates need store-only filter or sort fields | `core/tests/source_runtime_acceptance_test.rs` `a_store_only_filter_narrows_before_pagination` (tags are the store-only field) | passing |
| 24 | Search with requested limit five | `core/tests/source_runtime_acceptance_test.rs` `a_limit_of_five_returns_five_with_an_honest_total` | passing |
| 25 | Same record in multiple representations | `core/tests/source_runtime_acceptance_test.rs` `a_file_under_nested_sources_is_one_hit_from_the_arena` (loaded); `a_file_under_nested_sources_is_one_hit_from_the_stores` (store-backed) | passing / fixed (see F5) |
| 26 | Replica replacement with local assertions | none | not automatable today: replicas open read-only and carry no receiver-owned assertions (R6 results, "receiver-owned assertions on replica databases" is a registered follow-on gated on FD2), so there is no local assertion to preserve. `crates/store/tests/files.rs` `a_moved_file_carries_its_assertions_with_it` and `a_removal_takes_the_facet_and_leaves_the_assertion` cover the owner-side contract the replica path will have to reuse |
| 27 | Mapped volume with no source | `core/src/ops/indexing/volume_index.rs` `a_tracked_drive_maps_without_appearing_as_a_source` (browse and snapshot), `a_mapped_drive_without_a_source_is_watchable` (watcher); `core/src/ops/indexing/store.rs` `a_partition_with_no_store_still_browses` | passing |
| 28 | Status query and resource event | `core/tests/source_runtime_acceptance_test.rs` `listing_status_and_store_agree_on_a_sources_count` (`sources.list`, `core.index_status` and the store report one count and one observation time for one source) | passing |

Totals: 26 rows fully passing (21 at the commit that landed this file, plus
rows 3, 10, 11, 17 and 25 fixed since); row 9 passing except its APFS half,
which no Linux runner can produce; row 26 not automatable because the
feature it names does not exist yet. No ignored test remains.

## Failing and fixed rows

F1 to F5 record what failed at the commit that landed this file and what
fixed it.

### F1. Row 3: a repeat observation did not repair a failed write (fixed)

`core/src/ops/indexing/store.rs` `a_repeat_observation_repairs_a_failed_write`.
After a batch lost one row to a database failure, the ledger kept the
binding it made before the commit. The next walk observed the same file
unchanged, the ledger answered `Unchanged`, and nothing was written: the row
was missing for as long as the file's size and mtime held. Observed: the
store held `["docs"]` where `["docs", "docs/bad.txt"]` was expected.

Fix: `commit` (store.rs) now takes the ledger and calls
`Ledger::unsettle(uuid)` for every row the salvage could not land.
`unsettle` keeps the uuid at its path but clears the size and mtime
evidence, so the next observation resolves `Changed` and rewrites the whole
row under the identity the arena already holds. Dropping the binding
instead would have minted a second identity for the same file.

### F2. Row 10: an unreadable snapshot was deleted on load (fixed)

`core/src/ops/indexing/volume_index.rs` `an_invalid_snapshot_is_retained_for_diagnosis`.
`snapshot.rs` removed the file when it could not decode or parse it, so the
artifact was gone before anyone could inspect it. The source stayed visible
and its store still answered (the passing half of the row); only retention
failed.

Fix: `load_snapshot_impl` moves an artifact it cannot read, decode, or
accept the version of to `<name>.corrupt-<unix seconds>` beside the slot
and logs a WARN naming both paths. The slot is clear for the next save and
no launch parses the artifact again; the test asserts the bytes survive at
the aside path. One retained copy is enough: when a `.corrupt-*` sibling
already exists, a later unreadable artifact in the slot is removed instead,
so a recurring failure cannot fill the disk.

Follow-up: a partition whose snapshot is missing, quarantined, or from an
older format no longer costs a walk. `VolumeIndex::ensure_restored` starts
a background fill of every attached source on the drive from its store, the
same fill a library restore uses, and resolves at once so no reader waits
on it; each root is in progress until its fill lands, a listing serves the
store meanwhile, and the landed source announces its root, which routes it
to the arena and re-arms its watch. The coverage heal finds the source in
progress or covered and dispatches nothing. The rebuild
runs for a source whose registry row carries a record count, which a
snapshot save writes, so it is the evidence a map existed and was lost; a
source never walked to completion keeps answering from its store without
an arena (rows 4 and 5 of this matrix), and a source with no store records
is left for the heal to walk. A store rebuild covers
the registered sources and not the rest of the drive, so the discovery pass
(`map_attached_volumes` with defaults) still maps the drive around them in
the background, as it does for any drive without a snapshot. The real
restart is `core/tests/snapshot_rebuild_acceptance_test.rs`: the listing is
served from the store while the fill runs and from the arena once it has
landed, no job is dispatched, and `core.index_status` then reports the
source restored with nothing in progress. The 1M-record rebuild time is
recorded in `docs/core/design/ephemeral-snapshot-format.md`.

### F3. Row 11: a refused watch was reported active (fixed)

`core/tests/source_runtime_acceptance_test.rs` `a_refused_watch_is_not_reported_active`.
`FsWatcherService::watch_root` called `VolumeIndex::register_for_watching`
before `watcher.watch_path`. When the OS refused the subscription (the
directory was removed), the error propagated but the volume index kept the
root in `watched_paths`, so `is_watched` and `core.index_status` reported
an active watch that did not exist.

Fix: `watch_root` subscribes at the OS first and registers only after the
OS accepts; a refusal is recorded on the volume index with its reason
(`VolumeIndex::record_watch_refusal`) and surfaces as
`IndexStatus::refused_watches`, which `sd index status` prints beside the
watched roots. The service retries every refused root on
`FsWatcherServiceConfig::watch_retry_interval` (30 s) while it runs
(`retry_refused_watches`), and a retry the OS accepts moves the root from
the refused list to `watched_paths`. The test covers both halves: the
refusal with its reason, then the directory returning and one retry arming
the watch.

### F4. Row 17: arena allocation grows with history (fixed)

`core/src/ops/indexing/arena.rs` `repeated_clear_and_refill_keeps_allocation_bounded`.
One hundred clear-and-refill cycles over a 50-file branch left 54 live
paths and 5,154 allocated slots. `NodeArena::vacate` kept the slot, nothing
compacted, and the snapshot serialized every slot.

Fix (R4): `NodeArena` keeps a free list. `vacate` drops the node's heap,
marks the slot unoccupied (so a lookup through the old id answers nothing)
and queues it; the next `insert` takes a queued slot before growing. A
cleared and refilled branch therefore reuses exactly the slots it had: the
same 100 cycles end at 54 slots for 54 live paths. When a tree shrinks for
real the free list grows instead, and `Arena::compact_if_inflated`, called
after every removal, rebuilds the arena densely once vacant slots outnumber
live entries and number at least `COMPACT_MIN_VACANT` (4,096). The first
condition bounds allocation at twice the live tree and makes the pass
amortized (one walk of the live graph, paid for by at least as many
removals); the floor keeps a small arena from rebuilding over a few hundred
bytes of slack. `compact` renumbers parent links, child lists, both path
maps, uuid bindings, content kinds, collection flags, summary stubs and the
name registry together, and builds the new arena in full before swapping it
in, so a failed mapping leaves the old one answering.

Slot reuse and renumbering are safe because `EntryId` never leaves the arena
module: the public API speaks paths and uuids, the explorer's media cursor
is a source, a directory and a name, library search pages by offset over
sorted paths, and the watcher addresses entries by path. Record uuids,
rollups and query results are unchanged by either; a cursor taken before a
compaction names the same place after it, which
`a_listing_cursor_taken_before_compaction_still_pages` proves at the arena.

The restart snapshot (format version 4) is written through the same dense
projection whether or not the session's arena compacted, so it carries one
node per live entry and the reader restores a dense arena. A version 3
artifact is quarantined on load like any other mismatch and the source
reindexes once.

### F5. Row 25: store-backed search returned one hit per nested store (fixed)

`core/tests/source_runtime_acceptance_test.rs` `a_file_under_nested_sources_is_one_hit_from_the_stores`.
With an outer and an inner source both walked, the file under the inner root
is committed to both stores. After a restart with no snapshot, every source
answered from its store and `search_every_index` queried each store in turn,
so the file surfaced twice (`total_found` 2 where 1 was expected). The
loaded case was one hit because both registrations share one arena.

Fix: the store loop in `search_every_index` (arena_search.rs) reads sources
innermost first and keeps a set of local paths already answered, seeded
with every arena's full match set (collected before the page window
narrows it), so a file an inner store (its owner) returned is dropped from
the outer store's partition before it is counted. The total stays exact
because the de-duplication runs on each store's full match set ahead of the
page window.

## Not in the plan's table but covered on the way

- Unchanged refresh: `an_unchanged_generation_still_refreshes_the_owner_facts` (peer.rs).
- Resumable replica transfer: `an_interrupted_transfer_resumes_from_the_part_length`,
  `a_part_from_another_generation_starts_over` (peer.rs).
- Database replica restore: `a_database_artifact_restores_into_a_browsable_replica` (peer.rs).
