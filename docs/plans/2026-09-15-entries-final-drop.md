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

Tag definitions and applications still live in the library database. Tag
applications use `user_metadata.entry_uuid` or
`user_metadata.content_identity_uuid`, and tag operations still write through
`UserMetadataManager`.

`record_overlay` is ready for scalar assertions: it carries record identity,
portable rebind evidence, HLC, and device identity. The source store does not
yet have the `tag_definition` and `tag_assertion` tables specified by
`docs/core/design/tags-and-assertions.md`. Those tables and a verified migration
must land before the legacy tag tables can be removed.

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
| FD0 Baseline and ownership | ready | unowned | Current references and tables recorded; fixtures and build baseline pass |
| FD1 Remove runtime entry reads | ready | unowned | No production query or helper reads `entries`, `entry_closure`, or `directory_paths` |
| FD2 Move tags and assertions | ready after FD0 | unowned | Source stores are self-describing; legacy tag fixture migrates without loss |
| FD3 Remove legacy row sync | blocked on FD2 | unowned | No entry model registration or entry-specific replication remains; peer source capabilities pass |
| FD4 Replace the library schema | blocked on FD2 and FD3 | unowned | Fresh and upgraded libraries contain no retired tables |
| FD5 Retire compatibility surface | blocked on FD4 | unowned | Tests, examples, generated types, docs, and names describe one index model |
| FDA Acceptance | blocked on FD1-FD5 | unowned | Full matrix below passes |

## FD0: Baseline and ownership

1. Re-run the reference inventory. Classify every hit as production,
   migration, test, example, generated client, or historical documentation.
2. Record the current tables in a fresh library and in a fixture created by the
   last pre-drop build.
3. Create a legacy fixture with:
   - a file-scoped tag;
   - a content-scoped tag with two copies;
   - a tag definition applied nowhere;
   - an unapplied tag whose removal must not resurrect;
   - a Space item using the current navigation contract;
   - two paired devices with source replicas.
4. Run the repository baseline before edits. Keep failures that predate the
   slice separate from regressions.

Do not start with a schema deletion. The fixture is the proof that makes the
later deletion safe.

## FD1: Remove runtime entry reads

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

## FD2: Move tags and assertions

1. Add `tag_definition` and `tag_assertion` to the assertion half of every
   source store, using the schema and merge rules in
   `docs/core/design/tags-and-assertions.md`.
2. Keep both record UUID and convergent content UUID on an assertion. Carry the
   source-relative external ID as rebind evidence. Order apply and remove rows
   by HLC with device UUID as the tiebreak.
3. Rewrite tag apply, unapply, delete, and listing operations against source
   stores. Applying a definition copies it into every source receiving an
   assertion.
4. Keep unapplied definitions in a small library-level staging table until a
   source adopts them. Keep only client-local pinning, ordering, and color
   overrides beside it.
5. Migrate legacy rows idempotently:
   - find record-scoped targets by record UUID across source stores;
   - find content-scoped targets by convergent content UUID;
   - copy each used definition into every target source;
   - write assertions with stable HLC and device identity;
   - retain unresolved rows and report them instead of dropping them.
6. Compare definitions, applications, removals, and unresolved rows before and
   after migration. Only a zero-loss report unlocks FD4.

Scalar fields such as favorite, rating, and corrected title use
`record_overlay`. Do not put enumerable tags in its JSON fields.

## FD3: Remove legacy row sync

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

1. Define the final `library.db` baseline from the entities that still have a
   current owner. Do not copy the old migration list and subtract names by
   intuition.
2. Provide an upgrade migration that runs FD2's assertion transfer before it
   drops any source data. Make the migration transactional where SQLite allows
   it and resumable where it crosses source-store files.
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

- The legacy fixture preserves tag definitions, applications, removals, HLCs,
  device identities, and unresolved-row diagnostics.
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

Start with FD0, then FD1. FD2 is the durable-data gate. FD3 and FD4 must not run
ahead of it. FD5 is mechanical only after the schema is physically gone.

Update this file and `PROJECT_STATUS.md` together whenever a slice starts,
lands, becomes blocked, or changes owner.
