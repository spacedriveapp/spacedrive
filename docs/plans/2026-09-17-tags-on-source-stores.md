# Tags on Source Stores

> Status: complete. Landed, delivery verified live against titan, P6 done,
> search phase 4 riding on it. Remaining tag work belongs to later stretches.
> Captured: 2026-09-17
> Executed: 2026-09-17 (Fable), results below
> Owns: FD2 in `2026-09-15-entries-final-drop.md`
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/design/tags-and-assertions.md` (the model),
> `2026-09-15-entries-final-drop.md` (the drop this feeds),
> `2026-09-15-library-sync-research.md` (the boundary this stays out of),
> `2026-09-15-search-refinement.md` (phase 4 lands on these reads)

## Execution record (2026-09-17)

All phases through P5 are implemented and live-verified on the Mac daemon;
nothing is committed yet.

- P1: both tables in `RECORD_SCHEMA`, the store API in
  `crates/store/src/tags.rs`, and eight tests in
  `crates/store/tests/tags.rs` covering removal-wins ordering, duplicate
  delivery, content collapse, definition LWW, late binding, and
  generation-loss rebind. `entry_by_uuid` already existed in `read.rs` and
  is reused. `unicode-normalization` added for the slug fold.
- P2: the nine ops rewritten; `TagManager`, `TagClosureService`,
  `TagConflictResolver`, `TagUsageAnalyzer`, `TaggingFacade`,
  `UserMetadataManager`, `ops/metadata`, `domain/user_metadata`, and the
  validation module deleted. `tag_staging` migration and entity added. The
  `library.id()` device placeholder is gone; stamps come from a process
  clock in `ops/tags/stamp.rs` seeded by `DeviceManager`, and the hashing
  commit path late-binds content keys.
- P3: decoration in directory listings (arena and store paths), `files.by_id`,
  `files.by_path`, and replica listings. `files.by_tag` and target
  resolution serve from stores directly, so a cold daemon with no loaded
  arena answers correctly; this was caught live when a restart returned
  empty and is a regression test in spirit: the store is the floor.
- P4: `assertion_outbox` migration and entity, `sources.assertions.merge`
  registered, drain worker started from core init (device-connected events
  plus a 120 second tick, 30 second delivery timeout, backoff to 30
  minutes). Verified live against titan: a replica-targeted apply reports
  `1 target(s) pending delivery`, the row waits with recorded attempts, and
  the pending overlay shows the tag on the replica listing immediately.
  The ack-and-retire half needs titan running this build; its old daemon
  cannot answer the merge method. Run that proof at the next deploy.
- P5: types regenerated, `TagSelector` rewritten against generated types,
  `canonical_name` renamed to `name` across views, apply and unapply
  payloads slimmed, CLI gains unapply and delete and prints pending counts.
  Desktop production build passes; scoped typecheck clean.
- P5 follow-ups from James's live testing: Tag Mode's legacy content-identity
  gate (the "need to be indexed" toast) is gone and the palette is a real
  toggle over File targets; the inspector's `isEphemeral` gate on the Tags
  section is gone; `File::announce` now decorates its payload, which is what
  made tagging non-reactive: the event fired but carried empty tags, and the
  client's by-id merge (arrays replace) faithfully rendered that emptiness.
  The remaining honest refusal is a path outside every tracked source, which
  the UI words as an Add to Library prompt.
- Verified live end to end on the Mac: create, apply, adoption out of
  staging, decoration everywhere, unapply, removal-wins, restart
  durability, and direct SQL confirming definitions and HLC-stamped
  assertions in the `jamespine` store.

Stretch 1 completed 2026-09-18 (Fable):

- The delivery roundtrip is proven on real hardware. Two caller-side bugs
  surfaced and were fixed: the outbox named the merge by its bare name while
  the registry keys actions by their full wire method (now taken from the
  Wire const), and backoff could outvote presence, so a device connecting now
  forces delivery of its rows through `drain_for`. Ten batches authored
  against titan while its old server was wedged, nine of them from James's
  UI testing, delivered on reconnect, acked, and retired; titan's stores
  answer `files.by_tag` with the tagged files and `tags.search` names the
  definitions.
- Titan was redeployed per the runbook. Its `sd-server` had made no library
  writes since 2026-09-16 05:21 and was replaced by this build's `sd-daemon`
  (binaries rotated to `.prev`); the web server relaunch stays with James
  and now needs both `DATA_DIR=/mnt/pool/jamie-nas/spacedrive/data` and his
  `SD_AUTH`, after which it attaches to the running daemon.
- FD4's preconditions are banked: both libraries' legacy tag tables are
  verified empty, titan's checked read-only over SSH.
- P6 is done: `tagging.mdx` rewritten to the built system, `library.mdx`
  corrected, the design doc carries its reconciliations, the library tag
  count reads definitions instead of the legacy table, and the rg proofs
  leave only the FD3/FD4 residue (entities, migrations, row-sync
  registrations, and `route_from_dependency`).
- Search phase 4 landed on these primitives; results in the search plan.

## Outcome

A tag is a source primitive. Definitions and assertions live in each
`source.db`, so a store presents named, colored, hierarchical tags to any
library that mounts it or receives it as a replica. Favorite and notes scalars
move to `record_overlay`. Tagging a file on a remote-owned source works while
the owner is offline, through a durable assertion outbox that delivers when the
owner connects. The legacy tag tables in `library.db` end this plan with no
production callers, which is what FD3 needs to remove their sync registrations
and FD4 needs to drop their schema.

## Decisions recorded here

1. **No legacy migration.** This release is not backward compatible and there
   is no tag data to move. Verified read-only on 2026-09-16: the Mac library
   has zero rows in `tag`, `tag_relationship`, `tag_closure`,
   `tag_usage_pattern`, `user_metadata`, and `user_metadata_tag`. Titan holds
   its own library and was unreachable during the check; run
   `sd-cli --device titan tag search ""` before FD4 drops schema. FD2 loses its
   migration steps and zero-loss report, FD0's fixture loses its tag items, and
   FD4 loses the transfer-before-drop clause. Those edits land with this plan.
2. **A tag write is a row, and delivery is merge.** An assertion is
   append-only, HLC-stamped, and device-attributed, so a device can author the
   finished row locally and deliver it later. Merge on the owner is a set union
   with latest-HLC state, which makes duplicate delivery harmless. Reachability
   changes latency. It never changes behavior.
3. **This is source sync. Library sync stays deferred.** No membership, no
   library-state replication, no gossip. Delivery is point to point, author to
   owner, over the existing paired-device remote operation path. The scope in
   `2026-09-15-library-sync-research.md` is untouched.
4. **Schema conventions.** Assertions carry `content_uuid BLOB`, following
   `record_overlay`, where the design doc wrote `content_id TEXT`.
   `record_uuid` is NOT NULL: an application names a record always, and
   content-keyed-only applications stay an open design question. Definition
   merge is row-level last-writer-wins on `updated_hlc`; the design doc's
   reconciliation table says per field while its schema carries one clock, and
   the schema wins. The slug folds case and normalizes to NFC so `work` and
   `Work` converge; display keeps the author's casing. P6 reconciles the design
   doc to these points.
5. **Same-slug, different-uuid definitions park side by side.** The genuine
   merge stays a user decision per the design doc. It never blocks delivery.
6. **Device identity comes from `DeviceManager`.** Today's `tags.apply` uses
   `library.id()` as the device id. That placeholder dies here; every assertion
   and HLC carries the real device uuid.
7. **The outbox lives in `library.db`.** Unshipped claims are device-local
   state. A replica file is a blake3-verified artifact and is never written
   locally, so pending rows live beside it, never inside it.

## Boundaries

- Assertion tables are the irreplaceable half of a store. Walks, sweeps,
  eviction, refetch, and any future compaction leave them alone. Tests prove
  this, and the sweep fuse never counts assertion rows.
- Replica databases stay byte-identical to what the owner shipped, per the
  generation and blake3 contract from R3/R6.
- Frozen copies under `freezes/` are never opened by any code path.
- No new callers of the legacy tag tables. Their entity modules stay compiled
  until FD3 unregisters the row-sync models (`tag`, `tag_relationship`,
  `user_metadata`, `user_metadata_tag`) and FD4 drops the schema.
- `catalog.db` is not built here. Cross-source tag questions stay per-query
  fan-outs over open stores.
- `SdPath` gains no source variant.

## Schema

### source.db additions

Appended to `RECORD_SCHEMA` in `crates/store/src/record.rs`. Both are
`CREATE TABLE IF NOT EXISTS`, so existing stores gain empty tables on next
open and no row is touched.

```sql
CREATE TABLE IF NOT EXISTS tag_definition (
    uuid BLOB PRIMARY KEY,
    slug_id BLOB NOT NULL,
    path TEXT NOT NULL,
    color TEXT,
    icon TEXT,
    updated_hlc TEXT NOT NULL,
    origin_device BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tag_definition_slug ON tag_definition(slug_id);

CREATE TABLE IF NOT EXISTS tag_assertion (
    tag_uuid BLOB NOT NULL,
    record_uuid BLOB NOT NULL,
    external_id TEXT,
    content_uuid BLOB,
    asserted INTEGER NOT NULL,
    hlc TEXT NOT NULL,
    device_uuid BLOB NOT NULL,
    PRIMARY KEY (tag_uuid, record_uuid, hlc, device_uuid)
);
CREATE INDEX IF NOT EXISTS idx_tag_assertion_record ON tag_assertion(record_uuid);
CREATE INDEX IF NOT EXISTS idx_tag_assertion_content ON tag_assertion(content_uuid);
```

Notes:

- No foreign key to `record`, matching `record_overlay`. Assertions outlive
  any generation row and survive a rebuild.
- The assertion primary key doubles as the merge dedupe key. Delivery inserts
  with `ON CONFLICT DO NOTHING`, so replaying a batch changes nothing.
- `slug_id` is `uuidv5(TAG_NS, normalized_path)`. `TAG_NS` is a fixed constant
  in `crates/store` and never changes. The slug is a merge key consulted when a
  definition arrives from elsewhere. It is never a join key.
- `external_id` carries the source-relative path as rebind evidence, and
  `rebind_assertions` mirrors `rebind_overlays`.
- The state of a tag on a record is the latest `hlc` row for
  `(tag_uuid, record_uuid)`. Removal appends `asserted = 0`. Content collapse
  unions rows sharing `content_uuid` inside a store; the cross-store union
  happens at query time until `catalog.db` exists.

### library.db additions

- `tag_staging`: definitions applied nowhere yet, with the same columns as
  `tag_definition`. This is the one denormalization the design accepts. A
  source adopts the definition on first assertion, and adoption deletes the
  staging row.
- `assertion_outbox`: `id`, `source_uuid`, `kind` (`tag` or `overlay`),
  `payload` (JSON row image; tag rows embed the definition snapshot they
  depend on), `hlc`, `created_at`, `attempts`, `next_attempt_at`,
  `last_error`. Rows retire on the owner's ack. One outbox serves tag
  assertions and `record_overlay` writes, because both carry the same HLC and
  device shape.

Sidebar pinning, ordering, and color overrides get no table until a UI surface
needs them.

## Phases

### P1: store substrate

1. Add both tables to `RECORD_SCHEMA` with the notes above.
2. Store API in `crates/store`: `upsert_definitions` (row-level LWW on
   `updated_hlc`), `append_assertions` (batched, idempotent),
   `tags_for_records` (batched decoration read with content collapse),
   `records_for_tag`, `list_definitions`, `rebind_assertions`.
3. Late-binding hook: a content-identification commit back-fills
   `content_uuid` on assertions whose record just gained a hash.
4. Tests: removal wins across out-of-order inserts, duplicate append is a
   no-op, content collapse across two stores in one process, definition LWW,
   adoption copy, rebind, and a walk plus sweep that leaves assertion rows
   untouched.

Proof: `cargo test -p sd-store` passes with the new suite, and opening an
existing store adds empty tables without touching record rows.

### P2: write path

1. Plumb one write context for tags: an HLC from `infra/sync/hlc.rs` and the
   device uuid from `DeviceManager`, fetched once per action.
2. `tags.create` validates and normalizes the path, computes the slug, and
   writes to `tag_staging`.
3. `tags.apply` keeps `TagTargets` as Content or File, resolves targets to
   `(source, record_uuid, external_id, content_uuid)` groups, adopts the
   definition into each receiving store, and appends assertions. Local sources
   write directly. Remote-owned sources enqueue to the outbox through the
   interface P4 implements. Content targets with a local copy write to the
   local store that holds the copy and need no owner at all.
4. `tags.unapply` appends `asserted = 0` through the same paths.
5. `tags.delete` v1 is conservative: it removes the definition from staging
   and from stores it can write now, appending `asserted = 0` for live
   applications there. If an affected source is unreachable, the action
   refuses and names it. Tombstones stay open below.
6. `tags.search`, `tags.by_id`, `tags.ancestors`, `tags.children` read
   definitions across open stores plus staging, dedupe by uuid preferring the
   latest `updated_hlc`, and derive hierarchy from paths. The closure service
   dies.
7. Favorite and notes writes go through `SourceDb::set_overlay`.
   `UserMetadataManager` and `ops/metadata` are deleted. `TagManager`,
   `TagContextResolver`, `TagUsageAnalyzer`, `TagClosureService`, and
   `TagConflictResolver` are deleted. Validation slims to path, name, and
   color checks.
8. Keep the event contract from `0f5e4a1ef`: content-scoped changes emit for
   every copy. Events fire on local state change, committed or pending.

Proof: focused tests pass; on a fresh daemon, create, apply to two local
sources, unapply, restart, and the state holds; direct SQL shows the store
carrying the named, colored definition beside its assertions.

### P3: read path

1. Decorate `File.tags` in `directory_listing`, `file_by_path`, and
   `file_by_id` from store assertion state through one batched
   `tags_for_records` call per source per page. Assertions live only in
   SQLite; the arena never carries them, and the R6 single-backend rule
   governs record candidates only.
2. `files_by_tag` fans out across open stores, including replicas, with
   content collapse per store and union at the edge.
3. Replica sources render the owner's tags from the delivered database as of
   its generation, plus local pending rows once P4 lands.
4. Favorite and notes decoration reads `record_overlay` through the same
   handles.

Proof: a tagged file shows its badge in listing and `files_by_tag` across two
local sources; a replica delivered after an owner-side apply shows the tag.

### P4: outbox and delivery

1. `assertion_outbox` table plus a drain worker keyed by target device. It
   drains on enqueue, on device-connected events, and on a backoff timer,
   reusing the replica-fetch backoff discipline.
2. Owner-side merge is a registered library action, `sources.assertions.merge`,
   invoked through the paired-device remote operation path. Input: source
   uuid, definitions, assertions. The owner verifies it owns the source,
   upserts definitions, appends assertions, commits, then acks. Commit before
   ack is the durability barrier.
3. Ack deletes the outbox rows. A lost ack causes redelivery, which the
   assertion primary key absorbs.
4. After merge, the owner marks the source dirty so paced replica refresh
   propagates the new state to peers.
5. Reads for a remote-owned source union pending outbox rows into tag state,
   so the UI reflects the claim immediately and honestly: the row exists and
   is authored, and only delivery is pending.
6. `tags.apply` output reports per-target `committed` or `pending`.
7. `record_overlay` writes against remote-owned sources ride the same outbox
   with `kind = overlay`.

Proof: with titan disconnected, tag a replica file; the outbox holds the row
and the UI shows the tag. Reconnect titan; the row delivers, acks, and
retires, and direct SQL on titan shows the assertion. Redelivering the same
batch changes nothing. The next replica delivery shows the tag from the store
itself.

### P5: clients

1. Regenerate `ts-client` after the type changes. The `Tag` type slims to
   uuid, path, leaf name, color, icon.
2. Update `TagSelector`, `Tags/index`, `TagAssignmentMode`, the inspector
   variants, `useFileContextMenu`, and `useRefetchTagQueries` against the
   generated types. Surface pending state where apply output reports it.
3. Update `sd-cli tag` create, apply, and search against the new ops, and fix
   help text that still says entries.

Proof: desktop production build passes; tag flows work in the live app; CLI
create, apply, and search verified against a running daemon.

### P6: legacy caller removal and docs

1. Prove no production reads or writes remain against the legacy tables:

   ```bash
   rg 'UserMetadataManager' core/src
   rg 'user_metadata|user_metadata_tag' core/src --glob '!infra/db/entities/*' --glob '!infra/migration/*'
   rg 'entities::(tag|tag_relationship|tag_closure|tag_usage_pattern)\b' core/src --glob '!infra/sync/*'
   ```

   Remaining hits must be the entity modules, the migration chain, and the
   row-sync registrations that FD3 removes.
2. Reconcile `docs/core/design/tags-and-assertions.md` to decision 4 and mark
   its consequences section executed.
3. Sweep `/docs/core` pages that describe tag storage in the library database.
4. Update the FD plan register row and `PROJECT_STATUS.md` as each phase
   lands, in the same commit.

Proof: the rg checks return only the permitted hits; the docs build passes.

## Acceptance

- Local lifecycle: create, apply, unapply, delete, restart, and the state
  holds from store rows alone.
- Two-store content collapse: tag the bytes once, both copies show it, and
  `files_by_tag` returns both.
- Reindex and sweep leave assertion tables byte-identical.
- Replica display: titan's tags render on the Mac from the delivered database.
- Offline roundtrip: the P4 proof, run live against titan while the NAS is
  still up.
- Out-of-order merge: a removal authored later but delivered earlier still
  wins once the apply arrives.
- Checks: `cargo fmt --check`, `cargo check`, `cargo test -p sd-store`,
  focused core tag tests, `cargo run --bin generate_typescript_types`, desktop
  production build.

## Out of scope

- `catalog.db` and the global tag projection.
- Library sync: membership, bilateral device rows, library-state replication.
- Search refinement phase 4. It intersects `records_for_tag` and stays owned
  by the search plan.
- Sidebar tag pinning, ordering, and color overrides.
- Finder tag and xattr import.
- A merge UX for parked same-slug definitions.

## Open

- Definition tombstones across detached sources. V1 accepts that a returning
  store can resurrect a deleted definition through the adoption path, and
  `tags.delete` stays conservative because of it. Revisit with assertion sync
  hardening.
- Content-keyed-only applications, unchanged from the design doc.
- Whether spaces follow the same model, unchanged from the design doc.
