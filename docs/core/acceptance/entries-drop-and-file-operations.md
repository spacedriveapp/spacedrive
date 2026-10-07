# Entries drop and file operations acceptance matrix

The FDA phase of
[the entries final drop plan](../../plans/2026-09-15-entries-final-drop.md)
is its Acceptance section: a static boundary, four data-safety rows, four
product-behavior rows, and a build check. The
[action previews plan](../../plans/2026-09-22-action-previews.md) and the
[file operations plan](../../plans/2026-09-23-file-operations.md) each end
in a live acceptance case over Jamie's Expansion drive and `jamie-nas`,
which nobody can run in CI. This file maps every FDA row and every clause
of the two live cases to the automated test that proves it on the CI runner
(Blacksmith Ubuntu 24.04, `core_tests.yml`), states whether it passes at
the commit that landed this file, and records the observed behavior where
it does not. Nothing here depends on a personal machine or a NAS.

The release gates these rows are evidence for are in
[Releases](../releases.mdx), under Data safety: "Candidate content does not
authorize deletion", "Copy, move, archive, and duplicate cleanup verify
required outcomes", and "Source reindex preserves assertions", plus the
cross-platform gate "Non-UTF-8 or otherwise unrepresentable names fail
visibly".

Status values follow
[source-runtime.md](source-runtime.md):

- `passing`: the named test runs in CI and passes.
- `failing`: the named test exists, is marked
  `#[ignore = "FDA: <row>: <reason>"]`, and fails when run with
  `--ignored`. Each one is a fix brief.
- `not automatable`: no Linux test can prove the row; the reason says why.
- `live only`: the clause is a measurement on real hardware, with a
  CI-sized stand-in named where one makes sense.

How the suites run:

- Colocated tests (`core/src/...`) run in the `--lib` suite of
  `cargo xtask test-core --unit`. The file-operation rows live in
  `core/src/ops/files/acceptance.rs` on the three-folder `Fixture` the
  operation modules share.
- `core/tests/entries_drop_acceptance_test.rs` is one `Core` over temporary
  directories: the schema, boundary, pin, assertion, store-portability and
  naming rows. Two rows run their walk in a child process (the test binary
  re-invoked on one ignored test) because the tracing subscriber and
  `KeyManager::close`'s redb file are process-global.
- `crates/store/tests` run as the "Store crate tests" suite.
- `core/tests/dedupe_own_hash_test.rs` is SPAC-19's P1a test on the
  `addressing-p0-p1a` branch (#3112); it is cited, not duplicated.

Run an ignored row by name, for example:

```sh
cargo test -p sd-core --test entries_drop_acceptance_test a_rename_over_an_existing_file_lands_as_one_row -- --ignored
cargo test -p sd-core --lib acceptance::a_trashed_file_is_listed_with_its_location_and_comes_back -- --ignored
```

## FDA: static boundary

| # | Row | Test | Status |
|---|---|---|---|
| S1 | The three `rg` searches return no production references (`entities::entry`, `entry_closure::Entity`, `directory_paths::Entity`; `register_syncable.*"entry"`, `model_type == "entry"`; `core.ephemeral_(status\|reset)`) | `entries_drop_acceptance_test.rs` `no_production_source_names_the_entry_substrate` (walks `core/src`, `apps`, `crates`, `packages/ts-client/src`; also refuses a raw SQL statement in `core/src` that names any of the 29 dropped tables) | passing |
| S2 | No registered sync model writes to a retired table | `entries_drop_acceptance_test.rs` `every_registered_sync_model_names_a_current_table` (iterates the `inventory` registry; every `table_name` is one of the fourteen) | passing |
| S3 | A fresh `library.db` has no retired tables, indexes, FTS tables, or triggers | `entries_drop_acceptance_test.rs` `a_fresh_library_has_exactly_the_fourteen_tables` (exact table set; no trigger, no FTS shadow, no index on a retired table) | passing |

## FDA: data safety

| # | Row | Test | Status |
|---|---|---|---|
| D1 | Tag definitions, applications, removals, HLCs and device identities live in source stores under the new model | `crates/store/tests/tags.rs` (whole file; the tags plan's acceptance), `core/tests/source_runtime_acceptance_test.rs` `a_store_only_filter_narrows_before_pagination` (a tag filter answered from the store) | passing |
| D2 | Reindexing or evicting a source leaves its assertion tables unchanged (release gate "source reindex preserves assertions") | `entries_drop_acceptance_test.rs` `tags_survive_a_source_reindex` (arena cleared, source walked again; `tag_assertion` rows identical, record keeps its uuid, tag still reaches it); `crates/store/tests/tags.rs` `assertions_survive_generation_loss_and_rebind`; `crates/store/tests/files.rs` `a_moved_file_carries_its_assertions_with_it`, `a_removal_takes_the_facet_and_leaves_the_assertion` | passing |
| D3 | A source store moved to a clean library remains self-describing | `entries_drop_acceptance_test.rs` `a_frozen_store_describes_itself_and_is_not_written_by_a_reader` (a frozen copy opened from another directory with no registry row answers its schema, records and tags) | passing |
| D4 | Frozen copies are byte-for-byte untouched; live stores change only through tested migrations | same test (blake3 of the freeze and of the moved copy unchanged after reading); `crates/store/tests/migrate.rs` on `addressing-p0-p1a` (#3112) for the migration half | passing |
| D5 | An upgraded library keeps every user assertion and converges on the 14-table schema | `entries_drop_acceptance_test.rs` `a_pre_drop_library_upgrades_to_the_fourteen_table_schema` (the pre-drop chain applied, rows in `spaces`/`space_items` including a Location item, then the drop: fourteen tables, pin intact, Location item deleted, `integrity_check` ok, re-running the chain is a no-op) | passing |

## FDA: product behavior

| # | Row | Test | Status |
|---|---|---|---|
| P1 | Browse, search, collections, media view, tags, spaces, copy, move, delete, restart and watcher changes work on local sources | browse: `source_runtime_acceptance_test.rs`, `watcher_test`; search: `search_test` (now registered), rows 19 to 24 of source-runtime.md; tags: D1; spaces: P2; copy/move/delete: F1 to F4 below plus `copy_action_test`, `delete_strategy_test`, `file_move_test`, `folder_rename_test` (now registered); restart: P3; watcher: `watcher_test`, `resource_events_test` (now registered). Collections and media view have no dedicated suite: collections are Space items (P2) and the media listing is `source_runtime_acceptance_test.rs`'s read routing over the same stores | passing |
| P2 | Adding or removing a bookmark neither indexes nor deletes records and does not change processing policies; pins are Space items | `entries_drop_acceptance_test.rs` `a_pin_is_a_space_item_with_no_indexing_side_effect` (pin inside a source and on an untracked folder: no job dispatched, store revision and record count unchanged, source config unchanged, the untracked folder is not tracked; unpinning deletes nothing) | passing |
| P3 | Saved navigation survives a volume remount / restart | `entries_drop_acceptance_test.rs` `a_pin_survives_a_restart` (same data directory reopened; the pin is listed and resolves to its folder). Remount: `core/src/ops/indexing/sources.rs` `a_remount_keeps_the_source` (row 9 of source-runtime.md) | passing (restart, remount of the source) / not automatable (a pin's `SdPath` is a device path, so a drive that remounts at another path changes what the pin names; nothing in the tree re-anchors pins to volumes yet) |
| P4 | The Mac can list titan's sources, browse replicas, stream bytes, dispatch remote ops, watch remote jobs, follow remote logs | `core/tests/source_replication_test.rs` `test_source_replication` (two processes: list, replica fetch, read); `cross_device_copy_test`, `file_copy_pull_test`, `file_transfer_test` (remote ops and transfers); remote logs: SPAC-12's live run (`reports/SPAC-12.md`), no CI test | passing (listing, replicas, bytes, remote ops) / live only (remote job watch and log stream) |
| P5 | A daemon with an old library either upgrades successfully or stops with the documented recovery path | D5 for the upgrade; `entries_drop_acceptance_test.rs` `an_incompatible_library_is_refused_and_left_intact` (a library whose migration table names a migration this build lacks is refused by name, its schema and rows are unchanged, no second library is created) | passing |

## FDA: build and tests

| # | Row | Test | Status |
|---|---|---|---|
| B1 | `cargo fmt --check`, `cargo check`, `cargo check -p sd-core --all-targets --features wasm`, `cargo check -p sd-native`, `cargo test -p sd-store` | `.github/workflows/ci.yml` Formatting and Clippy jobs; `core_tests.yml` Store crate tests. `--features wasm` and `sd-native` are not in any workflow | passing (fmt, clippy, store) / not run in CI (`wasm` feature check, `sd-native`) |

## Action previews plan: live acceptance clauses

The case: track the Expansion drive, validate and preview the merge into
`jamie-nas`, run a consuming merge with policy `Skip`, verify the drive
retains exactly the conflicts and skips, verify no destination file changed
without a `Replace` or `KeepBoth` entry, verify skipped duplicates carry
confirmed integrity hashes, measure preview time, confirm no write.

| # | Clause | Test | Status |
|---|---|---|---|
| V1 | Validate and record the findings; preview and check the summary against a sample | `core/src/ops/files/merge/tests.rs` `a_plan_sorts_every_kind_of_leaf`, `validation_refuses_what_cannot_run_and_previews_a_detached_source`, `a_full_disk_is_a_warning_with_numbers` | passing |
| V2 | Consuming merge with `Skip`: the source retains exactly the conflicts and policy skips | `merge/tests.rs` `policies_apply_and_a_consuming_merge_prunes_the_source`, `the_job_settles_every_leaf_against_the_live_tree` | passing |
| V3 | No destination file changed without a `Replace` or `KeepBoth` entry | `core/src/ops/files/acceptance.rs` `a_merge_changes_no_destination_file_the_plan_did_not_name` (every changed byte has a `Replace` row; unnamed files hold their bytes) | passing |
| V4 | Skipped duplicates carry confirmed integrity hashes (release gate "candidate content does not authorize deletion") | `merge/tests.rs` `a_plan_sorts_every_kind_of_leaf` (candidate vs confirmed skip tiers); `acceptance.rs` `dedupe_keeps_a_copy_whose_bytes_differ_from_its_keeper`; `core/tests/dedupe_own_hash_test.rs` on #3112 (store-side: a sampled write never lands on a confirmed row) | passing |
| V5 | Preflight copy, move, merge, delete and dedupe previews match what the jobs do on the fixture | `acceptance.rs` `preview_rows_match_execution_for_copy_move_merge_and_delete` (the plan's projection of each root equals the files the job leaves there) | passing |
| V6 | Measure preview time from the index on the largest source | `acceptance.rs` `a_batch_rename_preview_over_a_thousand_files_answers_within_the_ceiling` (CI-sized stand-in: 1,000 files validate and preview under 10 s); the merge-preview measurement itself is live only | passing (stand-in) / live only (measurement) |
| V7 | Neither method made a filesystem write | `acceptance.rs` `preflight_makes_no_filesystem_write_and_moves_no_revision` (every byte on disk and every store revision unchanged after validate and preview of six operations) | passing |

## File operations plan: live acceptance clauses

The case: batch rename one day's photos to a dated sequence and undo it;
mirror the folder to `jamie-nas` with extras removed, confirm the plan's
deletes match `file compare` reversed, undo the mirror; flatten a downloads
folder with known collisions; trash a file on the Mac, on titan and on a
network mount and restore each from the trash view; measure batch rename
preview time; confirm no operation left a file the journal does not account
for.

| # | Clause | Test | Status |
|---|---|---|---|
| F1 | Batch rename to a dated sequence, and undo | `acceptance.rs` `a_dated_batch_rename_runs_as_previewed_and_undoes` (`{date:%Y-%m-%d}_{n:03}` plus lowercase extension; preview names every new name, the job produces exactly those, undo restores the originals); `core/src/ops/files/rename/preflight.rs` `a_batch_previews_collisions_and_renames_chains_and_cycles` | passing |
| F2 | Mirror with extras removed; the plan's deletes match `file compare` reversed | `acceptance.rs` `a_mirrors_deletes_are_the_reversed_comparison` (planned `Delete` rows equal the path comparison's only-in-destination set; after the job the destination's file set equals the source's); `merge/tests.rs` `a_mirror_removes_what_no_source_holds` | passing |
| F3 | Undo the mirror | `acceptance.rs` `undoing_a_mirror_restores_the_extras_and_the_replaced_bytes` | failing on Linux and Windows (see F-a); passing on macOS |
| F4 | Flatten a downloads folder with known collisions | `core/src/ops/files/organize/tests.rs` `flattening_numbers_collisions_and_prunes_emptied_folders`, `organizing_by_month_previews_folders_and_moves_and_keeps_identity` | passing |
| F5 | Trash a file and restore it from the trash view, with its location recorded | `acceptance.rs` `a_trashed_file_is_listed_with_its_location_and_comes_back` (journal holds the location, `files.trash_list` lists it present, undo restores the bytes); `core/src/ops/files/undo/tests.rs` `undoing_a_delete_restores_from_the_trash` (macOS) | failing on Linux and Windows (see F-a); passing on macOS |
| F6 | Trash on a network mount (a volume with no OS trash: the Spacedrive trash directory) | `core/src/ops/files/trash.rs` `a_spacedrive_trash_location_is_told_apart`; no test drives `spacedrive_trash` end to end, since it needs a volume the OS trash refuses | passing (recognition) / not automatable (the fallback needs a mount the `trash` crate refuses, which a temp directory on the runner is not) |
| F7 | Measure preview time for a batch rename over the largest folder | `acceptance.rs` `a_batch_rename_preview_over_a_thousand_files_answers_within_the_ceiling` | passing (stand-in) / live only (measurement) |
| F8 | No operation left a file the journal does not account for | `acceptance.rs` `every_file_a_job_touched_is_in_its_journal` (copy, rename, permanent delete: every path that appeared or vanished is, or sits under, a journaled effect) | passing |
| F9 | Archive and extract round trip | `core/src/ops/files/archive/tests.rs` `a_zip_round_trips_with_a_collision_and_a_replacement`, `a_tar_zstd_round_trips_with_a_collision_and_a_replacement`, `an_escaping_entry_is_refused_and_components_strip`, `a_resumed_extract_continues_at_its_entry` | passing |
| F10 | Trash restore on Windows and Linux, "written against the crate and not yet run there" (F3's exit proof) | F3 and F5 above are that run | failing (see F-a) |

## Known limits and release gates with no plan row

| # | Row | Test | Status |
|---|---|---|---|
| K1 | A rename over an existing file inside a source fails to land in the store on `UNIQUE(parent_uuid, title)` (PROJECT_STATUS.md known limit) | `entries_drop_acceptance_test.rs` `a_rename_over_an_existing_file_lands_as_one_row` | failing (see F-b) |
| K2 | Non-UTF-8 names are retained lossily and reported (known limit; release gate "unrepresentable names fail visibly") | `entries_drop_acceptance_test.rs` `a_non_utf8_name_is_retained_lossily_and_reported` (a `\xFF` name is a record under U+FFFD and the walk warns `file name is not valid UTF-8; recorded lossily`, read from a child process's output) | passing |

Totals: 31 rows. 25 pass on the Linux runner; 3 fail there and are
ignored tests (F3, F5 and F10 are one cause, F-a; K1 is F-b); P3, P4, B1,
F6 and the two measurements have a passing automated half and a half that
is live only or not automatable. Three ignored tests in all, four counting
`non_utf8_child_walk` and `pin_restart_child`, which are ignored only so
their parent tests can run them in a child process.

## Failing rows

### F-a. Rows F3, F5, F10: trash restore on Linux renames the `.trashinfo` file over the original

`core/src/ops/files/acceptance.rs`
`a_trashed_file_is_listed_with_its_location_and_comes_back`,
`undoing_a_mirror_restores_the_extras_and_the_replaced_bytes`.

Observed: `trash_os` on Linux records `PathBuf::from(item.id)` as the
item's location. In `trash` 3.3's freedesktop backend `TrashItem::id` is the
path of the `.trashinfo` file under `~/.local/share/Trash/info/`, not the
item under `files/`. `restore` then finds `symlink_metadata(location)`
succeeds (the info file exists) and takes the plain-rename branch, so the
original path ends up holding the trashinfo text (`[Trash Info]\nPath=...`),
and the real bytes stay in `Trash/files/`. The trash view reports the item
as present for the same reason.

Fix: on Linux and Windows either record the restorable file path
(`restorable_file_in_trash_from_info_file` is private in the crate, but the
`files/<stem>` path is derivable from the info path), or make `restore` go
through `restore_os` whenever the location is not a Spacedrive trash
directory instead of branching on `symlink_metadata`. `purge` has the same
branch. Half a day, with these two tests as the proof.

### F-b. Row K1: a rename over an existing file leaves two rows with one title

`core/tests/entries_drop_acceptance_test.rs`
`a_rename_over_an_existing_file_lands_as_one_row`.

Observed: `a.txt` renamed over `b.txt` on disk, reported through
`SourceStore::renamed`, then flushed. The moved record does take the new
name and keeps its uuid, and `a.txt` no longer resolves, but the overwritten
`b.txt` row is never removed, so the store holds two `file` rows titled
`b.txt` under one parent. Through a re-walk instead of the rename event the
end state is worse: `a.txt` still resolves, since nothing in the walk sweeps
a single vanished file inside a source that is otherwise intact. Fix: the rename ingest should treat an existing row at the
destination key as replaced (remove it, or fold its facets into the mover)
before writing the move, and the sweep should reconcile a title collision
under one parent. Half a day.
