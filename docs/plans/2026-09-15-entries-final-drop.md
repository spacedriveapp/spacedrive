# Entries Final Drop

> Status: ready to start, unowned
> Audited: 2026-09-15 against `1044e0977` and the current worktree
> Register: `PROJECT_STATUS.md`
> Replaces the executable part of P6 in
> `2026-08-20-entries-teardown-execution.md`. That document remains the history
> and inventory. This document owns the final drop.

## Outcome

Remove the legacy entry substrate without losing assertions or regressing the
source runtime that replaced it.

The primary filesystem writer, file queries, collections, and library search
already use the volume index and per-source stores. The remaining work is a
boundary cleanup: move the last durable user data, remove entry-aware sync and
compatibility reads, then delete the schema and names that only made sense while
two file models existed.

Completion means a fresh library never creates an entry table, an upgraded
library keeps every user assertion, and the paired-device source path still
works end to end.

## Preserve these boundaries

This is a breaking release. Location data migration and backwards compatibility
are not required. Remove its schema and API directly while updating current
callers to the replacement contracts.

- `crates/store` remains the durable record, content, edge, and assertion
  substrate. Never delete or recreate a source store as part of this work.
- The volume index remains the machine-wide filesystem read map. A source
  remains a retained scope over it.
- `sources`, volumes, devices, jobs, settings, and Space items remain library
  state. Remove location rows and location-specific Space item variants without
  migration, ID preservation or compatibility readers.
- The agreed follow-up accepts `SdPath` to configure processing policies and
  stores a separate source-relative target. `SdPath` gains no source variant.
  Locations are removed; they do not regain indexing, retention, or watcher
  ownership. See
  [Locations Research](2026-09-15-locations-research.md) for the addressing
  decision and the processing proposal.
- Preserve paired-device source discovery, snapshot replication, WebDAV byte
  reads, remote operations, remote jobs, and remote logs. Preserve the
  capabilities, even when their current implementation shares a module with
  legacy row sync.
- Preserve `infra/sync/hlc.rs` and `infra/sync/time_source.rs`. Assertion merges
  need them.
- `catalog.db` follows this work. Do not build it to make the drop possible.

The old P6 instruction to delete `infra/sync` and `service/sync` by directory is
not valid against the current tree. Trace each surviving capability before
removing code. Delete the legacy entry-row protocol, not modern source or peer
transport by association.

## Current residue

### Durable data without its final home

Tag operations still write to the library database through
`UserMetadataManager`, but the tables are empty: zero rows across every tag
and metadata table, verified read-only on the Mac on 2026-09-16. Titan holds
its own library and remains unverified; check it with
`sd-cli --device titan tag search ""` before FD4 drops schema. There is no tag
data to migrate.

`record_overlay` is ready for scalar assertions: it carries record identity,
portable rebind evidence, HLC, and device identity. The source store does not
yet have the `tag_definition` and `tag_assertion` tables specified by
`docs/core/design/tags-and-assertions.md`. Building them is a clean build,
owned by `2026-09-17-tags-on-source-stores.md`, and must land before the
legacy tag tables can be removed.

### Runtime reads and helpers

The primary file surfaces are cut over, but compiled code still reads or names
the old model:

- library discovery counts `entities::entry` rows;
- `File::route_from_dependency` resolves resource events through entries and
  the old content table;
- indexing state and `path_resolver` retain `directory_paths` helpers;
- `library/sync_helpers.rs` materializes directory paths for entry sync;
- collection, sidecar, conduit, content, and user-metadata entities retain entry
  relations even where their old operations have gone.

### Schema and replication

The migration chain still creates `entries`, `entry_closure`,
`directory_paths`, the old content and media tables, sidecar tables, semantic
tag tables, and entry-bound junctions. The entity modules remain compiled.

`entry` is still an inventory-registered sync model. Backfill, dependency
ordering, FK mapping, watermarks, resource routing, and tests still contain
entry-specific behavior. A peer can therefore still materialize rows that the
local indexer no longer writes.

### Cleanup surface

Examples and integration tests still exercise the retired hierarchy. The
`ephemeral` module, type names, two wire operations, generated TypeScript, and
some module documentation still describe a contrast that no longer belongs in
the product.

## Work register

Update this table and the Immediate register in `PROJECT_STATUS.md` in the same
commit when ownership or status changes.

| Slice | Status | Owner | Exit proof |
|---|---|---|---|
| FD0 Baseline and ownership | done 2026-09-18 | Fable | Recorded below: reference inventory, fresh-library tables, live fixture, baseline |
| FD1 Remove runtime entry reads | done 2026-09-18 | Fable | Production readers gone: File's dependency router deleted (only legacy row sync fed it), the ancestor entry-ID cache and SQL path resolver removed, discovery totals read source rows, indexing docs describe one writer |
| FD2 Build tags on source stores | landed; delivery verified live 2026-09-18 | Fable | Source stores are self-describing and live-verified; new-model tag tests pass; production tag and metadata callers of the legacy tables are gone, leaving the entity modules, migrations, and row-sync registrations for FD3/FD4 (`2026-09-17-tags-on-source-stores.md`) |
| FD3 Remove legacy row sync | done 2026-09-18 | Fable | Fourteen retired models unregistered with their Syncable impls and apply paths deleted; entry hierarchy sorting, self-referential FK resolution, closure and directory-path rebuilds, and the directory-path sync payloads are gone; survivors are device, volume, space, space_group, space_item, audit_log, and a registry test now refuses any retired model re-registering; live audit passed: replicas listed, remote ops answered, replica listings served, remote jobs endpoint responsive |
| FD4 Replace the library schema | done 2026-09-18 | Fable | Fresh, legacy, and live libraries all converge on the same 14-table schema; the Mac library upgraded live with a pre-drop backup; titan's library upgraded 2026-09-18 after a backup |
| FD5 Retire compatibility surface | done 2026-09-18 | Opus | Tests, examples, generated types, docs, and names describe one index model: the ephemeral qualifier is retired, entry-era tests and examples are deleted or repointed, and the persistent-era search, event, status, and error surface is gone; recorded below with what still fails and why |
| FDA Acceptance | ready | unowned | Full matrix below passes |

## FD0: Baseline and ownership

> Done 2026-09-18. Reference inventory: production entry readers were
> `library/sync_helpers.rs`, `ops/indexing/path_resolver.rs`,
> `ops/indexing/state.rs` (ancestor cache), `domain/file.rs`
> (`route_from_dependency`), and `service/network/protocol/messaging.rs`
> (discovery counts); everything else sits in entities, migrations, legacy
> row sync, tests, and examples. Registered sync models at the baseline:
> entry, content_identity, sidecar, mime_type, the three media_data tables,
> collection, collection_entry, location, tag, tag_relationship,
> user_metadata, user_metadata_tag, audit_log, device, volume, space,
> space_group, space_item. A fresh library creates 43 tables including the
> whole retired substrate (entries, entry_closure, directory_paths,
> content_identities, mime_types, sidecar tables, semantic tag tables,
> user_metadata tables, collections, location, the FTS search_index family,
> sync_checkpoints/conduit/generation). The live fleet is the fixture: Mac
> and titan paired with replicas, a Space item present. Build baseline:
> `cargo check` and the store and core lib suites pass; `--all-targets`
> fails only in pre-existing entry-era tests and examples
> (`library_demo`, `entry_move_integrity_test`,
> `location_export_import_test`, `event_system_test`, `fs_watcher_test`).


1. Re-run the reference inventory. Classify every hit as production,
   migration, test, example, generated client, or historical documentation.
2. Record the current tables in a fresh library and in a fixture created by the
   last pre-drop build.
3. Create a legacy fixture with:
   - a Space item using the current navigation contract;
   - two paired devices with source replicas.

   The fixture carries no tag rows. No tag data exists to preserve, and the
   tag semantics once listed here (removal must not resurrect, content
   collapse, unbound definitions) are tests of the new model, owned by
   `2026-09-17-tags-on-source-stores.md`.
4. Run the repository baseline before edits. Keep failures that predate the
   slice separate from regressions.

Do not start with a schema deletion. The fixture and the reference inventory
are the proof that makes the later deletion safe.

## FD1: Remove runtime entry reads

> Done 2026-09-18. `File::route_from_dependency` and its dependency list are
> gone; the only emitters were legacy row-sync applies, and file payloads
> announce directly from the volume index. The `seed_ancestor_cache`
> entry-ID machinery and the dead `entries_for_content` field left the
> indexer state, the SQL `path_resolver` module is deleted, library
> discovery totals in the pairing protocol read source rows through
> `calculate_file_statistics_static` with sources standing in for the
> retired location count, and the indexing module docs describe one writer
> and one read map.


1. Derive library discovery counts and bytes from source rows and source-store
   ledgers. Do not fall back to an empty entry table.
2. Route `File` resource events through the volume index and source stores.
   Record-scoped events resolve directly by record UUID. Content-scoped events
   fan out through store content joins.
3. Remove the SQL `path_resolver`, ancestor entry-ID cache, and directory-path
   materialization that no current index walk needs.
4. Remove dead entry imports and entry-era branches from file, search, spaces,
   copy, collections, redundancy, and networking code.
5. Correct indexing module documentation so it describes one writer and one
   read map.

Keep the schema during this slice. The compiler and focused tests should prove
that no production path needs it before FD4 removes it.

## FD2: Build tags on source stores

`2026-09-17-tags-on-source-stores.md` owns this slice. It adds
`tag_definition` and `tag_assertion` to the assertion half of every source
store, rewrites the tag operations against source stores, routes favorite and
notes scalars through `record_overlay`, and adds a library-level staging table
for definitions applied nowhere plus an assertion outbox for writes against
remote-owned sources.

There is no data migration. The legacy tables are empty (see Current residue),
so this slice is a clean build. Its exit proof for the drop: no production
code reads or writes the legacy tag tables outside the row-sync registrations
that FD3 removes.

Scalar fields such as favorite, rating, and corrected title use
`record_overlay`. Do not put enumerable tags in its JSON fields.

## FD3: Remove legacy row sync

> Done 2026-09-18. `entry`, `content_identity`, `sidecar`, `mime_type`, the
> three media_data models, `collection`, `collection_entry`, `location`,
> `tag`, `tag_relationship`, `user_metadata`, and `user_metadata_tag` lost
> their registrations and `Syncable` impls. Entry's `apply_state_change`,
> tombstone gating, closure and directory-path rebuilds went with them, as
> did backfill's entry hierarchy sort and per-record self-referential FK
> resolution, and the sync helpers' directory-path payload materialization.
> The generic engine, transport, HLC, watermarks, and dependency tracking
> stay for the six survivors, matching the library-sync research's plan to
> evolve rather than replace the framework. Retired-model test fixtures were
> repointed at survivors, and the registry test asserts the retired names
> never register again. Capability audit on the live fleet: replica sources
> listed, remote ops answered by titan, replica listings served, remote job
> reporting responsive.


1. Unregister `entry`, old content/media rows, sidecars, and entry-bound
   junctions from the generic row-sync registry once their durable replacements
   are live.
2. Remove entry hierarchy sorting, FK mappings, closure rebuilds, directory-path
   payloads, dependency cases, checkpoints, watermarks, and tombstones that only
   serve those models.
3. Delete sync tests whose subject is the retired shared entry tree. Repoint
   transport, HLC, assertion-merge, and source-replica tests at their surviving
   contracts.
4. Audit `service/sync`, `infra/sync`, and library startup by capability. Code
   may be removed only after proving it does not serve:
   - source-store assertion merge;
   - peer source discovery or snapshot refresh;
   - remote byte reads;
   - remote operations, jobs, or logs.

The exit proof is behavioral. A smaller directory tree is not proof that the
correct sync path survived.

## FD4: Replace the library schema

> Done 2026-09-18. `m20260918_000001_drop_entries_world` drops 29 tables in
> one transaction, children before parents: with foreign keys enforced,
> dropping a parent makes SQLite parse every child's constraints, and the
> first attempt failed on `sidecar`, whose other parent `entries` was
> already gone. That attempt also proved `Migrator::up` does not wrap SQLite
> migrations in transactions, so the migration opens its own. Dropped: the
> entry hierarchy, content identities and kinds, mime types, media data,
> sidecars, collections, locations, every semantic tag and user-metadata
> table, conduits and their generations, search analytics, indexer rules,
> and the FTS `search_index` family. Location Space items and Locations
> groups are deleted rather than migrated. A library now holds 14 tables:
> `assertion_outbox`, `audit_log`, `cloud_credentials`,
> `device_state_tombstones`, `devices`, `seaql_migrations`, `sources`,
> `space_groups`, `space_items`, `spaces`, `sqlite_sequence`,
> `sync_checkpoints`, `tag_staging`, `volumes`.
>
> Proof: the statement sequence replayed on a copy of the live library with
> foreign keys on (clean `foreign_key_check`, integrity ok, Space items
> intact); a library created by the pre-drop build, left half-migrated by the
> failed first attempt, completed on restart; a brand-new library converges
> on the same schema; the live Mac library upgraded after a backup to
> `~/.spacedrive/backup/pre-entries-drop/library.db`, where every dropped
> table held zero rows apart from the 25-row seeded kind lookup. Tags,
> tag-filtered search, sources, replicas, and remote ops verified after.
>
> Locations left the product with the schema: the ops, domain type, entity,
> CLI domain, and every client consumer. Their callers moved to sources and
> Space items: PathBar pins are space-level Path items shown under Pinned,
> the sidebar's Places come from the new `paths.system_folders` query, Add
> Storage tracks sources and validates through `sources.validate_path`, the
> device views list sources, and library statistics count sources as
> `source_count`. Historical migrations are not squashed; the upgrade
> boundary stays the full chain.
>
> Known gap, pre-existing: store `content.kind` is never written by the
> hashing pass, so kind statistics read zero, as the retired table did.


1. Define the final `library.db` baseline from the entities that still have a
   current owner. Do not copy the old migration list and subtract names by
   intuition.
2. Provide an upgrade migration that drops the retired tables directly. There
   is no assertion transfer; the legacy tag tables are empty, and titan's
   library gets the same emptiness check before its tables drop. Make the
   migration transactional where SQLite allows it.
3. Drop the entry hierarchy, location tables, old content/media/sidecar tables,
   entry-bound collections and conduits, old tag applications, FTS tables,
   triggers, and indexes after their production callers and sync registrations
   are gone. Locations need no migration or compatibility path.
4. Keep the source registry, volume and device facts, jobs, settings, Space
   items, and current library-level UI objects. New navigation uses Space items;
   new processing intent uses policy-owned source-relative targets.
5. Create a fresh library from the new baseline and upgrade the legacy fixture.
   Refuse an incompatible database with a precise recovery message. Never
   silently create a second library or discard source stores.

Squash historical migrations only after the upgrade fixture has passed and the
supported upgrade boundary is explicit. A clean baseline is not permission to
forget durable assertions.

## FD5: Retire the compatibility surface

> Done 2026-09-18. Fifteen entry-era integration tests and `indexing_demo`
> are deleted. The rest track a source or browse the way production does,
> through two harness steps: `track()`, which waits on the walk tracking
> dispatched, and `index_dir()`, which walks the volume's spelling of a path.
> Reviving them exposed decay unrelated to the drop, all fixed: the harness
> kept the core's data directory above its test files, so the watcher ignored
> every change as the daemon's own data; hand-built walks indexed `/Users`
> while search scoped to `/System/Volumes/Data/Users`; and the TypeScript
> bridge suites wrapped `createSubscription` with a stale signature and
> guessed the device slug from the hostname. A product bug surfaced as well:
> a browse of an empty folder ignored the first file dropped into it.
>
> Two raw-SQL readers of dropped tables survived FD4 because the compiler
> cannot see a table name in a string: the `content_kinds` statistics refresh,
> now removed, and `redundancy.summary`, which now says redundancy is not
> computed over source stores instead of failing or reporting zeros.
> `sd-bench` had not built since FD4; its scenarios track sources, and the
> content scenario times the identification pass tracking queues.
>
> Naming: `ops/indexing/ephemeral` folds into `ops/indexing`.
> `EphemeralIndexCache` is `VolumeIndex`, a drive's slot is `Partition`,
> `EphemeralIndex` is `Arena`, `IndexerJobConfig::ephemeral_browse` is `new`,
> and `core.ephemeral_status`/`core.ephemeral_reset` are
> `core.index_status`/`core.index_reset`. The on-disk `ephemeral.snapshot`
> keeps its name, since renaming it orphans every snapshot and re-walks every
> drive. Surface: `IndexType`, the location filter and facet, fifteen events
> nothing emitted, the legacy status fields, `LocationNotFound`, the stubbed
> pre-HLC transaction methods, and indexer state kept for promoting a browsed
> folder into a location are gone.
>
> Known failing, not caused by the drop: `watcher_test` and the TypeScript
> move and rename bridge suites (unpaired FSEvents renames, diagnosed in the
> pre-teardown brief); `sync_backfill_test`'s volume cases (two cores on one
> host collide on `devices.slug`); `sync_metrics_test::test_metrics_initial_state`
> (startup backfill exchanges devices and default spaces). Follow-ups:
> `indexing.start` walks the caller's spelling of a path without an arena
> handle, so CLI `index` walks do not reach search; redundancy needs rebuilding
> over source stores, and its search filters are accepted but not applied.

1. Remove or repoint entry-era integration tests, helpers, and examples.
2. Rename the `ephemeral` index module and Rust types around their actual role as
   the volume index. Keep in-memory arena names distinct from durable registry
   and source-store names.
3. Rename `core.ephemeral_status` and `core.ephemeral_reset` once, regenerate
   TypeScript types, and update all clients in the same change.
4. Remove compatibility fields and comments that describe persistent versus
   ephemeral indexing.
5. Update `AGENTS.md`, `PROJECT_STATUS.md`, and the convergence documents to say
   that the entries world no longer exists.

The naming pass follows the physical drop. Until then, `ephemeral` remains a
useful marker for references that have not crossed the boundary.

## Acceptance

### Static boundary

These searches return no production references. Historical plans may retain the
word when clearly marked as history.

```bash
rg 'entities::entry|entry_closure::Entity|directory_paths::Entity' core/src apps crates
rg 'register_syncable.*"entry"|model_type == "entry"' core/src
rg 'core\.ephemeral_(status|reset)' core apps packages
```

A fresh `library.db` has no retired tables, indexes, FTS tables, or triggers.

### Data safety

- Tag definitions, applications, removals, HLCs, and device identities live in
  source stores under the new model, proven by the tags plan's acceptance.
- Reindexing or evicting a source leaves its assertion tables unchanged.
- A source store moved to a clean library remains self-describing.
- Frozen copies are byte-for-byte untouched. Live source stores change only
  through explicit, tested source-store schema migrations.

### Product behavior

- Browse, search, collections, media view, tags, spaces, copy, move, delete,
  restart, and watcher changes work on local sources.
- Saved navigation survives a volume remount. Adding or removing a bookmark
  neither indexes nor deletes records and does not change processing policies.
  Verify this through Space items and the new target resolver. No location
  compatibility fixture is required.
- The Mac can list titan's sources, browse replicas, stream file bytes, dispatch
  remote operations, watch remote jobs, and follow remote logs.
- A daemon with an old library either upgrades successfully or stops with the
  documented recovery path.

### Build and tests

```bash
cargo fmt --check
cargo check
cargo check -p sd-core --all-targets --features wasm
cargo check -p sd-native
cargo test -p sd-store
```

Run focused assertion-transfer, tag, source-store, peer-mount, remote-operation,
Space item and policy-target tests in addition to the repository baseline.

## Handoff

Start with FD0, then FD1. FD2 has its own plan and can run in parallel. FD3
and FD4 still follow it, because the new tag system must exist before the old
tables and registrations go. FD5 is mechanical only after the schema is
physically gone.

Update this file and `PROJECT_STATUS.md` together whenever a slice starts,
lands, becomes blocked, or changes owner.
