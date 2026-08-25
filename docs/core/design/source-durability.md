# Source Durability

> **Status:** Design position, captured from James's direction 2026-08-22. Replaces the flat claim carried in `crates/store` that a source store is simply durable, and the older claim in the archive crate that it is simply disposable. Both were true of one half of a store on one kind of day.
> **Companions:** `docs/plans/2026-08-20-architecture-previs.md` (the identity model and the five tiers), `docs/core/design/tags-and-assertions.md` (the assertion layer this generalizes), `docs/plans/2026-08-22-source-convergence.md` (decision 2), `docs/core/design/shares-and-remote-libraries.md` (snapshot browsing, which sync reuses)

## The rule

**A store has two halves, and only one of them can ever be rebuilt.**

The *generation* is what an ingest produced: records, facets, content, edges,
FTS. It can be rebuilt for exactly as long as its origin still answers.

The *assertion layer* is what no ingest produced: ratings, favorites, corrected
titles, tags, album membership a person curated. Nothing can rebuild it, on any
day, for any source, however live the origin is. Gmail can hand back every
message it ever delivered and it cannot hand back which ones you starred in
Spacedrive.

So "a source store is disposable" and "a source store is durable" were each
describing a different half.

## Reproducibility is a property of the origin, at a moment

The generation's rebuildability is not a property of the store, or of the crate,
or of the kind of source. It is a property of whether the origin is still
reachable, and it has three properties that together decide the design:

- **It varies per source.** A mounted drive and a revoked OAuth token are not
  the same situation.
- **It varies over time.** An archived email account is reproducible right up
  until the day the account closes, and that day is not on the calendar.
- **The transition is silent.** No event fires when access is lost. You discover
  it on the next sync, which may be months later, and by then the only record of
  what was in that account is the store.

The third property is the load-bearing one. At any given moment the code cannot
tell which regime it is in, so it has to behave as though the store is the only
copy. That is a rule about what code may assume, and it stands even though the
underlying fact is false much of the time.

**The invariant, stated correctly: no code path may assume a store can be
rebuilt. Rebuild is an operation a person can ask for when the origin is known
to be answering. It is never a recovery the system helps itself to.**

## Origin availability is a registry column

If reproducibility varies then something has to hold it, because real behaviour
hangs off it. It belongs on the source row in `library.db`, alongside the root
and the nullable `volume_uuid`:

| state | meaning | how it is reached |
|---|---|---|
| `live` | the origin answered recently | a successful walk or sync |
| `degraded` | the origin has stopped answering and might come back | a detached volume, a failing token, a network mount that is not resolving |
| `lost` | the origin is gone | the person says so, or a terminal failure from the origin (account closed, token revoked with no reissue path) |

What reads it:

- **Space reclaim.** Dropping a generation to free disk is only offerable on a
  `live` source.
- **Delete semantics.** Deleting a `live` source discards a cache and some
  assertions. Deleting a `lost` source destroys the only copy of an archive.
  These need different confirmations, and today they are the same call.
- **Backup scope.** A `lost` source's file is irreplaceable and should be first
  in any backup the product ever offers.
- **Sync payload.** See below: a reproducible source can ship its assertions and
  let the far side re-derive, a lost one has to ship bytes.
- **What the person is told.** "This drive holds the only copy of these tags" is
  a thing a person should see before they wipe it, and it is derivable only from
  this column.

`degraded` and `lost` differ by intent, not by observation, which is why the
transition between them cannot be fully automatic. See Open.

## The line runs through the file, not between files

Both halves live in one `source.db`. That was settled by WAL: SQLite's atomic
commit across `ATTACH`ed databases works through a super-journal in rollback
mode only, so a batch and its watermark cannot commit together across two files
without giving up the journal mode that lets reads fan out while an index
writes (previs, decision 1).

One file, two halves, and the halves are marked by table membership:

| half | tables |
|---|---|
| generation | `record`, `facet_*`, `content`, `edge`, `search_index`, `_sync_state`, `_schema` |
| assertions | `record_overlay`, and the tag tables when they land |

Marking it is not bookkeeping. Four operations are the same operation once the
line is explicit:

- **Reindex** drops and recreates the generation tables in one transaction and
  leaves the assertion tables alone. This is more atomic than deleting a file,
  which is what the two-file shape offered.
- **Evict** does the same thing and does not re-walk, leaving a source whose
  assertions are intact and whose index is absent. Only offerable on `live`.
- **Sync a reproducible source** ships the assertion tables and lets the far
  side walk its own copy of the origin.
- **Sync a lost source** ships the whole file, because there is nothing to
  re-derive from.

## What sync becomes

Sync comes back once the migration lands. The deferral holds for a stronger
reason than the old implementation being flaky:
`infra/sync` and `service/sync` are 15,571 lines of replication over a single
shared mutable tree with device-owned and shared rows, which is the hardest
version of the problem that exists. Per-source replication is a smaller problem
with a different shape, and building it on the old substrate would have been
paying the hard version's price for the easy version's feature.

Two axes, and they are independent.

**Transport is chosen by update frequency.** A static source (an archive drive,
a closed account) replicates as a snapshot of its file. A hot source (an inbox,
a working directory) is read live over the network while the owner is online.
Half of this already exists in a different feature:
`shares-and-remote-libraries.md` browses a shared subtree from a snapshot so it
stays fast and works while the owner is offline, with the mount byte plane
fetching ranges on demand. Device-to-device sync is a share addressed to
yourself. Built that way it inherits one permission model, one snapshot format
and one byte path instead of growing a second set.

A snapshot is `VACUUM INTO` or the backup API, never a file copy, because these
pools run WAL.

**Merge is chosen by writer count.** This is the axis it is easy to get wrong,
because it looks like the first one and is not. A detached archive drive is
static and single-writer, so a snapshot is a complete replica. The moment two
devices can both tag things in the same source, that source has two writers and
a snapshot is a lossy overwrite of whichever device wrote last. Assertions are
exactly what a person edits on whichever machine is in their hand, so the
multi-writer case is the normal case for the half that matters.

Which gives the shape:

- **generation:** one-way. Replicate it or re-derive it, never merge it. It has
  one writer by construction, the ingest that owns the origin.
- **assertions:** merge. Last-writer-wins on scalar fields ordered by HLC,
  device uuid as the tiebreak. `tags-and-assertions.md` already specifies this
  for tag definitions and applications, including the convergent slug that lets
  a drive arriving at a foreign library find that library's existing "Work"
  rather than minting a second one.

The HLC retained out of the sync teardown is what orders the merge. It is the
only thing carried out of `infra/sync`, and this is the work it was kept for.

## What the schema has to reserve now

None of this is built during the migration. What the migration must not do is
make it expensive later, and three things currently would.

**1. Assertion rows need `hlc` and `device_uuid` at birth.** `tag_assertion` in
`tags-and-assertions.md` is specified with both. `record_overlay` as it exists
in `crates/store/src/record.rs` has `updated_at datetime('now')` and no device,
which is a wall clock with no tiebreak, and wall clocks across devices are the
thing HLC exists to replace. Adding two columns while the only rows are eleven
adapters' worth of email is free. Adding them once the table holds every rating
in the library is a migration over user data that has no second copy.

**2. Assertions need a portable rebind key as well as a stable local one.**
Neither key alone survives both tests:

| key | survives a rename | survives crossing to another device |
|---|---|---|
| record uuid (v7, assigned locally) | yes | no, two machines mint different uuids for the same file |
| `(type, external_id)` (path, relative to root) | no, the rebind procedure rewrites `external_id` on move | yes, relative paths are portable |

So an assertion carries the uuid as its key and `(external_id, content_id)` as
rebind evidence, which is the same two-factor shape the record resolution
procedure already uses and the same shape `slug_id` gives a tag definition.
`tag_assertion` is already specified this way, carrying `record_uuid` and
`content_id` together with neither substituting for the other. `record_overlay`
should match it.

**3. Content ids have to be convergent, and this is the strongest argument for
it.** The record rebind procedure in
`docs/plans/2026-08-21-filesystem-source-store.md` resolves on path, inode, size
and mtime. Inode does not survive a different machine or a different mount, so
of that evidence set only the path crosses a device boundary, and the path is
the weak factor. A content id derived from the bytes (`v5(CONTENT_NAMESPACE,
hash)`) is computable offline, by two machines that have never communicated,
retroactively, with no clock. For any file that has been hashed it is the only
strong cross-device evidence that exists. Today `content.id` is an `INTEGER`
rowid, which is the one form that cannot cross a machine boundary at all, and
`tag_assertion` is already written against the convergent form (`content_id
TEXT`).

## Decisions

1. **The durability invariant is a rule about code, not a claim about stores.**
   No path may assume a store can be rebuilt. Prose that states it as a fact
   about all stores, in either direction, is wrong and gets corrected where it
   appears.
2. **Origin availability lives on the source registry row** in `library.db`,
   with three states, and reclaim, delete, backup and sync all read it.
3. **Generation and assertion tables are named as two sets** in the store crate,
   so reindex and evict are expressible as transactions over one of them.
4. **Assertion tables carry `hlc` and `device_uuid`.**
5. **Assertion tables key on record uuid and carry `(external_id, content_id)`
   as rebind evidence.**
6. **Content ids are convergent uuids derived from the hash,** settled before a
   filesystem source writes its first million records rather than after.

## Open

- **The `degraded` to `lost` transition.** A drive unseen for six months and a
  drive that was thrown away are indistinguishable from observation. A time
  threshold guesses; asking the person makes them answer a question about a
  drive they are not thinking about. Likely both: a threshold that changes what
  the UI offers, and an explicit "this is gone" the person can assert.
- **Two devices indexing one origin, before sync exists.** A NAS mounted on a
  laptop and a desktop is ordinary today, and each machine mints its own record
  uuids for the same files. Nothing in the current model reconciles them.
  Decision 6 gives hashed files an answer. Unhashed files have none.
- **Whether evict is a user action.** Reclaiming space by dropping a generation
  is obviously useful and is also a way for someone to destroy an index they
  believed was a backup. The `live` gate is necessary and might not be
  sufficient.
- **Where a snapshot of a lost source is allowed to land.** Shipping the whole
  file to another device is the correct answer for durability and duplicates
  irreplaceable data onto a machine with different retention. This is a product
  question before it is a storage one.
