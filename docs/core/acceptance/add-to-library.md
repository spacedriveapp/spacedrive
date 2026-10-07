# Add to Library acceptance

The [Add to Library plan](../../plans/2026-09-16-add-to-library.md) ends in
acceptance cases. This file maps the ones the core half can prove on Linux
to the test that proves them and states whether that test passes on the CI
runner (Blacksmith Ubuntu 24.04, `core_tests.yml`). The modal is a separate
piece of work; rows that need it, a second daemon or a real removable drive
are listed as not provable here.

How the suite runs:

- `core/tests/add_to_library_acceptance_test.rs` runs in the `acceptance`
  job of `core_tests.yml` through `cargo xtask test-core --acceptance`. One
  `Core` over temporary directories tracks folders with defaults and with
  overrides, removes one, and tracks it again. The temporary root sits on the
  runner's data volume, which detection reports, so the volume rows are that
  volume's.
- The colocated tests run in the `--lib` suite of
  `cargo xtask test-core --unit`.

Status values follow `source-runtime.md`.

## Matrix

| # | Case | Scenario | Test | Status |
|---|---|---|---|---|
| 1 | Defaults | Every entry point uses the same effective defaults: Library Settings > Adding content, as the plan proposes them (store in library, offline copy kept, filtered capture, content identified) | `core/tests/add_to_library_acceptance_test.rs` `add_to_library_resolves_defaults_tracks_the_volume_and_keeps_the_catalog` (section 1); `core/src/library/config.rs` `add_defaults_are_the_plans_proposal`; `core/src/ops/sources/track/action.rs` `an_input_without_overrides_takes_the_defaults` | passing |
| 2 | Overrides | A per-add override replaces one default for that add and never writes the defaults; a changed default applies to the next add and moves nothing | acceptance test sections 2 and the closing default change; `core/src/library/config.rs` `an_override_replaces_one_default_and_leaves_the_rest` | passing |
| 3 | Volume | Adding a folder registers its containing volume once; a second folder on the drive reuses the row; the index maps the drive | acceptance test section 3 (volume row count stays 1 across two folders; `volume_mounted` answers) | passing |
| 4 | Placement | In library resolves under the data directory, On source under `<root>/.spacedrive/sources/<id>`; the same directory resolves after a restart, `sources.list` reports it, and the library's read queries (`sources.list_records`, `sources.list_items`, `sources.media_listing`) and `libraries.backup` read the store from there | acceptance test sections 1 and 2 (`sources.list_records` on the on-source source); `core/src/ops/indexing/volume_index.rs` `placement_resolves_the_store_directory` | passing |
| 5 | Managed directories | An unfiltered source does not ingest its own `.spacedrive` store, and an enclosing source excludes it and the library's data directory | acceptance test section 4; `core/src/config/mod.rs` `managed_directories_are_refused_by_component` | passing |
| 6 | Removal | Removing a source keeps its catalog and its volume row; the registration and the open handles go | acceptance test section 5; `core/src/ops/indexing/volume_index.rs` `a_removed_source_is_readopted_from_its_descriptor`; `core/src/ops/sources/delete/action.rs` `deletion_of_the_catalog_is_opt_in` | passing |
| 7 | Identity | Re-adding the scope reopens the catalog under the identity its descriptor carries, under either placement and whatever placement the add asked for; a re-track names nothing and changes nothing; a file keeps its record id; deleting the catalog is explicit and a later add starts over | acceptance test sections 6 and 7; `core/src/ops/indexing/descriptor.rs` `a_descriptor_binds_to_library_volume_and_path`, `find_bound_picks_the_store_of_this_scope` | passing |
| 8 | Network | On source is refused for a network or cloud volume, whose serving daemon keeps the store in the library | `core/src/ops/sources/track/action.rs` (refusal before registration) | not provable on the runner: no network volume; the refusal is unit-level code with no fixture yet |
| 9 | Remount | Detach and remount an on-source store without losing identity or replacing it | not built: an on-source store of a drive that is away resolves to no directory and is not reopened empty, but the loop-device remount row is not written |
| 10 | Offline copy | Verify offline behavior with and without a retained library copy | not built: `keep_offline_copy` is recorded intent; the copy itself is step 4 of the plan |
| 11 | Entry points | Every entry point uses Add to Library through the modal | the modal is separate work; `sources.track` and `volumes.track` share one body (`track_and_index`) and one override shape |

## Decisions carried

The effective capture default is filtered. The plan recorded that the
whole-volume entry point defaulted external drives to unfiltered while folder
adds defaulted to filtered. The library default is now one value, filtered,
and `volumes.track` passes an explicit unfiltered override for an external
drive unless the caller set one, so the archival behavior stays while every
entry point reads the same defaults.

A store's identity is adopted only by the library that wrote it, for the
same volume and the same path within it. A descriptor from another library
or another drive is evidence to inspect, and a new source starts beside it.

A restored backup lays every store in the in-library layout. An on-source
store is backed up from the drive, so the archive holds its assertions, but
a restore does not put it back on the drive; the registration still says
on source and resolves there. Relocation (plan step 5) is where that copy
would move.

Removing a source never deletes its volume row. The plan asks for an
explicit containing-volume retention rule; the rule built is that the row
stays, and `volumes.untrack` remains the way to forget a drive.
