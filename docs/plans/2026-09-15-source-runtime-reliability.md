# Source Runtime Reliability

> Status: draft for James's review. Research and documentation only.
> Audited: September 15, 2026, approximately 19:17–19:35 America/Vancouver.
> Code: `4bc442e43` plus the existing uncommitted worktree changes.
> Live builds: Mac `2026-09-16T01:46:44Z`; titan `2026-09-15T23:05:47Z`.
> Register: [PROJECT_STATUS.md](../../PROJECT_STATUS.md).

## Outcome

A source remains visible and useful when its arena is unloaded, its restart
snapshot is missing, or its origin stops answering. Its reported health tells
you what was captured, what committed, what is searchable, and what can still
serve file bytes. A successful indexing or hashing job means its promised
writes committed.

Keep fast arena reads for mounted filesystems. Make retained source stores
usable directly, including replicas, so searching 100 sources does not require
reconstructing 100 filesystem trees. Keep sources that only have cached
metadata visible with an explicit limitation.

This is a staged repair of the existing source architecture. It does not
replace the record table, introduce another source registry, revive entries,
turn every mapped volume into a source, or require `catalog.db`.

The plan is a proposal. No daemon restart, reindex, source repair, schema
migration, replica replacement, or deployment was performed for this research.

## Evidence from the running instance

The initial audit called `status`, `sources list`, `index ephemeral-cache
--detailed`, `library info`, `network status`, `network devices`, `job list`,
`op mounts.status`, `op paths.context`, and `search files` through `sd-cli`.
The same source and cache queries ran on titan through `--device titan`.
Read-only SQL checked the Mac's source store. The source databases on titan
were not independently re-counted during this audit.

These are dated observations, not a guarantee about the next session.
The two deployed binaries have different build times, and neither build time
proves its exact git revision or uncommitted source contents.

| Observation | Evidence | Meaning |
|---|---|---|
| One local source on the Mac | `jamespine`, 2,693 registered records; local database independently contains that count | The home source has a durable store, but registration does not prove complete coverage |
| Nine local sources on titan | 217,884 `jamie-nas`; 49,263 `cctv`; 28,079 `dev-tools`; 1,525 `calvin-nas`; 1,380 `footage`; five records across four small sources | These are owner registry counts, including directories |
| Eight active titan replicas on the Mac | `dev-tools` absent from both `sources.list` and `mounts.status` | The displayed inventory depends on successful arena replication |
| `dev-tools` is attached and registered on titan | Cache status reports `restored: false`; no indexed or watched root for it | An idle source can still be unavailable to browsing and replication |
| `dev-tools` fetches fail repeatedly | Mac log: `snapshot unavailable: No such file or directory` | Missing artifact is confirmed; why it originally disappeared is not yet established |
| An older `dev-tools` snapshot exists on the Mac | File under `mounts-remote`, modified at 03:58 local time | A cached artifact is not sufficient for offline startup or active discovery today |
| `Untitled` is mapped and watched without a source store | `paths.context`: memory and restart cache true, source absent, source store false | This is supported behavior, not a defect |
| Home source has missing directory ancestry | 70 nested directory records have `parent_uuid IS NULL`; Desktop, Documents, Downloads, Movies, Music and Pictures are missing from `directory_path` | The stored tree is structurally incomplete despite passing SQLite foreign-key checks |
| Home hashing errors share that shape | All 806 errors are on parentless file records; 802 report missing files and four report a directory | Wrong path reconstruction is a strong causal explanation; do not label all these as filesystem permission failures |
| Home content evidence is limited | 1,678 content rows, all sampled; no integrity hashes; zero `record_overlay` rows | Zero verified hashes is not itself a defect: verification is a separate operation |
| Root and descendant path status disagree about persistence | Home root reports `source_record: true`; Downloads and Projects report false | Root coverage is synthesized; source-store existence does not establish descendant coverage |
| Local cache status omits replicas | Mac: 7,361 path mappings and about 4.2 MiB estimated local index allocation; process RSS about 856 MiB | The index figure is not the daemon's memory use and is not a replica-memory measurement |
| Arena count exceeds addressable paths | titan: 2,121,658 arena slots versus 286,480 path mappings; about 282 MiB estimated local index allocation | Allocated slots must not be labeled as live files |
| Large replica refreshes repeat | Mac repeatedly loads a 1,759,478-slot snapshot in about 3.2 seconds | Repeated reconstruction is observed; attribute each transfer to a revision before claiming every transfer is unnecessary |
| Filesystem search is still in memory | `README`: 200 results in 34 ms, 199 from titan; `index_type: Ephemeral` | The query does not prove retained-store coverage |
| Search ignores the requested page size | The same probe requested `--limit 5` and returned 200 with a synthetic limit of 200 | Backend pagination is a confirmed defect already owned by the search refinement plan |
| Headline statistics have different scopes | `status` reports local file counts and zero peers; dedicated networking reports one connected peer; `library info` adds peer totals | Some differences are scope differences, and some are placeholder reporting; neither should be implicit |

Both devices have separate libraries named My Library. The older library-sync
setting is false, but peer source replication works independently. There are
no adapter sources or navigation pins in these libraries. These facts do not
require consolidation or automatic source creation.

## What the code establishes

The [September 16 volume audit](2026-09-16-volume-discovery-research.md) updates
the Untitled observation above: the current daemon sees it mounted but has no
active map or watcher, despite an older file at its expected snapshot path.
Startup and hot-plug use different paths. R2 and R5 must distinguish artifact
existence from active registration and restore state. An arena snapshot does
not require source registration.

### 1. A source root can change the meaning of a volume snapshot

[`SourceRegistry::volume_of`](../../core/src/ops/indexing/ephemeral/sources.rs)
uses the shortest registered source root on a volume as that volume's root.
[`EphemeralIndexCache::locate`](../../core/src/ops/indexing/ephemeral/cache.rs)
prefers this answer over the tracked volume's actual mount point. The slot is
still keyed by the same volume UUID.

Consequently, `/System/Volumes/Data` and a source at
`/System/Volumes/Data/Users/<user>` can select the same slot and snapshot file
while disagreeing about its root. `attempt_restore` deletes a snapshot when
its saved root differs from the resolved root and the new root exists.
The Mac log records this exact root mismatch repeatedly, including at
23:05 UTC on September 15.

Confirmed: inconsistent root selection and destructive mismatch handling.
Not yet proved: that this is how titan's `dev-tools` snapshot disappeared.
Its actual volume resolution and historical logs need a separate check.

### 2. The store can persist children without their real parent

[`SourceStore::write_loop`](../../core/src/ops/indexing/ephemeral/store.rs)
resolves `parent_uuid` through the in-memory ledger. An unresolved parent
becomes `None`, which is also the representation of a legitimate child of
the source root. `shallowest_first` sorts only the current observation batch.

[`discovery.rs`](../../core/src/ops/indexing/phases/discovery.rs) queues a
directory for another worker before sending that directory's own observation.
Children can reach earlier batches. Browsing a descendant also supplies its
contents without guaranteeing all missing ancestors have been persisted.
The arena creates ancestor nodes internally; that does not create source
records for those ancestors.

[`file::address`](../../crates/store/src/file.rs) reconstructs a parentless
file as its bare title. The content job joins that title to the source root.
This explains how a file beneath Downloads can be opened as though it lived
directly under home. A foreign key cannot reject a legal SQL NULL with the
wrong domain meaning.

Confirmed: the writer permits this shape, and it is present in the live
store. The exact historical ordering that produced each row is unknown.
Already-hashed parentless rows also require review: opening the wrong path
can succeed if an unrelated file happens to have the same name.

### 3. Queue drainage is being reported as durable success

In [`store.rs`](../../core/src/ops/indexing/ephemeral/store.rs):

- `send` logs a closed writer channel and returns no failure.
- `identify` returns identities before their rows commit.
- `commit` salvages a failed batch row by row, then clears all staged work,
  including rows that still failed.
- The ledger has already accepted those rows, so a later identical
  observation can resolve as unchanged and never retry the missing write.
- `flush` receives `()` even if commits failed. Content and error writes also
  log failures without returning them to the job.
- Pending-work queries turn database failures into an empty vector or zero.
  A hashing job can interpret a failed read as a drained queue.

Row salvage is useful and must remain. It needs a fallible acknowledgement,
durable retry evidence, and a ledger that distinguishes assigned identity
from committed state. A full walk must not sweep after an incomplete commit.

### 4. Store opening includes destructive schema recovery

[`SourceManager::apply_schema`](../../crates/store/src/source.rs) calls
`discard_unaddressable_generation`, which drops generation tables for an
older addressing shape. `SourceStore::open` calls `ensure`, so an apparent
read-side attachment can reach this path.

This contradicts the rule in
[Source Durability](../core/design/source-durability.md) that an origin cannot
be assumed reproducible. It is a confirmed code hazard, not a proven cause of
this instance's current record count. Adding more store reads must not spread
this behavior into search and recovery.

### 5. Replica discovery, artifacts and loaded arenas are conflated

[`sources.list`](../../core/src/ops/sources/list/query.rs) appends
`remote_shares()`. [`peer.rs`](../../core/src/service/mounts/peer.rs) inserts a
remote share only after downloading and loading its arena snapshot. It does
not rebuild the remote inventory from the snapshot files already on disk.
There is no persisted manifest carrying enough owner, source and generation
information to make those files discoverable independently.

A fetch failure skips a source. A cold restart therefore loses that source
from the visible list even if an older usable artifact exists. A failure
during a refresh happens to retain an already-loaded share, but that is not
a complete offline lifecycle. Owner removal and revocation also need explicit
reconciliation; an omitted or failed listing must not be interpreted as a
successful authoritative removal.

On the owner, watchers are armed on snapshot restoration or successful
indexing. Snapshot failure can therefore leave a registered, attached source
without a watcher. The one-shot restore gate caches failure for the session;
placing a repaired artifact on disk alone does not guarantee a retry.

### 6. Replica version comparison is not tied to the bytes delivered

[`ByteRangeProtocolHandler`](../../core/src/service/network/protocol/byterange.rs)
advertises a generation from snapshot file size and modification time.
`FetchSnapshot` may save a newer arena before opening the file. The response
header carries only its length. The receiver labels the downloaded snapshot
with the earlier generation from the listing.

This can cause an additional fetch of the same logical result. Meanwhile
[`add_entry_with_registry`](../../core/src/ops/indexing/ephemeral/index.rs)
marks an existing entry dirty even when its serialized metadata is unchanged.
The code has a per-device sync lock and a dirty-save guard already; the plan
must extend them rather than propose them as missing fixes.

Confirmed: generation/response race and unconditional dirtying. The fraction
of current traffic caused by those defects versus real writes needs revision
and transfer counters. The refresh loop and reconnect path both schedule sync,
which makes an immutable response identity particularly important.

### 7. Arena allocation survives removal and is serialized again

[`NodeArena::vacate`](../../core/src/ops/indexing/ephemeral/arena.rs) replaces a
node but retains its slot. `iter` and `len` include every allocated slot.
[`snapshot.rs`](../../core/src/ops/indexing/ephemeral/snapshot.rs) serializes
all of them and reconstructs all of them. Repeated subtree clearing can grow
the stored and restored allocation independently of live paths.

[`ephemeral_status`](../../core/src/ops/core/ephemeral_status/query.rs) sums
only `cache.all_indexes()`, which excludes peer arenas, and exposes arena
length as `total_entries`. Both lifetime and reporting need repair. Process
RSS must remain a separate measured quantity, since it includes much more
than arena allocation.

### 8. Search can only query the loaded part of the inventory

[`search.files`](../../core/src/ops/search/query.rs) uses local and remote
arenas. [`sources.search`](../../core/src/ops/sources/search/query.rs) uses
the adapter-only registry projection. The filesystem schema has no declared
FTS fields. Store discovery currently goes through `SourceStore`, which loads
the entire identity ledger and starts a writer, so reusing it for 100 cold
source queries would still allocate a large amount of memory.

The current file-by-ID, file-by-path and directory readers also depend on
arenas. Adding search hits from SQLite without implementing these follow-up
reads would create results you cannot open or inspect.

## Proposed contract

The source store owns retained records and assertions. The volume arena is a
fast filesystem view and may include paths no source retains. Neither
representation needs to contain the other's entire scope.

The agreed read strategy keeps a suitable loaded arena as the fast path.
Use SQLite when the arena is absent, lacks the requested coverage, or cannot
answer the query. Retained metadata must remain browsable, searchable and
inspectable after restart with its arena unloaded and its origin offline.
Those reads use the saved capture; opening file bytes still depends on an
available origin or retained copy.

| Concern | Authority | Derived representations |
|---|---|---|
| Source membership, owner and retained root | Existing source registry | Sidebar, CLI list, peer manifest projection |
| Committed records, content evidence, assertions | Source store | Search indexes, arena hydration, portable replica generation |
| Live mapped filesystem state | Volume arena and current observations | Local restart snapshot, hot search results |
| Whether bytes can be read now | Current origin/byte-provider state | Availability shown beside retained metadata |
| Whether a scan covered a subtree | Committed capture evidence and explicit exclusions | Health, path status and completeness reporting |
| Whether a replica is usable | Persisted validated generation plus owner metadata | Optional loaded arena |

Preserve these rules throughout the work:

1. A missing cache never removes a source registration or destroys a store.
2. Reconstructing an arena from retained records is allowed without the
   origin. Replacing retained records by walking the origin is a separate,
   explicit operation.
3. A volume's identity and mount root come from the volume resolver. Source
   registration order cannot redefine them.
4. `parent_uuid = NULL` means an actual child of the source root, never an
   unresolved ancestor. The source root may remain a boundary without its own
   record; represent that case explicitly in status.
5. Stable record UUIDs survive restoration, repairs and compaction. Arena
   `EntryId` values are internal to an arena generation.
6. Assigning a UUID, displaying an observation and committing a record are
   different events. Only a commit acknowledgement can establish durability.
7. Assertions survive generation repair and replica replacement. No whole
   database overwrite may erase receiver-owned assertions.
8. Missing, filtered, summarized, inaccessible and never scanned are distinct
   coverage states. None proves deletion.
9. A replica has an owning device and a validated generation even when no
   arena is loaded. Origin availability is separate from metadata usability.
10. Search reports its participating sources and coverage. A partial result
    cannot claim a complete fleet count.
11. `Untitled` continues to work without a source store. Pins continue to
    have no indexing or watcher ownership.
12. Filesystem and adapter sources keep one record-store shape and shared
    read services. Ingest and optional hot caches may differ.
13. Choose one primary read backend per source per request, using coverage
    and query capability. An empty result does not trigger a second backend.
    Loading an arena is never a prerequisite for querying a readable store.

## Execution register

All implementation slices are proposed and unstarted. The dependencies below
describe implementation order after review; they are not permission to repair
the production stores while this draft is being discussed.

| Slice | Scope | Depends on | Exit proof |
|---|---|---|---|
| R0 | Preserve evidence and establish fixtures | Review | Reproducible failures and read-only baselines, no production mutation |
| R1 | Correct commits, ancestry and migrations | R0 | Failed writes cannot complete successfully; retained state survives repair |
| R2 | Stable volume roots and recovery lifecycle | R0; R1 for store hydration | Missing snapshot no longer strands a source or changes identity |
| R3 | Reliable replica inventory and generations | R0; R2 for owner recovery | Offline restart preserves inventory; unchanged generations do not reload |
| R4 | Arena compaction and honest memory accounting | R0, R2 | Live data preserved with bounded allocation and explicit scope counters |
| R5 | Source health and capture coverage | R1–R3 | CLI, path status and UI agree about what is retained and usable |
| R6 | Arena fast path, direct store fallback and portable store replicas | R1, R3, R5; coordinated with search plan | Equivalent reads across suitable arenas and stores; 100-source search without 100 arenas or ledgers |
| R7 | Controlled recovery of this instance | R1–R5; R6 for full cold-read proof | Home ancestry repaired; all nine titan sources usable on Mac after restart |
| R8 | Release acceptance and documentation | R1–R7 | Fault, compatibility, scale and live matrices pass |

### R0: Preserve evidence and build the failing cases

Capture owner and client views with timestamps and build identity. Use
structured CLI output, and make the inspection tool omit credentials from
mount responses. Record source IDs, volume anchors, capture policy, committed
counts, restored roots, watcher roots, failed jobs and replica generations.
Add build revision and dirty-build identity to diagnostic output so the next
comparison can establish which code is actually deployed.

Make consistent SQLite backups through the backup API before migration or
repair. Preserve WAL contents; a raw copy of a live `.db` is not the baseline.
Keep current and dated NAS freezes intact. Record counts and hashes of the
backup artifacts. Do not run `ensure` or schema application as part of a
read-only audit. Do not treat the current `freeze` command's successful
return as proof that all preceding queued writes committed until R1 lands.

Build small synthetic fixtures from the observed failure shapes:

- A home-like source whose Desktop and Downloads ancestors are absent, with
  nested files and a same-name file at the root.
- A child observation arriving one batch before its parent, plus a shallow
  browse inside a source whose store is initially empty.
- One volume, two nested sources, and both APFS path spellings. Vary source
  registration order and which path first creates the arena slot.
- A valid store with a missing, truncated, incompatible, or wrong-root arena
  snapshot. Repeat with the origin detached.
- A peer with nine registrations, eight successful artifacts, an older ninth
  artifact on the receiver, and no owner connection on the next restart.
- Repeated add/delete and subtree-reindex cycles over a bounded live tree.

Use synthetic names and bytes in committed fixtures. Keep personal database
copies and unredacted daemon logs outside the repository.

### R1: Make durable writes and addressing trustworthy

Establish separate assigned and committed state in the writer. An observation
may receive a stable identity promptly, but failed writes remain pending or
explicitly failed; the ledger cannot classify them as durably unchanged.
Choose a sequence-based acknowledgement so `flush` can report whether every
write before its barrier committed, including partial salvage failures.

Return `Result` through channel send, flush, content updates, pending-work
queries and final job output. A dead writer, SQLite failure, or unaddressable
claim must produce a visible failure or partial outcome. It must never look
like an empty queue. Preserve successful salvage rows, report failed IDs and
paths, and retain enough retry evidence to repair them without restarting the
daemon. Bound retries and apply backpressure instead of dropping data.

Use one writer instance per source, including concurrent first-open calls.
Audit `store_for`: it opens and spawns before inserting into the store map,
so simultaneous callers can perform duplicate schema/open work and start
temporary independent writer tasks before one wins the cache insertion. This
does not establish that two writers currently ingest the same observations.
Add a per-source initialization gate with an explicit failure/retry lifecycle.

Make parent resolution a prerequisite for committing a descendant. A batch
must carry its required ancestor observations or the writer must resolve them
from committed records and validated filesystem observations. Queue an
unresolved child without pretending it lives at the root. Bound the pending
ancestry set and fail the scan explicitly when it cannot be resolved. Sorting
one batch is useful but cannot be the correctness condition.

Make a repeat observation repair structural fields even when size and mtime
are unchanged. Validate that a directory's stored path agrees with its parent
and title. Add an explicit unresolved-address diagnostic for legacy malformed
rows. Prevent hashing, duplicate decisions and byte reads through such rows.

Hash against validated record observations. Before accepting a result, verify
that the record still names the observed file and that relevant size, time,
kind and identity evidence has not changed during the read. A concurrent
rename or modification must leave work pending against the newer observation,
not attach an old hash to it. Track sampled-work and full-verification errors
separately enough to retry the intended stage.

Do not automatically clear all 806 errors. After fixing ancestry, revalidate
the affected addresses and invalidate only content evidence whose association
cannot be trusted. Recheck parentless records that already have content IDs.
Keep actual permission and missing-origin errors as observations with a time
and retry disposition.

Replace destructive schema opening with read-only inspection and versioned,
data-preserving migrations. The old full-path schema contains evidence from
which parent addressing can be migrated without walking the origin. Stage and
validate that conversion, preserve assertions and original IDs, and retain the
original if conversion fails. Unsupported versions remain visible and intact.

Finally, tie scan completion and sweep eligibility to successful commit
barriers and enumerated coverage. A failed row, cancelled pass, blocked
ancestor, or uncertain coverage prevents a destructive sweep of that scope.
Keep the existing large-sweep fuse as an additional safeguard.

Define whole-source coverage by the actual source-relative scan boundary and
policy, not by `is_volume_indexing`. The current predicate couples sweeping
to whole-volume jobs and cannot establish a complete retained subtree pass.
Do not clear the last queryable arena before a replacement walk succeeds:
stage the replacement or keep the validated store view available throughout
the walk. Watcher observations that arrive during a staged scan must be
ordered against the scan's commit boundary and replayed before publication.

Proof: induce closed channels, lock contention, transaction failure, one bad
row, a failed parent, and a crash between assignment and commit. Assert exact
surviving rows and IDs after recovery. A queue read error must fail the job.
An unchanged second observation must repair a previously failed write.

### R2: Fix volume identity and recover from retained stores

Resolve volume identity, mount root and source-relative scope independently.
Anchored sources use the volume manager's mount root, including its offline
identity record. Remove the shortest-source-root calculation for anchored
volumes. Preserve the pinned volume-index namespace; changing directory names
is not a repair. Define one fallback for unanchored roots and test nested
registration changes without silently moving existing artifacts.

Store snapshot paths relative to the volume boundary in the next format.
Carry volume identity, schema version, generation, capture coverage and a
payload checksum. Normalize APFS aliases through the existing volume resolver;
do not infer identity from a path existing. A genuine remount rebases the
view while preserving UUIDs. A different drive at the same mount path must
never inherit the old drive's records.

Replace the boolean, once-only restore result with an explicit lifecycle:
unloaded, loading, ready, unavailable with reason, and retryable after a
relevant artifact, mount or migration change. Coalesce concurrent loads.
Move decompression and reconstruction off the async runtime. Ordinary queries
must not repeatedly parse an artifact known to be unsupported or corrupt.

Preserve incompatible or corrupt snapshots for diagnosis until a replacement
is validated. A missing or unusable cache first tries a read-only view of the
retained store. For warm browsing, construct an arena projection from valid
records using their UUIDs. Do not load every source merely to make it visible.
Hydrated coverage is exactly the coverage retained in the store, not proof of
a complete volume walk. If both store and cache are unusable, expose that
condition and retain both artifacts.

Reconcile watcher ownership from current source policy and mapped live roots,
independently of whether restoration emitted an event. Register an active
watch only after the OS accepts it. An offline source is not watched; a failed
subscription reports its reason. Preserve permission-aware startup: no full
walk or macOS permission prompt is triggered as a hidden consequence of a
search or failed cache load.

Proof: missing-cache and offline-store fixtures remain browsable; identical
IDs survive remount and alias changes; registering a nested source never
changes the volume root; an unreadable snapshot remains on disk; a repaired
artifact can be retried without restarting the daemon.

### R3: Separate peer inventory from loaded representations

Persist peer source metadata through the existing registry boundary. Extend
that boundary with owner identity, source identity, source-relative root,
last successful listing, artifact generation, supported formats and replica
health. These are administrative source fields, not a second file model.
Keep record and assertion data in source stores. Local registrations and peer
replicas must be enumerated through one service, whether loaded or not.

On startup, restore the known peer inventory without contacting the owner.
Validate cached artifacts lazily or within a small background budget. Failed
refresh preserves the last validated generation and reports staleness. A
source whose cache is missing remains listed as unavailable, including its
last known owner count. Report origin reachability separately from whether
its metadata can be queried.

Refresh owner metadata, display name and attachment observations even when
the record generation is unchanged. The current unchanged-generation branch
skips updating `RemoteShare.info`, so a payload optimization must not freeze
the source's availability state.

Record explicit source removal and grant revocation. A failed request or
partial enumeration is not deletion evidence. Define cache retention after
revocation separately from permission to fetch bytes or new metadata; do not
silently continue authorized access after revocation.

Give each response the identity of the artifact actually transmitted. The
owner opens or creates one immutable generation, then returns its generation,
format, length, checksum, source boundary and coverage in the header. The
receiver validates a temporary artifact before publishing it. It records the
header's generation, never a generation read from an earlier listing. Bound
compressed and decoded sizes and clean up failed temporary transfers.

Make dirty state reflect changes to serialized data. Compare the packed
metadata and relevant identity or summary fields before advancing a revision.
Do not suppress legitimate ownership, type, timestamp, rename, or size changes
merely because one current UI field did not change. Separate record-generation
changes from runtime access bookkeeping.

Retain the existing per-device sync lock. Coalesce refresh and reconnect
requests, track failure backoff per source, and avoid an unavailable source
creating a repeated load or warning loop. Expose generations advertised,
received and served, bytes transferred, skipped unchanged generations, retries,
and decode time. One corrupt source must not block its healthy siblings.

For old peers, negotiate the arena format and retain the prior compatible
reader. Do not claim equivalent fields the old header cannot provide. Persist
enough provenance to reopen existing cached snapshots without manufacturing
new source identities.

Proof: restart offline with nine known sources and eight fresh artifacts;
all nine remain visible and the older ninth is usable if valid. A listing/fetch
race publishes the generation actually received. With the owner unchanged,
30 minutes of refreshes, past the store pool's idle timeout, transfer zero
additional payload and reconstruct zero additional arenas. Inject a real
mutation to prove it still propagates.

#### R3 results, 2026-09-19

A nested source's generation is now its store's revision instead of the size
and mtime of `data.db` and its WAL. Those files move whenever the store's pool
drains: sqlx closes idle connections after 10 minutes, SQLite deletes the WAL
when the last one closes, and the next read recreates it. That re-copied the
Mac's unchanged home store to titan four times in one night, each round a
736 MB transfer and a 57-minute arena rebuild. The R6 stability check watched
twelve intervals, six minutes, which ends inside the idle timeout. The
evidence, the revision design and its measured write cost are in
`2026-09-19-incremental-replication.md`, which also carries this slice's
temporary-file cleanup and the delta replication R6 registered.

### R4: Compact arenas and expose memory honestly

Expose live path count, allocated slot count, vacant slot count, capacity,
estimated allocation, snapshot bytes and restore time separately. Include
local volumes, scratch and peer arenas, with subtotals and a clear total.
Keep process RSS as a separate process measurement; do not present the sum of
estimates as resident memory or add overlapping views of one arena twice.

Implement a compact projection from the live graph. Rebuild parent/child
links, reverse paths, names, UUID bindings, content kinds, collection flags,
summary stubs and rollups together. Preserve record UUIDs, byte totals and
query results. Audit any consumer retaining an `EntryId` across locks or
generations before publishing a compacted arena.

First use the compact projection for new snapshot artifacts. Then add bounded
background replacement of inflated live arenas. Build against an observed
revision and publish only if it still matches, or replay a bounded mutation
log; never replace newer writes with a compacted older view. Readers must
finish against their valid generation. Cap temporary memory and defer work
when compaction would exceed the budget.

A reviewable initial trigger is an arena with at least 100,000 slots and more
than twice as many allocated slots as live nodes. Tune it with measurements;
the acceptance criterion is bounded growth for a fixed live tree, not this
particular threshold. Never reuse an old slot without the generation contract
that makes stale references safe.

Proof: after 100 add/delete or subtree-reindex cycles, the compacted live
shape and results equal a freshly built index. Snapshot node count follows
live nodes, including required ancestors and summaries, rather than historical
allocation. Concurrent mutation and active-reader fixtures preserve identity.

Results (2026-10-07): the compaction half landed in
`core/src/ops/indexing/{nodes,arena,snapshot}.rs`. `NodeArena` reuses vacated
slots through a free list, so a cleared and refilled subtree reuses the slots
it had (100 cycles over 50 files: 54 slots for 54 live paths, where 5,154 were
allocated before). `Arena::compact` is the compact projection, run after a
removal once vacant slots outnumber live entries and number at least 4,096; it
renumbers every internal reference together and swaps in a fully built arena.
The audit found no consumer retaining an `EntryId`: the type never leaves the
indexing module, listings and search page by path, the watcher addresses by
path, so there is no generation contract to publish and no reader to drain;
compaction runs inline under the partition's write lock. Snapshot format v4
writes the dense projection. `ArenaStats` now reports live entries, allocated
slots, vacant slots and capacity separately; surfacing them on
`core.index_status` with the other memory figures is the remaining half.

### R5: Report source health and capture coverage consistently

Add a typed source inspection result shared by CLI, source cards, path status
and diagnostics. Prefer extending the existing operation family to adding a
separate management subsystem. Include these independent facts:

- Owner, origin availability, and the time that availability was observed.
- Store existence, readable schema, committed revision and structural health.
- Scan scope, capture policy, last complete pass, exclusions, summaries,
  denied coverage and interrupted or failed work.
- Arena readiness, revision, scope and restore failure, if any.
- Snapshot existence, validation result, generation and age.
- Replica generation, owner generation, last successful refresh and error.
- Desired and actual watcher state with any subscription failure.
- Committed records, files, directories and symlinks; pending identification,
  sampled identities, verified copies and failures by stage.

Use a compact user-facing summary such as ready, partial, offline with retained
metadata, or needs repair, backed by those typed fields. `idle` remains a job
activity fact and cannot stand in for health. An empty but completely scanned
source must remain distinguishable from a store never populated.

Put capture evidence in source-store metadata and commit it with the data it
describes. Keep registry counters as a projection of an acknowledged revision.
Do not update source counts only when a snapshot happens to be saved, and do
not substitute whole-arena counts for an empty source. Store existence,
root-boundary coverage and a committed record at the exact path must remain
separate in `paths.context`. An ancestor marked indexed does not prove a
descendant was enumerated.

Define scope on every aggregate. Local owned counts, cached replica counts,
owner-reported fleet totals and current searchable coverage are useful but
different. Use the same calculation on query and `ResourceChanged` paths.
Fetch actual networking state in the top-level status operation. Label device
counts by scope, and retain peer observation times.

Until `catalog.db` exists, sums of source content counts cannot be labeled as
globally unique content. Nested sources, repeated replicas and shared physical
capacity must not silently inflate totals. State which quantities are sums
and which are deduplicated, and by what identity.

Regenerate TypeScript types for public Rust type changes. Adapt existing
typed hooks and CLI output; do not redefine backend types in the frontend.

Proof: the same source ID and revision produce compatible CLI, card and path
status. An attached source with a missing cache is visible and degraded. A
failed watcher is never reported active. Each reported count reconciles to
its named source set and revision.

### R6: Keep the arena fast path and add direct store reads

Keep routing behind the existing query operations and share their result
contract. Select an arena when it is loaded, covers the requested scope and
supports the query's matching, filters and ordering. Otherwise, read the
source's SQLite store. A shallow or partially mapped arena cannot answer a
complete source search merely because it exists. Retain explicit capture
freshness and coverage whichever backend answers.

Choose one primary backend for each source for the duration of a request.
Do not search both representations and reconcile their hits. An empty result
from a suitable arena is valid and must not trigger a SQLite retry. Reads
requiring store-only fields can use SQLite directly. Batched enrichment of
arena candidates is also allowed, but any fields affecting filtering or
ordering must be resolved before final pagination.

Reuse `SourceDb` and its existing path, record, content and overlay reads in
`crates/store`. Add the missing file and directory queries there, plus a
read-only opening path for supported schemas. Opening for reads must not
create a writer, run migrations, load `Ledger`, or hydrate an arena. Bound
connection and statement caches; let SQLite load the database pages needed
by each query. This needs a shared read contract and a coverage/capability
check, with no cost-based query planner or automatic arena loading.

Route reads by capability:

| Requested data | Available representation | Read path |
|---|---|---|
| Filesystem records, local or replicated | Loaded arena covering the requested scope and supporting the query | Arena fast path, with freshness and coverage attached |
| Retained filesystem records without a suitable arena | Readable source store | Direct indexed directory, record and search queries |
| Fields unavailable in an otherwise suitable arena | Readable source store | Direct SQL query or bounded enrichment before filtering, sorting and pagination |
| Adapter records | Readable source store | Shared record read service with adapter facets |
| A peer source without a suitable arena | Validated local store generation | Same direct store reads as an owned source |
| Older peer with only an arena artifact | Compatible cached snapshot | Explicit compatibility path, loaded within the cache budget |
| Bytes for a retained record | Available origin/provider | Existing byte-provider path; offline metadata remains usable |

Implement directory listing, file-by-ID, file-by-path and relevant inspector
reads before advertising cold-store search results. Return the same generated
`File` contract with owning-device addressing. Preserve record identity when
switching between representations. Keep record freshness separate from byte
availability so stale metadata does not authorize a destructive action.

Library search enumerates the source registry and mapped local volumes, not
only `all_indexes()` and `remote_shares()`. A 100-source search can use five
suitable loaded arenas and 95 SQLite stores. Query a bounded number of stores
at a time, cancel obsolete requests, and merge bounded candidate windows
through one result pipeline. Each source contributes through its selected
backend. Scope volume arenas to the participating source or unretained mapped
region. Deduplicate identities shared by nested sources and replicas, and
keep source provenance available to callers.

Preserve the existing Unicode case-folded substring semantics. SQLite FTS
token or prefix matching is not an equivalent replacement. Benchmark a
normalized record-local candidate index, potentially trigram-based with an
exact substring check, against simple bounded scans. Choose and document the
index only after verifying semantics and update cost. Any index belongs with
the record store, and both filesystem and adapter read paths use the shared
contract where their fields support it.

Apply filters and one deterministic global sort before final pagination.
Use stable identity as a final tiebreak. Distinguish an exact total from a
partial or budget-limited count; never silently truncate at 200 and claim it
is complete. Report unavailable or unqueried sources. Offset or cursor paging
must define its behavior when participating generations change between pages.
Coordinate this implementation with the existing search refinement plan.

Land direct local reads and the routing rule first, then bounded cross-source
search. Peer database delivery is a subsequent slice using the same reader;
local SQL reads do not depend on changing the replication protocol.

Add a negotiated portable source-generation format for peers using the same
SQLite record schema. Produce a consistent export through the backup API or
equivalent supported snapshot path. Include source identity, generation,
schema, coverage and checksums. Export exactly the authorized source scope;
a volume arena may include unrelated sibling paths and is not the portable
source boundary.

Validate a received generation before installation. An initial read-only
replica can use an immutable database artifact. Subsequent generation changes
must preserve receiver-owned assertions. If assertions share the installed
database, replace only generation data in a transaction; do not rename a new
whole file over an active pool or discard local edits. Assertion merge follows
the HLC/device rules and FD2. Generation transport must not invent a second
last-writer-wins assertion system. Existing assertions on unsupported formats
remain preserved with their compatibility status.

Whole-generation transfer is sufficient for a first correct implementation.
Delta replication is optional follow-on work once revision counters show the
actual transfer cost. Direct cold queries provide the memory benefit without
requiring an incremental replication protocol first.

Proof: run the same fixtures and captured revision through the arena and SQL
readers, comparing identities, matching, filters, ordering and pagination.
Assert that a suitable loaded arena is selected, that its empty result causes
no SQL retry, and that insufficient coverage or query support selects SQL.
Verify store-only enrichment affects filters and ordering before pagination.

Query 100 synthetic retained sources containing at least one million records
in aggregate, first with all arenas unloaded and then with five suitable
arenas loaded. No identity ledgers or writer tasks may be created by reads.
Assert global result equivalence, pagination, offline browsing and bounded
connection concurrency. Measure peak and steady memory,
latency, descriptor count and cancellation. Proposed initial query working-set
budget: 256 MiB above the idle fixture, excluding unrelated preloaded arenas;
confirm or revise it from the benchmark before making a product promise.

#### R6 results, 2026-09-16

Landed as `9e29e7c46` (read-only store path), `4cf25c17c` (routing), and
`5f1fdd287` (database delivery), verified live on the fleet.

- `SourceManager::open_read_only` and `open_file_read_only` open without
  DDL, migrations, ledger or writer; the unaddressable-generation refusal
  still applies. `crates/store/src/read.rs` carries the entry projection,
  children/by-path/by-uuid lookups, a paged full scan, and case-folded
  substring search whose semantics match the arena's registry exactly —
  folding happens in Rust over a keyset scan, since SQLite's `lower()` folds
  ASCII only. Totals keep counting past the hydration cap and the search
  output carries `total_is_exact`.
- Routing: one backend per source per request. `arena_answers` (restored
  partition or a walk covering the scope) selects the arena; otherwise the
  store answers, and an empty result from the selected backend is final.
  Library search enumerates the registry past the loaded partitions, scoped
  search falls back the same way, and a detached source whose snapshot did
  not cover a directory lists it from its store. The equivalence fixture
  runs one capture through both backends and asserts identical matches,
  filters and scores, accents included.
- Database delivery: a nested source is advertised as such and its replica
  travels as a `VACUUM INTO` export of exactly its records, with the same
  header identity contract as snapshots (generation from the live database
  and WAL, blake3, length). The receiver opens the artifact read-only and
  rebuilds an arena from its rows before anything is replaced; restores
  prefer a snapshot and fall back to the database; an owner too old to
  export answers on the snapshot path. Verified live: dev-tools now ships
  as an 11.3 MB database instead of the pool volume's arena snapshot, the
  sibling leak is gone, the rebuilt replica lists and searches identically,
  the stale snapshot artifact was removed, no export temp files linger, and
  the generation held stable across twelve refresh intervals.
- Scale: `crates/store/tests/scale.rs` at the plan's 100 stores and one
  million aggregate records answers a cold sequential fan-out in 1.7 s,
  slowest single store 21 ms, all hundred opens in 138 ms, no ledgers or
  writers created. The 256 MiB working-set budget is met with room; the
  trigram/candidate index stays deferred with these numbers as the bar it
  must beat.

Registered follow-ons, per this plan's own text: delta replication
(optional once revision counters show transfer cost), per-request
cancellation of obsolete store queries, database delivery for non-nested
sources (their arena snapshots have the correct boundary already), and
receiver-owned assertions on replica databases — whole-file replacement is
safe today because replicas open read-only and assertions live with the
owner, which is the constraint to revisit when FD2 moves them.

### R7: Repair this instance after the mechanisms pass

Treat the live stores as evidence. No blanket delete-and-reindex recovery.

1. Re-capture both deployed build identities and source health, and create
   verified consistent backups. Preserve the dated NAS freezes separately.
2. Validate titan's nine stores directly, including ancestry, path resolution,
   identity and generation metadata. Owner registry counts alone are not
   sufficient. Record the exact reason `dev-tools` has no owner snapshot.
3. Restore `dev-tools` service from its retained store or a validated existing
   artifact, preserving UUIDs. Do not require a new NAS walk for cache repair.
   Reconcile its watcher only after confirming the live volume identity.
4. Repair the Mac's home ancestry from unambiguous retained evidence. Use a
   dry-run report of affected rows and hashes. If a row's original path cannot
   be recovered, retain it unresolved; never guess from a filename match.
5. If fresh capture is needed, show the proposed home scope and capture policy
   and run an explicit reconciliation pass with the fixed writer. Keep the
   prior committed generation until new coverage and writes are validated.
   Preserve UUIDs using valid existing evidence, and report any ambiguity.
6. Retry content work only after the affected addresses pass validation.
   Recheck successful hashes whose old parentless addresses were suspect.
   Keep unrelated accepted NAS read errors intact.
7. Verify all nine titan sources on the Mac, including after restarting the
   Mac while titan is unavailable. Confirm metadata search and browsing, then
   reconnect and verify an actual byte read and a remote operation.
8. Measure unchanged refreshes and arena memory. Keep `Untitled` mapped
   without a source, and verify its search behavior still works.

The deployment/recovery step is a later task after review. Record results
against each source ID and generation. Do not mark the NAS archive itself
complete again from the number of visible source cards.

#### R7 results, 2026-09-16

Both machines run build `4a690c84a` (Mac debug daemon; titan
`x86_64-unknown-linux-musl` release at `~/spacedrive/bin/sd-daemon`, previous
binary kept as `.prev`). Checks against the stores used read-only SQLite over
each `data.db`.

Titan's nine stores validated directly. Every store: zero records whose
`parent_uuid` points at a missing row, zero directories without a
`directory_path` row, and the parentless rows are exactly the source root's
own top-level children (dev-tools: 14, matching its listing). Totals:
jamie-nas 217,890 (+2: the deploy staged `sd-daemon.new`/`sd-server.new`
into `~jamie/spacedrive/bin`, which lives inside the source; the rows went
stale when the swap happened while the daemon was down and will sweep on the
next walk — they are also the only pending-identification rows in the fleet),
calvin-nas 1,525, cctv 49,263 (748 accepted root-owned `content_error` rows),
footage 1,380, dev-tools 28,079, jamie-public 2, jamvm/vm-data/windows-vm 1
each. dev-tools' missing owner snapshot was the explicit-discovery gap plus
the empty-partition fuse; `5a65d0998`'s heal walked it and its first complete
snapshot now serves.

The Mac home store validated the same way: 1,759,739 records (1,507,147
files, 221,511 directories), zero dangling parents, zero directories without
paths, zero pending identification, 3 `content_error` rows. Its 217
parentless rows are the home directory's top-level children. No surgical
repair was needed; R1's rebinds plus the re-walk resolved the malformed
ancestry entirely, so steps 4-6 close with no unresolved rows.

Live fleet proofs on this build:

- Startup restores all nine arenas on titan in about a second and arms all
  nine watchers, dev-tools included (`8c8499413`, `847144ba0`, `3ee5449bf`).
  The pool volume watch still fails on the root-owned
  `ix-applications/docker` directory, and the per-source fallback covers
  every registered source under it.
- Cold restart with titan stopped: the Mac lists all nine replicas from disk
  and searches them (dev-tools answered `zig-linux` in 121 ms while the owner
  was down). Titan relaunched and reconverged with zero transfers.
- Byte read: streamed `jamie-public/.spacedrive-volume-id` bytes through the
  local WebDAV share, served from titan's disk. Remote operation:
  `sd --device 3cee5a0b… sources list` executed on titan; remote log stream
  connected.
- Generations recorded per source in
  `~/.spacedrive/mounts-remote/3cee…/manifest.json` name the delivered
  artifacts (`98e8a1485`); the restart settle after both daemons moved to
  this build transferred nothing on either side.
- Refresh economics: a dirty listing over an unchanged artifact paces at
  five minutes (`4a690c84a`), which ended titan re-pulling the Mac's 116 MB
  home artifact once a minute during builds. Memory for R4's ledger: Mac
  daemon 2.70 GB RSS (own 1.76 M-entry arena plus nine replicas), titan
  2.14 GB RSS (nine own arenas plus the Mac home replica).
- Untitled: mounted, unregistered, and its files answer search through the
  data-volume spelling of `/Volumes`. Its own partition is not active this
  session; the hot-plug mapping gap is recorded in
  `2026-09-16-volume-discovery-research.md`.

### R8: Acceptance, release and documentation

Run the smallest meaningful checks for each implementation slice, then the
combined matrix. Use `crates/store/tests/files.rs` and `record.rs`, colocated
source-store/cache tests, the indexing harness, watcher tests, and new peer
protocol fixtures. Do not make acceptance depend on personal machines or an
available NAS.

| Scenario | Required result |
|---|---|
| Child before parent across batches | Correct parent relation and path; no accidental root record |
| Shallow browse inside an empty retained source | Required ancestry persisted or write explicitly pending |
| One failed row and successful siblings | Siblings retained; exact failure reported; job not falsely complete |
| Writer dies or pending query fails | Typed failure; no success-shaped empty queue |
| Restart between identity assignment and commit | Stable identity recovered and missing write retried |
| Interrupted or partially denied scan | Last good records preserved; no unsupported sweep |
| Old schema with origin offline | Data-preserving migration or readable compatibility status; no table discard |
| Same volume, nested roots, reversed registration order | One stable volume boundary and shared file identities |
| Remount and APFS alias changes | Same source and record IDs; usable retained metadata |
| Missing or invalid restart snapshot | Source remains visible; store reads work; artifact retained for diagnosis |
| Failed watcher subscription | Inactive/error reported accurately and retry reconciled |
| Offline client restart | Known peer inventory restored; validated cached metadata usable |
| One failing source among nine | Nine visible; eight fresh and one stale/unavailable with reason |
| Listing/fetch generation race | Receiver records the delivered artifact's actual generation |
| Unchanged owner for ten intervals | No payload transfers or arena reconstructions after convergence |
| Continuous real writes | Bounded refresh work; eventual newest generation; no lost committed records |
| Repeated subtree clear and refill | Compaction bounds allocation; UUIDs and results preserved |
| Cold search across 100 stores | Bounded reads and memory; no arenas or ledgers required |
| Same capture read through arena and SQLite | Equivalent identities, matching, filters, ordering and pagination |
| Suitable loaded arena returns no matches | Arena selected; no SQL retry |
| Loaded arena has insufficient coverage or query support | SQL selected for the source request; limitations remain explicit |
| Five suitable arenas and 95 stores | One backend per source; same global results and pagination as store-only search |
| Arena candidates need store-only filter or sort fields | Enrichment applied before final filtering, ordering and pagination |
| Search with requested limit five | Five results with honest total, scope and page metadata |
| Same record in multiple representations | One logical hit with correct owner and provenance |
| Replica replacement with local assertions | Assertions preserved and merged through the shared contract |
| Mapped volume with no source | Fast browse, watcher and snapshot work without registration |
| Status query and resource event | Same scope definitions, counts and observation times |

For Rust changes run focused tests, applicable core/store/CLI checks and
`cargo fmt`. For public types regenerate the TypeScript client; check changed
hooks and the desktop production build. Preserve the documented build baseline:
the full interface typecheck already has dependency/SVG declaration failures,
and macOS cannot use a blanket workspace/all-targets check for Windows-only
packages. Record pre-existing failures separately.

Add a two-process compatibility matrix: old owner/new receiver, new owner/old
receiver, and new/new. Unsupported capabilities must produce an explicit
fallback or limitation. Restart and partial-transfer tests must include actual
process termination, not only clean shutdown.

Update the indexing, source durability, CLI, mounts and search docs after their
behavior changes. Document that an arena snapshot is optional acceleration
for retained sources, and that portable records remain useful without one.
Replace the dated project-status claims with measured acceptance results and
the build identities that passed them.

## Coordination with existing plans

[Search Refinement](2026-09-15-search-refinement.md) owns controls, navigation,
filters, sort and pagination. Its current arena-only boundary is valid for its
first UI slices but conflicts with R6 as a final architecture. On approval,
amend that boundary explicitly and share one backend result pipeline. Its
proposed legacy tag join must also be coordinated with FD2; do not grow a
long-lived entry-backed tag path to satisfy these tests. Unsupported filters
must not silently pass all results.

[Entries Final Drop](2026-09-15-entries-final-drop.md) owns assertion migration
and removal of legacy rows/schema. R1 and R2 address source correctness without
requiring that drop. R6's record reads do not require `catalog.db`. FD2 remains
the gate for multi-writer tag assertions and for removing their legacy home.
The final drop must rerun the peer and source-recovery matrix from this plan.

[Source Convergence](2026-08-22-source-convergence.md) owns the one-store and
one-registry direction. This plan makes that direction operational under
restart, partial failure and unloaded caches. Retain `ephemeral` compatibility
names until the agreed teardown slice removes them; rename cleanup does not
fix any failure above.

## Decisions to refine in review

Read routing is agreed: use a suitable loaded arena first and SQLite when
coverage or query support is insufficient, through the same operations and
result contract. R6 records the routing and acceptance criteria.

1. Adopt portable record generations as the long-term peer format, with arenas
   optional and negotiated. Recommended: yes. Keeping only arena transport
   would leave the 100-source memory problem in place on the receiving node.
2. Prioritize R1–R5 and the missing-replica recovery before the broader cold
   query path. Recommended: yes. These repair existing promises without
   waiting for new search capabilities.
3. Decide the exact source overlap rule. Current writers often select one
   innermost store, while a source walk can retain the outer subtree too.
   Recommended: each explicitly retained source describes its own declared
   scope, with shared record UUIDs and deduplicated projections. Audit writer
   routing before claiming nested source completeness.
4. Keep whole-generation replication first and add deltas only after measuring
   real update cost. Recommended: yes. Immutable, verifiable publication and
   offline recovery matter before optimizing the transport.
5. Choose the cold-search candidate index and memory budget from the R6
   benchmark. Keep substring semantics and honest coverage as fixed
   requirements; do not promise instant arbitrary searches without evidence.

Completion requires both structural repair and proof of behavior. A source
count, a successful job row, or a working warm search alone cannot close this
plan.
