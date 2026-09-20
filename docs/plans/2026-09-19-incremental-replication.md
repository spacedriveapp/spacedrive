# Incremental Replication

> Status: proposal. P0 landed 2026-09-19.
> Captured: 2026-09-19
> Owns: delta replication, registered as a follow-on of R6 in
> `2026-09-15-source-runtime-reliability.md`
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/design/mounts.md` (replication by generation),
> `2026-09-15-source-runtime-reliability.md` (R3 generations, R6 database
> delivery), `docs/core/design/source-durability.md` (what sync needs from
> stores), `crates/store/src/revision.rs` (the store revision)

## Outcome

A replica stays current by applying the changes its owner committed since the
replica's revision. It copies a whole store only to start, or when it has
fallen further behind than the owner keeps changes for. A one-file change
costs a one-file transfer and a one-file update to the replica's arena. An
unchanged source costs one listing request.

## Why

Today every replication is a full copy. `FetchSnapshot` and `FetchDatabase`
are the only ways to refresh a replica, and both send the whole artifact. The
receiver then discards its arena and rebuilds one from the new file. The
protocol has no request for what changed, and the store keeps no change
history that could answer one.

On the night of 2026-09-18, titan copied the Mac's home source in full four
times between 20:25 and 00:51. Rounds 2 to 4 carried exactly the records
round 1 did: `data.db` was last written at 20:22:56, before the Mac's daemon
started, and its WAL stayed empty. Times are America/Vancouver.

| Round | Started | Transfer done | Published |
|---|---|---|---|
| 1 | 20:25 | not logged | 21:30 |
| 2 | 21:38 | 21:47 | 22:44 |
| 3 | 22:44 | 22:51 | 23:47 |
| 4 | 23:48 | 23:54 | 00:51 |
| 5 | 00:51 | 00:58 | 01:55, the switch to P0 |

- Trigger. A nested source's generation was folded from the size and mtime
  of `data.db` and `data.db-wal`. The store's pool runs on sqlx defaults: idle
  connections close after 10 minutes and none are kept. When the last one
  closes, SQLite deletes the WAL, and the next read recreates it. Either step
  moved the generation. The WAL was recreated at 20:58 and 23:16 and deleted
  by 00:49, while `data.db` stayed untouched.
- Cost per round. The owner runs `VACUUM INTO` over the 766 MB store and
  streams the 736 MB result at about 1.5 MB/s, 6 to 9 minutes. Titan then
  rebuilds a 1.88M-entry arena from it in about 57 minutes on one core. A
  round ran about 64 minutes end to end.
- Loop. The generation is stamped when the export starts. A pool drain
  anywhere in that hour made the replica stale on arrival, and the next
  30-second pass started another round.
- Why verification missed it. The R6 check held the generation stable
  across twelve refresh intervals, and R3's proof asks for ten. Both windows
  end before the pool's 10-minute idle timeout.
- Leaks. An export is deleted only when its handler finishes, and a
  download's temporary file only when its fetch returns. A daemon exit
  mid-transfer strands either. The Mac held four orphaned exports (2.8 GB,
  removed by hand); titan holds 58 orphaned downloads (about 3.6 GB).
- Fallback. Twice that night titan took the 117 MB snapshot path for the
  nested home source, the fallback for owners that cannot export. A snapshot
  replica records the snapshot's generation, which never equals the database
  generation in the next listing, so the fallback guarantees another full
  round.

## P0: store revision (landed 2026-09-19)

`crates/store/src/revision.rs` gives each store a `_revision` row: a store id
drawn when a writer first opens the store, and a count of committed changes.
Triggers on every tracked table (`record`, `directory_path`, `content`,
`edge`, `record_overlay`, `tag_definition`, `tag_assertion` and the data
type's facet tables) add one per inserted, deleted or changed row. The update
trigger compares old and new values and ignores `record.scan_epoch` and
`record.indexed_at`, so a restamped epoch or an identical re-put changes
nothing. Triggers are installed on every writable open and replaced only when
their SQL differs, so reopening an unchanged store writes nothing, and a facet
column added later is compared from then on. A store no writer has opened
reads as untracked: nil store id, count zero.

A nested source's listing generation and its `DatabaseHeader` generation are
now the store revision folded through blake3 (`revision_generation` in
`core/src/service/network/protocol/byterange.rs`). File metadata no longer
decides whether a replica is current.

Cost. A trigger makes every single-row write to a tracked table run inside a
statement journal. With SQLite's default file-backed temporary storage that
journaling is expensive, and with temporary storage in memory it costs little.
On `crates/store/tests/scale.rs` with one store of 300,000 files:

| Build | Store build |
|---|---|
| Before P0 | 12.3 to 13.7 s |
| P0 | 16.3 to 17.9 s |
| P0 with `temp_store = MEMORY` on writable pools | 8.9 to 10.2 s |
| Before P0 with `temp_store = MEMORY` | 8.3 to 9.2 s |

The last row shows the walker already paid part of this before P0. In-memory
temporary storage removes both costs, but `duplicate_copies` and the
pending-content queries also run on the writable pool and would sort in memory.
That trade is decision 1 below.

Live: the Mac restarted on this build at 00:49, and its home store was
tracked at 00:50 with store id `01a0b8a5…` and revision 0. Round 5 was the
one-time switch from file generations to revision generations, published at
01:55. In the 19.5 hours after it, to 21:26, titan transferred nothing at all:
no database copies and no snapshot copies, across a checkpoint that moved
`data.db`'s mtime at 01:55 and a restart of the Mac's daemon at 02:02 that
closed every pool. Its manifest kept refreshing the owner's facts without a
transfer. The same window on the old scheme would have cost about 18 rounds.
Two probe files created in the home folder did not reach its store within a
minute, which matches `data.db` going unwritten since 20:22.

Titan was deployed onto this build at 21:34 the same day, with `bin/ffmpeg`
and `bin/ffprobe` (n8.1.2, from the CCTV remux tools) and the two path
variables in its start command, so video tiles bake there: a forced tile on a
CCTV clip spawned `bin/ffmpeg` with the 384px thumbnail filter and reported
one generated. Its stores read as untracked until a writer opens them, which
is stable and costs nothing.

## Target shape

- The owner's store records the revision each row last changed at, and a
  tombstone for each row it deletes.
- A listing carries the raw revision, store id and count, so a replica knows
  where it stands as well as whether it is current.
- A replica asks for the changes since its revision. The owner answers with
  changed rows and tombstones in revision order, in pages, or tells it to
  start over with a full copy.
- The replica applies each page to its database copy in one transaction, and
  to its arena in place.
- A full copy happens at bootstrap, after the owner's store is recreated (a
  new store id), or when the replica is behind the owner's oldest retained
  tombstone.

## Design

### Row stamps

Each tracked table gains a `revision` column holding the store revision its
row last changed at. The P0 count moves once per row change, so a row's stamp
is the count right after its own increment, and every row changed in one
transaction gets its own consecutive value. `changes_since(n)` reads every
tracked table for rows stamped above `n`, inside one read transaction, and
reports the count in that same snapshot as the new revision.

Stamp from the triggers P0 already runs: bump, then write the new count into
the changed row. Recursive triggers stay off, and `revision` joins the ignored
columns so the stamp cannot re-fire its own update trigger. Measure the added
row write against the scale test. Writer-set stamps are the alternative only if
that cost is material, because triggers are the one mechanism that counts
every write path.

### Tombstones

Every delete on a tracked table writes `(table, key, revision)` to a
`_tombstone` table, keyed by that table's primary key. Applying a tombstone
for a row the replica never had is a no-op, so cascaded child deletes need no
special case.

The owner retains tombstones for a window and records the revision below which
its change history is no longer complete. A replica asking for changes below
that point is told to take a full copy.

### Protocol

- `RemoteSourceInfo` carries the raw revision for store-backed sources. New
  fields take serde defaults, so a fleet mid-deploy keeps decoding. There is
  no second code path for old peers.
- `FetchChanges { source_id, since, max_rows }` answers with a header naming
  `from`, `to`, and whether a full copy is required, followed by change pages
  in revision order. A different store id always requires a full copy.
- `FetchDatabase` stays for bootstrap and for full-copy answers.

### Replica apply

- The replica keeps its database copy in `mounts-remote/<device>/<source>.db`,
  writable by the replicator alone. Each page applies in one transaction:
  upsert rows, apply tombstones, then set the replica's `_revision` to the
  owner's. The replica's own triggers count during apply; the final
  assignment overwrites them, so the replica's revision always names the
  owner's.
- The same changes update the arena in place (add, update, move, remove),
  through the operations the watcher already uses. A full
  `arena_from_database` rebuild happens only at bootstrap.
- A page that fails to apply leaves the last good revision in place for a
  retry. Repeated failures fall back to a full copy.

### Bootstrap

Profile `arena_from_database` before relying on it: 57 minutes on one core for
1.88M entries is far more than paging 2,000 rows at a time should cost.

Remove the snapshot fallback for nested sources. Both sides run this protocol,
and a snapshot replica's generation can never match a database listing.

### A restart must not re-copy what the replica already has

Restarting titan at 21:34 on 2026-09-19 cost a full 736 MB copy of the Mac's
unchanged home source, finishing at 21:38, followed by the hour-long rebuild.
The manifest held the right generation the whole time.

`restore_from` publishes a share only after `restore_artifact` rebuilds its
arena, which for a database replica is the same rebuild that takes about an
hour at this size. It also inserts the manifest into the known set only after
that loop. The first sync pass runs 30 seconds after the peer connects, finds
no share for the source, and never reaches `transfer_due`, so it fetches.

The persisted generation has to be visible to the transfer decision before the
artifact finishes loading: read manifests into the known set first, and let a
source with a known generation and an unfinished restore report itself as
current-but-loading rather than as absent. R3 already asks for this in
`2026-09-15-source-runtime-reliability.md` ("offline restart preserves
inventory; unchanged generations do not reload"), and the proof there needs a
case that restarts a replica holding a large database.

### Cleanup

This executes R3's "clean up failed temporary transfers".

- Owner: at daemon start, delete `sources/*/export-*.db`. No export can be in
  use before the first request.
- Replica: at daemon start, delete `mounts-remote/*/*.tmp`.
- Both: remove the temporary file on every exit path of a transfer,
  cancellation included, through a guard that deletes on drop.

### Volume-root sources

Volume-root sources replicate as arena snapshots, with a file-metadata
generation and paced `dirty` refreshes. Once store deltas work, move them onto
store replication as well. That is R6's registered follow-on "database
delivery for non-nested sources", and it retires snapshot replication and its
pacing.

## Phases

| Phase | Scope | Exit proof |
|---|---|---|
| P0 | Store revision; nested generations from it | Landed. An unchanged source transfers nothing across a pool drain; see Acceptance |
| P1 | Transfer cleanup, and a restart that keeps what it has | Killing either daemon mid-transfer leaves no export or temporary file after restart. Restarting a replica with an unchanged owner transfers nothing |
| P2 | Row stamps, tombstones and `changes_since` in the store | Property test: after random walker batches, adapter upserts, renames, removals and tag writes, applying `changes_since(n)` to a copy taken at `n` equals a fresh copy. Identical re-puts produce no changes. Write cost reported against the scale test |
| P3 | `FetchChanges` and replica database apply | A one-file change on the Mac reaches titan's replica database within one refresh interval, with bytes proportional to the change. An owner restart transfers nothing. A recreated store and a replica behind compaction each take exactly one full copy |
| P4 | Arena apply and bootstrap profiling | A delta updates titan's arena in time proportional to the delta, and the result matches an arena rebuilt from the replica database. Bootstrap time measured and bounded |
| P5 | Volume-root sources onto store replication | No replication path reads file metadata as a version |

## Acceptance

Every live proof in this plan observes for at least 30 minutes, or forces a
pool drain, so it outlasts the pool's idle timeout. Report generations
advertised, received and served, bytes transferred, skipped transfers, full
copies and apply time, the counters R3 asks for.

## Decisions for James

1. `temp_store = MEMORY` on writable store pools. It makes bulk writes about
   25% faster than before P0 and removes P0's cost, but moves large sorts on
   that pool, such as `duplicate_copies`, from temporary files into memory.
2. Tombstone retention: a fixed window by count or age, or retention until
   every known replica acknowledges a revision.
3. Whether adapter sources replicate to peers. Today only filesystem sources
   are listed.

## Relationship to sync

Library sync stays deferred. This is replication of one store from its owner,
which is the only writer of the rows it sends. The revision is the order
changes landed in one store file. It is not a clock and does not assume a
single writer. Assertion sync keeps its HLCs and the outbox. When sync
returns on top of sources, the change feed built here is the transport for the
rebuildable half of a store.
