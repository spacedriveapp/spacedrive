# Entries Teardown — Execution

> Companion to `2026-08-20-entries-teardown.md`. Part 1 is the plan's claims
> checked against the tree at `495011f07`. Part 2 is the ordered task register
> built from what's actually there. The plan stays the statement of intent;
> this is the thing you hand to agents.
>
> **Related docs.** `2026-08-22-source-convergence.md` runs ahead of this
> register: it inserts a truth pass and a re-charter of `crates/archive` before
> P2 here, and pulls T6.1 forward. Read it first — it changes the ordering
> below, not the content. `2026-08-20-architecture-previs.md` is the destination
> written as though finished, and carries the decisions and open questions.
> `2026-08-20-pre-teardown-brief.md` is the one-pager for work landing next to
> the teardown before it starts. `2026-08-21-filesystem-source-store.md` designs
> T2.1/T2.2/T2.5 — how a filesystem source gets a `source.db` and what flows
> through it. The mounts lineage is separate: `docs/core/design/mounts.md` and
> `2026-08-20-byte-plane-and-block-cache.md`.
>
> **Current handoff.** `2026-09-15-entries-final-drop.md` owns the remaining
> execution. This document keeps the original audit and task history. Do not
> execute P6's path-based deletion list against the current tree: source and
> peer replication landed after it was written. Locations are removed without
> backwards compatibility; the agreed policy-addressing follow-up is recorded
> in `2026-09-15-locations-research.md`.

## Status

This table records the original sequence. Update it when old task status
changes, and update `2026-09-15-entries-final-drop.md` for the remaining work.
The `.tasks/` tree is stale (May, describing the entries world this document
deletes) and is not being revived.

More than one session works this tree at a time. Put your session's name in
**Owner** before starting a task, so a concurrent agent can see what is live
rather than inferring it from a diff.

| Task | Status | Owner | Note |
|---|---|---|---|
| T0.0 Reclaim disk | done | — | `target/debug/incremental` removed, 22 GiB freed |
| T1.1 Identity-correct removal | done | — | recursive `detach`, arena `vacate`, `NameRegistry::remove` |
| T1.2 Duplicate adds update in place | done | — | the one that unblocked both caches |
| T1.3 Snapshot writes on any change | done | — | dirty flag replaces the entry-count gate |
| T1.4 Fingerprint-verified identity | done | — | absorbed by the volume-keyed index and the source registry's nullable `volume_uuid` |
| T1.5 Registry writes are fallible | done | — | plus explicit `SourceRegistry::in_memory` |
| T1.6 Snapshot id guard | done | — | |
| T1.A Acceptance | done | — | 5 new tests; 343 unit tests green |
| T2.0 Store shape | **decided** | — | one file per source, evidence-tuple rebind. WAL settles it |
| — index to keep | **decided** | — | a source store is never rebuilt; it is user data, not a cache |
| T2.0b Volume/source boundary | done | — | convergence P2.7/P3: a volume owns the index; a source is a retained scope with nullable `volume_uuid` |
| — source store design | **decided** | — | `2026-08-21-filesystem-source-store.md`: shared store, forked ingest, ledger folds into the record table |
| T2.1–T2.6 | core cutover landed; remainder split | — | `crates/store`, filesystem ingest, durable identity, record shape, and addressing are live; tag assertions moved to FD2 and source-scoped sidecars follow the product sequence |
| T2.7 Content identity | done | — | absorbed by convergence P1.7: `content.uuid`, `sampled_hash` UNIQUE, upsert |
| T3.1a Format v2 | done | — | `pvcache/layout.rs:12` `FORMAT_VERSION = 2`, four header fields on the frame |
| T3.1b Native reader | done | — | `native/src/source/pvcache.rs:325` builds at the content extent |
| T3.1c Producers return true extent | done | — | `ScaleMode::Fit` in `bake/src/raster.rs:48` |
| T3.1d Producer in the daemon | landed | — | `core/src/service/thumbs/service.rs` — `BakePool` + `dirs.thumbs_file(source_id)`; not exercised end to end here |
| T3.1e `/hot-thumb` route | done | — | `apps/tauri/src-tauri/src/server.rs` serves versioned pvcache tiles with immutable caching |
| T3.1f `Thumb.tsx` prefers hot tier | **implemented, uncommitted; live recheck pending** | current session | hot-tier completion epochs survive virtualized cell unmounts; current-source load and error state cannot hide the fallback permanently |
| T4.1–T4.9 | primary cutover landed | — | writes, file reads, locations, tags, and the unreachable media pipeline moved or were removed; runtime residue and final tag assertions are FD1/FD2 |
| T4.4 Enrichment policy follow-up | addressing, compatibility and setup UX agreed; implementation pending | Codex research | Accept `SdPath`, persist a separate source-relative policy target, and resolve execution addresses through the source layer. No source variant in `SdPath`. Space items own navigation. Remove locations without row migration, ID preservation or API compatibility. See `2026-09-15-locations-research.md`. Reuse source change delivery and the job executor. `2026-09-16-add-to-library.md` defines the shared setup modal, containing-volume tracking, configurable defaults and library/on-source store placement; processing can be configured in that flow. Its September 16 implementation audit identifies the common resolver, portable identity, managed-directory exclusion and catalog-preserving removal work. |
| T4.10 Global search router | done | — | `1044e0977` searches every local and paired source |
| T5.1–T5.3 | superseded | — | content hashing writes source stores; the old media pipeline was deleted; current source-scoped media work follows the product sequence |
| T6.1–T6.7 | done through FD5; FDA ready | Opus | executed as `2026-09-15-entries-final-drop.md` FD0–FD5; its acceptance matrix (FDA) is next |
| P7 Cross-source queries | compare and dedupe landed; at-risk next | — | no `catalog.db`: asked of the source stores at query time. Search, alternates and content tags already read every store; `paths.compare` compares two folders by path or content, `files.delete` takes a comparison as its targets, removing from A what the set names after reading both copies in full, its validation counts across every store which files are the last copy of their content, and dedupe removes surplus copies within a source or of chosen files anywhere, reading each pair in full first |

**Not in this register, tracked elsewhere:** the byte plane, block cache and
mount frontends, in `2026-08-20-byte-plane-and-block-cache.md` and the
implementation status in `docs/core/design/mounts.md`. All four byte-plane
phases and the SMB frontend have landed (`core/src/service/mounts/` now
carries `provider.rs`, `cache.rs` and `smb.rs`). It sits on the sources path
and appears nowhere in the teardown's move or delete lists.

**Known failing before you start:** `ephemeral_watcher_test` fails on a clean
tree — a rename leaves the index and does not return. Pre-existing, diagnosed in
the pre-teardown brief, not owned by any task here.

## Baseline

```
cargo check                                          # default-members
cargo check -p sd-core --all-targets --features wasm # tests and examples
cargo check -p sd-native                             # gpui tree, not a default member
```

`--features wasm` is not optional on the second: `wasm_job_execution_test` reads
`Core::plugin_manager`, which is `#[cfg(feature = "wasm")]`. Without it that one
test fails to compile and reads as a regression.

Do not use `--workspace --all-targets` — it pulls
`apps/tauri/crates/file-opening-windows` into the graph, which does not compile
on macOS. Pre-existing and unrelated.

If a build dies with `No space left on device`, `target/debug/incremental` is
what to remove: it recovers ~22 GB, keeps the dependency artifacts in
`target/debug/deps`, and costs one cold rebuild of the workspace crates. It is
not `cargo clean` and re-runs no setup script.

---

# Part 1 — Verification

## Phase 0 gate: all four confirmed, line refs drifted

**1. Duplicate-path adds never update metadata.** Confirmed at
`ephemeral/index.rs:208-217` (doc says 203). `add_entry_with_registry` returns
early on a `path_index` hit with `Ok((None, existing_uuid))`. Size, mtime and
ctime from the new observation are dropped, and `bump_ancestor_bytes` never
runs, so rollups go stale on every re-add of a changed file.

**2. Snapshot rewrite skipped on unchanged entry count.** Confirmed at
`ephemeral/cache.rs:469-478` (doc says 457; 457 is now the restore-gate warn
added later). `last_saved_entries == entry_count` returns before
`save_snapshot`, so a rename-only change or any balanced add/delete churn never
reaches disk.

**3. Root-match fallback hands a mount path's source id to a different drive.**
Confirmed at `ephemeral/sources.rs:113-121`, and it is worse than described. When
a fingerprint is supplied but matches nothing, the exact-root arm still wins, and
lines 114-116 then *overwrite* the stored fingerprint with the new drive's. So
drive B mounted where drive A used to live inherits A's source id and snapshot,
and A's fingerprint is destroyed, breaking A's own rebind on its next return.

**4. Attachment by path existence.** Confirmed. Every decision is
`root.exists()`: `cache.rs:169`, `cache.rs:210`, `cache.rs:408`, and the
`attached` field in `sources()` at `cache.rs:178`. No fingerprint is consulted
after registration, and nothing subscribes to `VolumeMountChanged` on the
ephemeral side (`volume/manager.rs:810` emits it; the cache never listens).

### The three plausible-unverified reports

**Recursive-reindex orphans — confirmed, and larger than reported.**
`index.rs:435 clear_directory_children` removes direct children from
`path_index`, `id_to_path`, `entry_uuids`, `content_kinds` and
`collection_flags`, then prunes the parent's `children` vec. It never recurses.
Grandchildren keep full `path_index` and `id_to_path` entries while their parent
chain is severed, so a subsequent re-add of that subtree hits the stale id, takes
the duplicate path in bug 1, and keeps stale metadata permanently. The two bugs
compound: fix 1 without fixing this and the stale node is still detached from
every rollup.

Two more leaks under the same roof, in all three removal paths
(`clear_directory_children`, `remove_entry:780`, `remove_directory_tree:815`):
`arena` has no `remove` at all — it is append-only (`arena.rs` exposes
insert/get/get_mut/iter only) — and `registry`, the name trie behind
`find_by_name`/`find_by_prefix`, has no `remove` either (`registry.rs`). Every
delete leaks an arena slot and a trie entry for the life of the session. Search
results stay correct only incidentally, because `reconstruct_path` resolves
through `id_to_path`, which *is* cleaned; the stale ids filter themselves out.

**Registry save failures swallowed — confirmed.** Five sites, all
`let _ = self.save()`: `sources.rs:108, 119, 133, 162, 170`. A failed write means
the source registration, the rebind, or the stats update is lost silently at next
launch.

**Snapshot `source_id` unchecked on restore — confirmed, low severity.**
`cache.rs:381` compares `meta.root_path` against `record.root` and discards on
mismatch, but `meta.source_id` is never compared to `record.id`. The snapshot
file is keyed by `record.id` in its path, so this only bites on a copied or
hand-edited cache directory. One-line guard, worth taking while in the file.

### Acceptance target exists

The round-trip test the plan says to extend is in `cache.rs` at ~800
(register → index → snapshot → unplug → restore → uuid survives), alongside
`test_unrestored_session_cannot_clobber_snapshot` at ~866. Both are the right
shape to extend with the rename-only and same-name-different-drive cases.

## Phase 1: one design collision, and the API named doesn't exist

The archive crate already ships a per-source store, but not the one contract 1
describes, and the difference is a decision, not a gap.

- **File name.** `crates/archive/src/source.rs:91,108` opens
  `<source_dir>/data.db`. The plan says `source.db` throughout.
- **Where durable lives.** `crates/archive/src/library.rs:1-10` states the
  opposite of contract 1 as a design position: source indexes are disposable,
  and everything durable (`record_overlay`, `grouping`, `lib_edge`) lives in
  `registry.db`, "one durable file, one backup unit." Contract 1 wants the
  rebuildable generation and the durable tables inside one transaction boundary
  in the same file. Both are defensible. Pick one before phase 1 opens, because
  every later task keys off it.
- **The rebind key is the wrong shape.** `registry.db` rebinds by
  `(source_id, type, external_id)`. For a filesystem source `external_id` is the
  path, so a move or rename loses the assertion — exactly what contract 2's
  evidence-tuple two-factor ledger exists to prevent. There is no ledger table
  in `record.rs:17-73` and no assertion table anywhere. Contract 2 is fully
  greenfield.
- **`apply_mutations` does not exist.** The write path is `SourceDb::upsert`
  (`db.rs:173`), one statement per record, with `set_cursor` (`db.rs:565`)
  outside any transaction. The blocker the plan names is real — the checkpoint
  is not atomic with its batch — but the task is to introduce the batched API,
  not to make an existing one transactional.
- **Pooled handles: confirmed unbuilt.** `SourceRegistry::open` (`source.rs:102`)
  builds a fresh `SqlitePool` per call; `create` opens one and closes it at
  `source.rs:96`.

## Content identity: two gaps in the target substrate

Today content identity is a library-wide table whose uuid is *derived*, not
assigned: `v5(CONTENT_NAMESPACE, content_hash)` (`content_identity.rs:105`).
That makes it convergent — two machines that have never communicated compute the
same id for the same bytes, offline and retroactively, with no coordination.
`SdPath::Content { content_id }` is a real address because of this property, not
a local handle.

Neither half of it survives the move as the record table is written today.

**The convergent uuid is not carried over.** `record.rs:40` gives `content` an
integer local primary key and no uuid column. Ship that and content identity
becomes a per-source rowid: every cross-source or cross-device claim has to be
reconstructed by comparing hash strings rather than by two parties independently
deriving the same identifier, and `SdPath::Content` stops resolving. The
derivation is free — it is a pure function of a hash already being stored — so
this is a column, not a feature.

**Dedup does not work inside a single source.** `db.rs:495 set_content_identity`
does an unconditional `INSERT … RETURNING id`, and `idx_content_sampled`
(`record.rs:47`) is a plain index, not UNIQUE. Two identical files in the same
source get two content rows with two ids, and the local join finds neither from
the other. The substrate is double-counting before the catalog is built on top
of it.

While in the file: `deterministic_uuid` names the mechanism, and the mechanism
is implied — telling a reader that a hash-derived id is deterministic is telling
them a hash is a hash. `uuid_for(content_hash)` says what you get and what you
give it. The property itself is worth naming once in module docs, where the
honest word is *convergent*, rather than in every call site.

## Volumes and sources describe the same thing twice

`volume` (`library.db`) and `SourceRecord` (`sources.json`) overlap on seven
fields: `fingerprint`/`fingerprint`, `mount_point`/`root`,
`last_seen_at`/`last_seen_secs`, `tracked_at`/`created_at_secs`,
`total_file_count`/`entry_count`, `unique_bytes`/`total_bytes`, and
`is_online` against attachment derived from `root.exists()`.
`total_file_count` additionally exists on `location`, so one number has three
homes.

`ops/volumes/index/action.rs` writes both in a single action — `register_source`
at 107, then a `volume::Entity::update_many()` filtered on
`Column::Fingerprint.eq(...)` at 250 — joined by a fingerprint string that is a
foreign key in neither direction. They also update at different moments (volume
stats at index completion, source stats at snapshot save), so the two counts
drift during normal operation with nothing to notice.

They cannot simply merge: the cardinality is many sources to *optionally* one
volume. Nested roots register as distinct sources and are tested
(`cache.rs:758`), and adapter, cloud and fingerprint-less network sources have no
volume at all.

The boundary that holds is medium versus index. A volume is a device fact —
capacity, filesystem, speed, removable, online, which machine — true whether or
not anything is indexed. A source is a Spacedrive fact — a root, a record count,
a snapshot, a restore state. Split the fields along that line and give `source` a
nullable `volume_uuid`.

The payoff is not tidiness. Attachment is a property of the medium, and the
volume manager already tracks it (`is_online`, maintained from mount events).
With the FK, a source is attached when its volume is online and its root
resolves under that volume's current mount point — which makes phase-0 bug 4
unrepresentable rather than patched at the four sites that currently call
`root.exists()`.

This has a deadline. `sources.json` becomes a real table in `library.db` for the
first time at T6.1. Authoring it correctly costs nothing then; changing it later
is a migration against real libraries.

**Unnamed prerequisite: there are two source registries.**
`ephemeral::SourceRegistry` (`sources.json`, fingerprint-keyed, filesystem roots,
written only from `ops/volumes/index/action.rs:107`) and `archive::registry`
(`registry.db`, adapter/data-type keyed, driving `sources.*` ops). The ephemeral
module's own header (`sources.rs:9-10`) says it "converges with the archive
sources registry when filesystem sources gain durable record stores." That
convergence is phase 1's actual first task and no phase names it.

## Phase 2: register corrections

**`files.media_listing` is not a branch deletion, it's a rewrite.** 642 lines,
zero references to `ephemeral` anywhere in the file, DB-only from
`find_parent_directory` down. `sources.media_listing` is the archive-sources op,
a different surface. Same situation for `files.content_kind_stats` (108 lines,
DB-only, 1 UI caller). `files.unique_to_location` (268 lines) is DB-only with
zero callers and is a straight delete.

Ephemeral coverage across `ops/files/query`, measured:

| op | lines | ephemeral branch | disposition |
|---|---|---|---|
| `files.directory_listing` | 1028 | yes | delete DB branch |
| `files.by_id` | 372 | yes | delete DB branch |
| `files.by_path` | 327 | yes | delete DB branch |
| `files.collection_listing` | 164 | yes | delete DB branch |
| `files.size_tree` | 179 | yes | delete DB branch |
| `files.media_listing` | 642 | **no** | **rewrite on arena** |
| `files.content_kind_stats` | 108 | **no** | **rewrite or drop** |
| `files.alternate_instances` | 361 | **no** | return empty until phase 5 |
| `files.unique_to_location` | 268 | **no** | delete, 0 callers |

**The DB branch isn't cold by construction.** `directory_listing.rs:136-160`
prefers the database whenever a location row exists with `IndexMode != None` and
the parent entry resolves. It's cold because nothing adds locations in practice,
not because the code deprioritizes it. Fine given no install base, but the
teardown is a behavior change for any library that has a location row, and that
should be said out loud rather than assumed away.

**Tags.** The manager is `ops/metadata/manager.rs` (698 lines), not under
`ops/tags/*` as listed. Nine registered tag ops (`ops/tags`, 4,824 lines);
`apply`/`unapply` route through `UserMetadataManager` into `user_metadata` +
`user_metadata_tag` keyed on `entry_uuid` or `content_identity_uuid`, and
`files.by_tag` reads back through entries. The plan's move is accurate.

`user_metadata` also carries `notes`, `favorite`, `hidden` and `custom_data`.
None have an op surface — the only reader is `locations.export/action.rs:474` —
and listing `is_hidden` is derived from the path (`is_hidden_path`), not from
this table. They drop with no destination needed. Worth stating explicitly so it
isn't rediscovered as a regression.

**Watcher.** `watcher_old` is **not compiled** (`service/mod.rs:23`: "kept as
reference during migration but not compiled"). 5,559 lines of free deletion; its
6 entity references are noise, not work. The live watcher registers roots from
exactly one place, `ops/locations/add/action.rs:124`, and there is no boot-time
restore of watched roots at all. "Re-anchor boot-time watched roots to the source
registry" is therefore new capability, not a move.

**Missing from the register entirely.** 76 files reference the dying entities.
The plan names roughly a third. Not listed anywhere:

- `core/src/location/*` — 1,729 lines, a top-level module, `manager.rs` alone
  carries 31 entity references. The plan deletes `ops/locations/*` but never
  mentions this.
- `core/src/domain/` — `file.rs` (7), `location.rs`, `addressing.rs`,
  `content_identity.rs`.
- `ops/indexing` persistent half — `database_storage.rs` (1,230),
  `change_detection/persistent.rs` (918), `handlers/persistent.rs` (697),
  `phases/processing.rs` (767), `phases/discovery.rs` (632),
  `phases/aggregation.rs` (310), `verify/` (773), `path_resolver.rs` (378),
  `persistence.rs`, `responder.rs`.
- `ops/search/filters.rs` (10 refs), `facets.rs`, `sorting.rs` — only
  `query.rs` is named.
- `library/mod.rs` statistics — `calculate_file_statistics` at 993 and its
  static twin at 1330 scan `location::Entity` then `entry::Entity`;
  `calculate_sidecar_statistics` at 1273.
- `ops/redundancy` (372), `volume/manager.rs`, `service/network/protocol/messaging.rs`.
- **`apps/mobile`** — four files call `locations.list`, including
  `screens/explorer/hooks/useVirtualListing.ts`. The plan's surface list names
  only settings UI and CLI. Mobile is a second full explorer surface.

**Frontend footprint of `locations.list`:** 12 callers, and they are not
settings screens. `ShellLayout.tsx`, `SpacesSidebar/LocationsGroup.tsx`,
`explorer/components/PathBar.tsx`, `explorer/hooks/useVirtualListing.ts`,
`routes/overview/index.tsx`, `overview/DevicePanel.tsx`,
`Inspector/variants/LocationInspector.tsx`, plus the four mobile files.

## Sync: the claim is wrong, the conclusion holds

"Nothing reads `sync_enabled`" is false as written.
`service/network/transports/sync.rs:376,390` filters peer routing on
`device.sync_enabled`, and `ops/sync/get_sync_partners/action.rs:84,103` reports
it. What is genuinely write-only is the *library-level* `settings.sync_enabled`:
set by `ops/config/library/update.rs:146`, read back only by
`ops/config/library/get.rs:72`, gating nothing. The drop removes the device-level
readers along with the service, so the conclusion stands.

The surface is bigger than "`service/sync`, ~264K". Everything that goes:

| module | lines | files |
|---|---|---|
| `core/src/service/sync` | 8,315 | 14 |
| `core/src/infra/sync` | 7,256 | 22 |
| `core/src/service/file_sync` | 985 | 4 |
| `core/src/ops/sync` | 659 | 17 |
| `core/src/service/sidecar_sync` | 266 | 3 |
| `core/src/library/sync_helpers.rs` | — | 1 |

Roughly 17.5k lines. `infra/sync` holds `syncable.rs`, the trait every dropped
entity implements, and is not on the plan's delete list. Live wiring to unpick
sits at `lib.rs:356-384` and `library/manager.rs:539-620`.

## File actions: confirmed path-native

`ops/files/{copy,rename,delete,create_folder,validation}` reference zero
entities. The only two references in all of `ops/files` are
`query/content_kind_stats.rs:9` and a test at `query/directory_listing.rs:881`.
The claim holds exactly as written.

## Phase 4 table list: exhaustive and correct

22 dropped + 11 survivors = all 33 entity modules. No orphan, no double-count.
Two notes:

- The survivor list names a *source registry*, *settings* and *jobs* table. None
  exist as entities today — the registry is `sources.json`, settings are JSON in
  `library/config.rs`, jobs are not an entity. The fresh baseline is therefore
  partly new schema, not a subset of the old one.
- 40 migrations squash to 1 (`infra/db/migration/`).
- `space_item.entry_uuid` is uuid-keyed as claimed (`space_item.rs:15`), so no
  schema work. Continuity across the drop depends on the ledger minting the same
  uuids, which it won't — but libraries are recreated, so it's moot. Space items
  pointing at files go dark on existing libraries and that is intended.

## Phase 3.1: further along than the plan implies

`crates/pvcache` and `crates/bake` are built and tested (2,164 lines combined,
reader contract exercised in `apps/native/src/source/pvcache.rs`).
`SourceDirs::thumbs_file` (`infra/source_dirs.rs:78`) already yields
`sources/<id>/thumbs.pvcache`, and `cache.rs:187` already publishes `thumbs_path`
on every `SourceStatus`. What's missing is narrow: core never constructs a
`Pvcache` writer — the only producer wiring in the tree is
`apps/native/src/demo.rs:126` — and the mounts server has no `/hot-thumb` route
(it serves `/`, `/dav`, `/dav/` only). The "no-op stub" is `writer.rs:245`, where
file processors are disabled outright for browse responsiveness.

---

# Part 2 — Task register

Groups marked **‖** run in parallel. Everything lands on the current branch.

## P0 — Unblock

**T0.0 Reclaim disk.** Remove `target/debug/incremental` (22 GB). Confirm
`cargo check` and `cargo check -p sd-core --all-targets` both go green. This is
the baseline every later acceptance compares against.

## P1 — Gate fixes (`ops/indexing/ephemeral`) ‖ with P3.1

One module, so one agent unless split by file.

**T1.1 Identity-correct removal.** `index.rs`.
Recurse in `clear_directory_children:435` so descendants leave `path_index`,
`id_to_path`, `entry_uuids`, `content_kinds`, `collection_flags`. Add
`Arena::remove` and `NameRegistry::remove`, and call both from all three removal
paths (`clear_directory_children`, `remove_entry:780`,
`remove_directory_tree:815`). Accept: removing a nested tree leaves
`arena.len()`, `registry.total_entries()` and `path_index.len()` at their
pre-insert values.

**T1.2 Duplicate adds update in place.** `index.rs:208-217`.
On a `path_index` hit, write the new size/mtime/ctime into the node and apply the
size delta through `bump_ancestor_bytes`. Keep the existing uuid. Accept: re-add
of a grown file moves `subtree_bytes` on every ancestor.

**T1.3 Snapshot writes on any change.** `cache.rs:469-478`.
Replace the entry-count comparison with a dirty flag set by every mutating index
call. Accept: rename-only churn persists across save/restore.

**T1.4 Fingerprint-verified identity.** `sources.rs:96-135`, `cache.rs`.
`register` must not fall through to exact-root when a fingerprint was supplied
and matched nothing — mint a new source instead, and never overwrite a stored
fingerprint with a different one. Replace `root.exists()` at `cache.rs:169, 210,
408, 178` with fingerprint-verified attachment, driven by a `VolumeMountChanged`
subscription. Accept: same-name different-drive mount at a known path gets its
own source id and does not restore the other drive's snapshot.

This is the patch, not the fix. The structural version arrives with T2.0b, where
attachment derives from the volume's `is_online` and the four `root.exists()`
sites stop existing. Do it here anyway — P2 is a long way off and the bug
corrupts identity now.

**T1.5 Registry writes are fallible.** `sources.rs:108,119,133,162,170`.
Propagate `save()` errors to callers; log at `error` at the boundary. Accept: a
read-only registry dir surfaces an error rather than silently losing a
registration.

**T1.6 Snapshot id guard.** `cache.rs:381`.
Discard on `meta.source_id != record.id` alongside the existing root check.

**T1.A Acceptance.** Extend the round-trip test at `cache.rs:~800` with a
rename-only change and a same-name different-drive mount; both survive
save/restore with correct routing.

## P2 — Durable substrate

**T2.0 Store shape — settled: one file per source.** `source.db` holds the
rebuildable generation, the identity ledger and the assertions together.

The deciding fact is the journal mode. `source.rs:31` opens every source pool in
WAL. SQLite provides atomic commit across `ATTACH`ed databases through a
super-journal in rollback mode, but not in WAL — multi-database atomic commit is
unsupported there. So the two-file layout cannot commit a batch and its watermark
together without giving up WAL, and WAL is the mode this design wants: reads fan
out across sources while indexing writes.

Note what is being reversed. The two-file split is not an accident in
`crates/archive`. Two prototypes converged on it independently: the native
prototype (`~/Projects/spacedriveapp/native`, June 2026), whose `crates/model`
treats a source index as disposable and rebinds durable state through
`record_overlay` on re-add; and Console (`~/Projects/console`, July 2026), whose
`archive/src/schema.ts` splits a disposable per-source mirror from a durable
library, with the reasoning written down in `docs/specs/archive-contract.md`. A
source is "removable without surgery" precisely because of that split.

Contract 1 was written later without engaging with either. WAL is a real
constraint neither prototype faced, so the decision stands — but it inherits an
obligation. "Delete the rebuildable generation, lose nothing a person typed" was
structural in both prototypes and is a discipline here, held by dropping and
recreating the generation tables inside a transaction. Whoever ports the archive
crate is undoing a deliberate design: expect the seams to be load-bearing, and
read the contract spec before cutting them.

The two-file advantage — `rm` the index without touching what a person typed —
turns out not to be an advantage worth keeping. **Decided 2026-08-21: index to
keep. A source store is never rebuilt from the source.**

"Disposable mirror" assumed every source can be re-read on demand. That holds for
a photos library or a folder. It is false for Gmail (deleted mail is gone from
the API), Slack (retention has already rotated it), GitHub (deleted issues, lost
access) and browser history (browsers prune) — five of the eleven adapters in the
tree, and precisely the ones the word *archive* was pointing at. For those the
mirror is the only remaining copy, so deleting it is data loss rather than cache
eviction.

The alternative was to split sources by whether they can be re-obtained, with a
flag beside `trust_tier`. Rejected as unnecessary complexity: everything is kept.

Three consequences to carry:

- The mirror/library split loses its remaining rationale. It existed so an index
  could be dropped freely; nothing is dropped freely now, so one durable file per
  source is simply the shape.
- A source store is user data. It needs backing up, and `delete_source` is a
  destructive operation rather than a cache clear.
- `_sync_state` holds the adapter cursor, and `record.rs` notes the JSONL protocol
  is a delta stream resumed from it. Losing that file was already worse than it
  looked; now it is not something the system does at all.

Rebind key follows contract 2: the evidence-tuple ledger, not
`(source_id, type, external_id)`. See T2.5.

The assertion tables are shaped here rather than at T4.9, because a definition
that has to survive travelling on a drive constrains what may be stored on it:
`docs/core/design/tags-and-assertions.md`. `crates/archive`'s `record_overlay`
folds into that table on the way in — it is the same idea in the two-file layout,
keyed by a path.

`crates/archive` is built the other way and has to move.

**T2.0b Decide the volume/source boundary.** Blocking for T6.1, not for T2.1.
Medium facts stay on `volume`, index facts move to `source`, `source` gets a
nullable `volume_uuid`. Attachment then derives from the volume's `is_online`
plus a root that resolves under its current mount point. Decide it before the
registry becomes a table.

**T2.1 Converge the two source registries.** `ephemeral/sources.rs` +
`crates/archive/src/registry.rs`. One registry, fingerprint-capable, serving both
filesystem roots and adapter sources, with the field split from T2.0b. Currently
the only writer of the ephemeral side is `ops/volumes/index/action.rs:107`.

**T2.2 Batched transactional writes.** `crates/archive/src/db.rs`.
Introduce `apply_mutations` taking a batch, wrapping the per-record `upsert:173`
work and the `set_cursor:565` watermark in one transaction. Accept: kill mid-batch,
reopen, no partial batch and no advanced cursor.

**T2.3 Pooled source handles.** `crates/archive/src/source.rs:26,96,102`.
Cache `SqlitePool` per source id instead of open-per-call.

**T2.4 adapter.toml removal for Rust connectors.** `crates/archive/src/adapter/`.
Native adapters declare their schema in Rust; `script.rs` keeps the TOML path.

**T2.5 Identity ledger.** New. Evidence-tuple rebinding, two-factor minimum, so
record uuids survive reindex, rename and move. Depends entirely on T2.0, and
**gates T4.9**.

Part of this is a move, not just new construction. Record uuids are durable state
living inside `ephemeral.snapshot` today, alongside six fields that are pure
cache — and the file is governed by cache rules, including a `remove_file` when a
drive returns at a different mount point. The ledger is where the uuid bindings
go so the snapshot can go back to being a projection it may discard freely.

Accept: delete every snapshot, re-walk, and record uuids are unchanged. That is
the property the snapshot cannot provide and the ledger exists for.

**T2.6 Connector fan-out.** One observation stream from walk + watcher, uuid at
first sight, arena and the source store both consumers, checkpoint in the batch
transaction from T2.2.

**T2.7 Content identity in the record table.** `crates/archive/src/record.rs`,
`crates/archive/src/db.rs`. Three parts, all small, all cheap now and expensive
after there is data:

- Add a `uuid` column to `content`, carrying the convergent derivation forward.
  Without it `SdPath::Content` stops resolving and cross-device dedup becomes a
  reconciliation problem.
- `UNIQUE` on `sampled_hash`, and make `set_content_identity:495` an upsert that
  reuses an existing row. Today it inserts unconditionally, so identical files in
  one source get separate content rows.
- Rename `deterministic_uuid` to `uuid_for`, and state the convergent property
  once in module docs instead of in the function name.

Accept: two copies of one file in a source share a content row and a uuid; the
same file on two sources yields the same uuid with no coordination.

**T2.A Acceptance.** Index a filesystem source, tag a file, delete every snapshot
and the arena, re-walk — the record uuid is unchanged and the tag is still
attached. Unplug mid-index, replug — no duplicate records.

Note the change of subject. The old wording was "delete the rebuildable
generation, reindex", which under *index to keep* is no longer an operation for
an archive source: its store is the only copy. The property being tested is that
identity survives losing the **cache**, which is the filesystem lineage's problem
(record uuids currently live in `ephemeral.snapshot` — see T2.5). For an archive
source the equivalent test is that its store is never rebuilt at all: assert that
`delete_source` refuses without an explicit destructive confirmation.

## P3.1 — Hot tier ‖ with everything

Start here. It depends on nothing else in this document, it touches no entry
table, and it fixes a surface that shows nothing today.

### Why the existing thumbnail path cannot serve filesystem sources

Not an unwired feature — a key mismatch. The sidecar tree is content-addressed:
`content/{h0}/{h1}/{content_uuid}/{kind_dir}/{variant}.{ext}`
(`crates/sidecar-path/src/lib.rs`). Writing a thumbnail needs a `content_uuid`,
which needs a content hash, which needs reading the file — which is exactly what
the browse path refuses to do (`ephemeral/writer.rs:245`, processors disabled for
responsiveness). The frontend closes the loop: `Thumb.tsx:64` returns null
without `file.content_identity?.uuid`, and `File::from_ephemeral` sets
`content_identity: None, sidecars: Vec::new()`.

pvcache keys on `(record_uuid, source_version(size, mtime))` instead. The arena
already holds all three inputs, so no hashing, no entry row, no location, no
sidecar table. The trade is that two identical files bake twice; correct for a
hot tier, where regeneration is milliseconds and content-addressed dedup only
pays for expensive derivatives.

`generator.rs` (483 lines) is already path-in/path-out and survives. What goes is
the `entry_uuid` → entry row → `PathResolver` → `content_identity` lookup in
`ops/media/thumbnail/action.rs`.

### The format change

Thumbnails are mostly not square, and both the explorer grid and media view with
grid mode off render true aspect. Today's slots are fixed `tile_width ×
tile_height` with geometry in the file header and no per-record dimensions, so
aspect is not representable.

The asymmetry decides the design: `Cover` is derivable from `Contain`, and
`Contain` is not recoverable from `Cover`. Bake the aspect-true frame once and
let each surface crop at render time.

Store **packed, not letterboxed** — write `content_w × content_h` pixels
contiguously and stop. Letterboxing would bake `ScaleMode::Contain`'s background
color into every tile, which renders wrong the moment the theme changes and would
mean rebaking a library over a color. Packed also leaves a landscape frame's tail
pages untouched, so sparse allocation gives back the difference.

`SLOT_HEADER_LEN` is 64 bytes with only 32 used (seq 0..4, flags 4..8, version
8..16, uuid 16..32). The dimensions fit in the free half:

```
content_w: u16   32..34    // true extent of the stored frame
content_h: u16   34..36
source_w:  u32   36..40    // native dimensions of the original
source_h:  u32   40..44    // 20 bytes still spare
```

`FORMAT_VERSION` → 2; reject v1 and rebake, since tiles are regenerable and
existing caches are dev-only. `source_w/h` lets a grid size a cell before its
tile exists, without decoding the file or waiting for P5's dimension assertions.

**Envelope ≈ 1.5× the square target.** For a derived square crop to stay sharp,
an aspect-fit frame's short side must still reach the square size. At a 256
target a 3:2 frame needs 384×256, so a 384×384 envelope keeps both orientations
sharp (landscape 384×256, portrait 256×384, square crop 256² either way) and
degrades gradually past 3:2 rather than falling off. Allocated slot cost is
384²×4 = 590KB against 262KB at 256, but packed writing means a 3:2 landscape
writes 393KB and the remainder stays sparse.

### Tasks

**T3.1a Format v2 in `crates/pvcache`.** `layout.rs` gains the four fields and
the version bump; `writer.rs:118 write` takes content and source dimensions and
accepts `pixels.len() == content_w * content_h * 4` instead of requiring
`tile_len`; `reader.rs` returns the dimensions with the tile so callers know the
valid extent and row stride. Slot stride stays the full envelope, so seqlock,
growth and offset maths are untouched. Accept: a 384×256 frame round-trips
through save and reopen with its dimensions intact, and a v1 file is rejected
rather than misread.

**T3.1b Native reader follows.** `apps/native/src/source/pvcache.rs` and
`grid/mod.rs` build `Arc<RenderImage>` at the content extent. gpui owns the Metal
sprite atlas and takes `image::Frame`, so variable-size tiles are native there —
no fixed-cell atlas rework and no row-alignment constraint. Ships in the same
commit as T3.1a.

**T3.1c Producers return true extent.** `crates/bake/src/raster.rs` — producers
emit a frame at its fitted size rather than filling an envelope.
`ScaleMode::Cover` stays for callers that want a square bake; nothing in this
plan uses it.

**T3.1d Producer in the daemon.** Port the `BakePool` chain from
`apps/native/src/demo.rs:126,256` into core, fed from the arena
(`get_or_assign_uuid` for the key, `source_version(size, modified)` for the
version), writing `SourceDirs::thumbs_file(source_id)`. Viewport-first, replacing
the disabled processors at `ephemeral/writer.rs:245`.

**T3.1e `/hot-thumb/:source_id/:record_uuid`.** `core/src/service/mounts`, which
today serves only `/`, `/dav`, `/dav/`. Encode the packed frame for DOM clients
and return its content rect alongside.

**T3.1f `Thumb.tsx` prefers the hot tier.** Falls back to the sidecar path while
both exist. `squareMode` becomes a render-time crop of the packed frame rather
than a request for a different artifact.

**T3.A Acceptance.** Fresh library, clean build: thumbnails in every grid surface
including ephemeral, at true aspect, with the sidecar tables untouched and no
row written to `content_identity`. Switching theme does not change a tile.

## P4 — Re-anchor survivors

**‖ group A — self-contained moves**

- **T4.1 Path gates.** `service/network/protocol/file_transfer.rs:404` and
  `ops/files/delete` gate re-anchored from location roots to registered source
  roots, matching the byterange protocol.
- **T4.2 Watcher roots at boot.** New capability. Register watched roots from the
  source registry at startup; route events by source-root prefix. Delete
  `change_detection/persistent.rs` with the entries write path. Free deletion of
  `service/watcher_old` (5,559 lines, not compiled).
- **T4.3 Library statistics.** `library/mod.rs:993,1273,1330` off entry and
  location scans onto arena rollups (`subtree_bytes`) and source stores.
- **T4.4 Enrichment policy.** `location.job_policies` becomes policy rows on
  source subtrees; `ops/locations/trigger_job` re-anchors to (source, subtree),
  then leaves `ops/locations`.

**‖ group B — query surface**

- **T4.5 Delete DB branches** in `directory_listing`, `file_by_id`,
  `file_by_path`, `collection_listing`, `size_tree`, `search/query.rs` plus
  `search/filters.rs`, `facets.rs`, `sorting.rs`.
- **T4.6 Rewrite `files.media_listing`** (642 lines) on the arena. Not a branch
  deletion.
- **T4.7 Rewrite or drop `files.content_kind_stats`** (1 UI caller); delete
  `files.unique_to_location` (0 callers); `files.alternate_instances` returns
  empty until phase 5.
- **T4.10 Global search router.** Gates the drop. `SearchScope::Library` is the
  default scope and is served only by the entry tables;
  `execute_ephemeral_search` rejects anything that is not `Path` scope
  (`query.rs:1555`). Build the filesystem fan-out — query every source, compose
  overlays, re-sort, truncate — as the single scatter-gather implementation the
  read path uses everywhere. `crates/archive/src/search/router.rs` is the
  template. Accept: unscoped search returns hits from every attached source,
  correctly ranked and paginated, with no per-call-site merge code.
- **T4.8 `spaces/get_layout`** entry joins (`query.rs:6,92,171`) serve from arena
  lookups. Verify space items resolve through arena/ledger; no schema work.

**‖ group C — tags**

- **T4.9 Rewrite tags.** `ops/metadata/manager.rs` (698) and `ops/tags/*`
  (4,824, 9 ops) against `docs/core/design/tags-and-assertions.md` — a rewrite,
  not a re-key, because definitions now have to travel with a source and merge
  on arrival. Definitions and assertions both land in `source.db`; definitions
  replicate into every source that uses them; an assertion is append-only,
  HLC-ordered, and carries a record uuid and a content id together rather than
  `user_metadata`'s exclusive `entry_uuid`/`content_identity_uuid` pair, which
  is what makes content-attached tags stop degrading. `library.db` keeps sidebar
  pinning, ordering, color overrides and definitions not yet applied anywhere.
  The definition fields that cannot merge across libraries
  (`composition_rules`, `attributes`, `applied_variant`, `confidence`, the
  four-way naming) drop. `notes`/`favorite`/`hidden`/`custom_data` drop with no
  destination — no op reads them.

  **Ordering constraint: T2.5 must land first.** An assertion carrying a record
  uuid is only as durable as that uuid, and today record uuids live in
  `ephemeral.snapshot` — a cache with five discard paths, one of which deletes
  the file outright when a drive returns at a different mount point
  (`cache.rs:414`). Re-key tags to record uuids while that is still true and
  tags disappear when a drive is plugged into a different port. It presents
  intermittently and nothing points at the mount path.

  The bug is currently masked because tags key to `entry.uuid` in `library.db`,
  which the snapshot never touches. T4.9 removes the mask. So the uuid bindings
  have to move out of the snapshot into the identity ledger before anything
  durable keys to them — the snapshot then holds a projection it may discard
  freely, which is what it was always written to assume.

**T4.A Acceptance.** Full UI pass — browse, search, collections, media view, tag
a file, spaces — with entries branches compiled out behind a temporary flag,
before anything is dropped.

## P5 — Media in the record table

- **T5.1 Content hashing** as the tiered ladder writing to source-store records
  instead of `content_identity`.
- **T5.2 Media metadata as assertions.** EXIF, duration, dimensions from drain
  processors, replacing `image/video/audio_media_data`. Blurhash dropped.
- **T5.3 Derivatives as assertions.** Content-keyed per-source storage;
  regeneration is the migration. Both shell sidecar servers die.

## P6 — The drop

> **Historical inventory.** Use `2026-09-15-entries-final-drop.md` to execute
> this phase. In particular, do not delete `infra/sync` or `service/sync` by
> directory. Preserve current source/peer capabilities while removing the legacy
> entry-row protocol. The breaking-release decision removes locations without
> migration; Space items own navigation and policies own processing behavior.

One commit series. Sized from the actual tree.

- **T6.1 Schema.** Drop 22 tables plus FTS5 tables and triggers. Squash 40
  migrations to one baseline, which must *add* source-registry, settings and jobs
  tables that don't exist as entities today. Version check refuses old files with
  a reindex message. The source-registry table is where T2.0b lands: index facts
  only, plus a nullable `volume_uuid`, with the medium facts left on `volume`.
  This is the last moment that split is free.
  Rename `ephemeral.snapshot` to `index.snapshot` here too (T6.7): this commit
  already recreates libraries, and doing it separately costs a second reindex.
- **T6.2 Entities and modules.** 22 entity modules; `core/src/location/*`
  (1,729); `ops/locations/*` (3,235, 10 ops); the persistent indexing half
  (`database_storage` 1,230, `change_detection/persistent` 918,
  `handlers/persistent` 697, `phases/processing` 767, `phases/discovery` 632,
  `phases/aggregation` 310, `verify` 773, `path_resolver` 378,
  `persistence`, `responder`); `DatabaseAdapter` and the `Persistent` sink;
  `ops/redundancy` (372); `SidecarManager` DB flows; the `domain/` types.
- **T6.3 Sync.** `service/sync` (8,315), `infra/sync` (7,256),
  `service/file_sync` (985), `ops/sync` (659), `service/sidecar_sync` (266),
  `library/sync_helpers.rs`, and the wiring at `lib.rs:356-384` /
  `library/manager.rs:539-620`.
  **Salvage first:** `infra/sync/hlc.rs` and `infra/sync/time_source.rs` move out
  before the delete. HLC is the right tool for ordering assertion writes — two
  machines tagging the same file offline is a causality question — and it returns
  the moment assertions need last-writer-wins across devices. Neither file
  touches the entry schema. Deleting them here and rewriting them in P7 is pure
  waste.
- **T6.4 Tests.** 15 integration tests reference the dying world; the 5 sync
  tests (~4,300 lines) and `location_export_import_test.rs` (695) go outright,
  the rest re-point. `tests/helpers/sync_harness.rs` deletes;
  `tests/helpers/indexing_harness.rs` re-points (10 dependents). Three examples
  in `core/examples` reference dropped entities.
- **T6.5 Surface.** Unregister dead ops, regenerate TS types, remove the CLI
  `location` domain, and strip locations from **both** frontends. Rename
  `core.ephemeral_status` / `core.ephemeral_reset` and `EphemeralSourcesGroup.tsx`
  in this commit (T6.7) — the wire names break clients at runtime rather than at
  compile time, so they should land in the one regeneration clients already have
  to absorb. Frontends: interface
  (`ShellLayout`, `SpacesSidebar/LocationsGroup`, `explorer/PathBar`,
  `explorer/useVirtualListing`, `overview/index`, `overview/DevicePanel`,
  `Inspector/LocationInspector`, `AddLocationModal`, `AddStorageModal`) and
  mobile (`screens/explorer/useVirtualListing`, `screens/browse/LocationsGroup`,
  `screens/overview/OverviewScreen`, `screens/overview/DevicePanel`).
- **T6.6 Flip the record table rule** in AGENTS.md ("Current direction") from "do not
  deepen the entries world" to "the entries world does not exist," and drop
  the `ephemeral` note once T6.7 has run.
- **T6.7 Drop the `ephemeral` qualifier.** Its own commit, no behaviour riding
  along. See below.

### T6.7 — retiring "ephemeral"

The word marked a contrast with the durable entries index. That contrast dies
here, and the module becomes the only indexer there is. It is also already
inaccurate: the module persists registrations to `sources.json`, saves and
restores per-source snapshots across launches, and serves detached drives from
them. None of that is ephemeral.

Measured surface: 88 Rust files, 954 occurrences, ~30 distinct identifiers, plus
123 in TypeScript. The teardown removes 180 of the Rust occurrences on its own
(`watcher_old`, `handlers/persistent`, `phases/`, sync tests); **774 survive**, so
waiting shrinks this by a fifth, not by most.

**Do it after the drop, not before.** While both substrates exist the qualifier
is load-bearing: `ephemeral` versus `persistent` is what tells a reader which
world a call site belongs to, which is exactly the signal the teardown needs to
stay auditable. Renaming earlier also muddies the deletion diff.

Most of the work is compiler-enforced and therefore safe — `EphemeralIndex`,
`EphemeralIndexCache`, `ephemeral_cache()`, `EphemeralEventHandler`,
`from_ephemeral`, `set_ephemeral_index`. Wrong means it does not compile.
`IndexPersistence::{Persistent, Ephemeral}` collapses entirely; the enum exists
only to choose between two sinks and one of them is gone.

One structural note: folding `ops/indexing/ephemeral/` into `ops/indexing/`
collides on `mod.rs` and `responder.rs`. There are two `responder.rs` doing
different jobs — one applies events to the arena, the other wraps
`DatabaseAdapter`. The second dies in T6.2, so the collision resolves itself at
this point in the sequence and would need hand-merging if attempted earlier.

Keep the arena and the durable pieces distinct while renaming rather than
substituting one word throughout. The arena is genuinely in-memory and
rebuildable; the source registry and its snapshots are not. A find-and-replace
would carry the old conflation into the new names.

**Three items land elsewhere, deliberately.** Each invalidates existing libraries
or breaks clients, and each is free if bundled with a change already doing that:

- `ephemeral.snapshot` (`infra/source_dirs.rs:19`) → renaming it silently orphans
  every existing snapshot: no error, the index reads empty and re-walks. Bundle
  into **T6.1**, which already recreates libraries. Separately it costs a second
  forced reindex for no visible benefit.
- The two wire ops `core.ephemeral_status` and `core.ephemeral_reset` are
  client-visible and fail at runtime rather than compile time. Rename them in the
  same commit that regenerates TS types for the dropped schema (**T6.5**), so
  clients absorb one breaking change instead of two.
- `EphemeralSourcesGroup.tsx` is a file rename; it moves with T6.5's frontend
  pass.

**T6.A Acceptance.** Clean build with no reference to dropped entities;
fresh-start daemon reaches the full coherent-startup experience;
`grep entities::entry` returns nothing.

## P7 — Cross-source queries

No `catalog.db`. Every source store stays readable while its drive is
detached, so the questions a swept projection was to answer are asked of the
stores at query time, and there is no second copy to keep current. Detached-drive
search (T4.10), alternates (`files.alternate_instances`) and content-attached
tags already read every store this way. `paths.compare` compares two folders by
location or by content, streaming both sides in index order. Duplicates across
stores and at-risk content are next, as queries over the same stores. A store
kept on its source rather than in the library answers offline only through a
retained library copy, which `2026-09-16-add-to-library.md` owns. The trust
rule is unchanged: never a destructive decision from an index row alone.

## Ordering

P0 → P1 → P2 → P4 → P6 is the chain. P5 gates P6's sidecar and media table
drops. P7 follows P6.

**P1 then P3.1 is the recommended start.** P3.1 is parallel with the teardown
chain and commits to nothing, but it is *not* parallel with P1. Both the hot
tier and the byte plane's block cache key on `source_version(size, mtime)`, and
T1.2 is the bug that stops arena metadata updating when a file changes — so
without it, both caches serve stale data for modified files and report it fresh.
Fix T1.2 first, then P3.1 is the best-value work available: it makes a visible
surface better rather than preserving one, and it shrinks the teardown, since
grids served from the hot tier take a live consumer away from
`content_identity`, `sidecar` and `entry`.

Parallel work landing before the teardown — mounts, byte plane, block cache —
has its own briefing in `2026-08-20-pre-teardown-brief.md`.

**T2.0 is settled** (one file per source, evidence-tuple rebind), so P2 is
unblocked. **T2.0b** (volume/source boundary) is still open; it blocks only
T6.1, but T6.1 is where the source registry becomes a table for the first time,
so that is the last point at which the answer is free rather than a migration.

Search gates the drop: **T4.10** must land before P6, or `SearchScope::Library`
goes dark with the entry tables.
