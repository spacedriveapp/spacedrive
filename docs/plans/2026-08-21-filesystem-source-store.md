# The filesystem source store

> How a filesystem source gets a database, and what flows through it.
> Companion to `2026-08-20-entries-teardown-execution.md` (T2.1, T2.2, T2.5).
> This is the front half of phase 1: it is what everything durable in P2 waits on.

## The gap

`SourceDirs::source_db_file(id)` resolves `sources/<id>/source.db`. It has **zero
callers**. Nothing creates it, nothing opens it, nothing writes it.

So the filesystem lineage today is: walker → `MemoryAdapter` → arena → snapshot.
Every record uuid is minted `Uuid::now_v7()` at first sight and lives only in
`ephemeral.snapshot`, a file governed by cache rules with five discard paths. The
archive lineage has the store (`data.db` per source, spine + facets) but its
writer is one record at a time with no transaction.

Neither half works alone. This closes the gap by giving the filesystem the
archive's store and giving the store a writer that can take a walk.

## The decision: shared store, separate ingest

A filesystem source gets a `source.db` with the **same** schema shape as every
other source — `SPINE_SCHEMA` plus one facet table. What it does *not* get is the
archive's ingest path.

The reasons ingest has to fork:

| archive adapter | filesystem walker |
|---|---|
| subprocess, JSONL over stdio | in-process, streaming |
| delta stream resumed from a cursor | complete enumeration of a subtree |
| `external_id` is stable at the source (message-id, note uuid) | path and inode are both unstable |
| ~10³–10⁵ records | 10⁵–10⁷ records |
| `serde_json::Value` per record | `EntryMetadata`, already typed |

The reasons the store must *not* fork: cross-source search, cross-source edges,
and `catalog.db` in phase 5 all join on one shape. Two shapes means two of
everything downstream, forever.

So: **one file format, two writers.** `SourceDb::upsert` (adapters) and a batched
filesystem writer become peers over the same tables. `upsert` is reimplemented as
a batch of one, so there is still only one set of SQL.

Deliberately *not* doing: registering the filesystem as an archive data type with
a TOML schema and a `builtin:filesystem` adapter. That buys registry convergence
at the cost of round-tripping `PackedMetadata` through `serde_json::Value` on
every file. Convergence is worth having, and it comes later at the registry layer
(§7) where it costs nothing.

## The schema

The spine is unchanged. One facet table is added, and it is the only new DDL:

```sql
CREATE TABLE IF NOT EXISTS facet_file (
    record_uuid TEXT PRIMARY KEY REFERENCES record(uuid) ON DELETE CASCADE,
    size       INTEGER NOT NULL,
    mtime      INTEGER NOT NULL,
    inode      INTEGER,
    mode       INTEGER,
    extension  TEXT,
    is_hidden  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_facet_file_inode ON facet_file(inode);
```

The spine row for a file:

- `external_id` — path relative to the source root. Relative, so a drive that
  remounts elsewhere does not invalidate every row.
- `type` — `"file"`, `"directory"`, `"symlink"`. Open string, never an enum.
- `title` — the file name.
- `parent_uuid` — the containing directory's record uuid. This is the closure;
  `entry_closure` does not come back.
- `content_id` — the local `content` row, once hashed. Convergent content uuids
  land in T2.7; until then this stays a rowid and is source-local.

**No FTS for filesystem sources.** The arena's `NameRegistry` already serves name
search and is faster than FTS5 for the prefix queries the UI issues. FTS becomes
worth its write cost when full-text content search arrives, not before. The
search contract stays empty, which `diff_schemas` already tolerates.

### There is no separate identity ledger

The plan called for one. Writing it out, it turned out to be `record.external_id`
plus three columns of `facet_file` — the same rows, duplicated. So the ledger is a
**resolution procedure over the spine**, not a table:

| evidence | outcome |
|---|---|
| path hit, `(size, mtime)` unchanged | same record, nothing written |
| path hit, `(size, mtime)` changed | same record, facet updated, `content_id` cleared |
| path miss, inode hit, **and** size or mtime matches | **rebind** — the file moved; `external_id` updated |
| path miss, inode hit, size and mtime both differ | new record — inode reuse is real, so decline |
| path miss, inode miss | new record, `Uuid::now_v7()` |

Row 3 is the two-factor minimum: an inode alone never rebinds. Row 4 is where the
conservatism sits — a wrong rebind silently moves someone's tags to an unrelated
file, and a missed rebind only costs a re-tag.

A rename the watcher actually saw needs none of this; it carries old→new
directly. Resolution is for the gap — daemon off, drive in a drawer, walk
interrupted.

Once content uuids exist (T2.7) they become the strongest evidence available and
slot in above inode, but they cost a hash and are only present for files already
hashed. The procedure is written so that arm is additive.

## Identity is resolved in memory

The arena cannot wait on SQLite — it is the primary read path and its
responsiveness is why it became one.

So at attach, the ledger loads into the slot: `HashMap<RelPath, (Uuid, size,
mtime)>` plus `HashMap<inode, Uuid>`. Roughly 60 bytes per record; 500k files is
~30 MB, the same order as the `entry_uuids` map already deserialized from every
snapshot today. Resolution stays a hash lookup. Writes go to a batch channel.

The consequence, and the point of the whole exercise: **`entry_uuids` leaves the
snapshot.** The snapshot becomes purely rebuildable — six cache fields, no
durable seventh — and all five of its discard paths become harmless. Deleting
every snapshot on the machine then loses nothing but startup time.

**The migration window is free.** Arena uuids have nothing durable keyed to them
yet: tags still key to `entry.uuid` in `library.db`, which is the entries world
and is being dropped anyway. So the ledger starts empty and backfills by walking.
No uuid preservation step, no backfill from existing snapshots. This window
closes the moment T4.9 re-keys tags to record uuids — which is why T2.5 has to
land first.

## The data flow

### Attach

A drive is plugged in, the daemon starts, or a folder is added.

1. `register_source(root, fingerprint)` → source id. *(exists)*
2. `create_source_dir(id)`, open `source.db`, apply spine + `facet_file` DDL.
   Idempotent — `IF NOT EXISTS` throughout, same as `SourceManager::open`. **new**
3. Load the ledger into the slot. **new**
4. `ensure_restored` deserializes the snapshot into the arena. *(exists)*
5. If the watermark is behind or absent, dispatch a walk.

### Walk

1. The walker produces `EntryMetadata` — path, kind, size, mtime, inode,
   permissions, hidden. This is already the observation type; nothing new is
   introduced.
2. Resolve identity against the in-memory ledger → `(uuid, outcome)`.
3. `arena.add_entry(path, uuid, metadata)` — synchronous. The UI sees it now.
4. Push onto the batch channel unless the outcome was *unchanged*.
5. The drain commits every N records or T milliseconds: record upserts, facet
   upserts, and the watermark **in one transaction**. A crash mid-walk resumes
   from the watermark rather than restarting.

### Watch

The fs-watcher produces the same `EntryMetadata` and takes the same three steps.
One code path for walk and watch, which is what `MemoryAdapter` already
guarantees by implementing both `ChangeHandler` and `IndexPersistence`.

### Deletion

A walk of a subtree is a **complete enumeration**, so a record in that subtree
carrying a stale `scan_epoch` is genuinely gone. The sweep is scoped to the
walked subtree and runs at walk completion.

This is a real semantic difference from the archive, whose adapters send deltas
and where `spine.rs` deliberately declines to sweep. Both behaviours are correct
for their ingest; the sweep belongs to the filesystem writer, not to `SourceDb`.

### Read

- Directory listing, search, media, sizes: **arena only**. `source.db` is not on
  the read path for browsing and never becomes one.
- Tags, assertions, content ids: `source.db` by record uuid, joined at read time.
- Cross-source answers: dark until `catalog.db` in phase 5.

### Detach

Arena stays (snapshot-restored), `source.db` stays. Both readable, writes
refused. Unchanged from today except that identity now survives properly.

### Forget

`rm -rf sources/<id>/` takes the snapshot, thumbnails, blocks and database
together. One directory, one removal — which is why the block cache belongs at
`sources/<id>/blocks/`.

## What has to change in the archive crate

1. **Writes take an executor, not `&self.pool`.** `put_record`, `put_facet`,
   `put_edge`, `set_content_identity` currently each grab their own connection
   from the pool, so a multi-statement write can straddle connections. They need
   to accept `impl SqliteExecutor` so a caller can hand them a transaction.
   Mechanical, touches every write method, and it is the prerequisite for
   everything else here.
2. **`SourceDb::apply(&[Observation]) -> Result<Vec<Uuid>>`** — one transaction,
   prepared statements reused across the batch. `upsert` becomes `apply` of one.
3. **`data.db` → `source.db`, and paths come from `SourceDirs`.**
   `SourceManager` currently hardcodes both the filename and
   `sources_dir.join(source_id)` with a `String` id, which does not agree with
   `SourceDirs`' `id.simple()` form. One layout owner.
4. **Drop `adapter.toml` for in-tree connectors** (already on the register as a
   phase-1 blocker; it is in the way of the same paths).

Nothing above changes adapter behaviour. Item 1 fixes a latent correctness bug on
its own.

## Phases

**1 — Transactional writes.** Archive changes 1 and 3. No new callers; the
existing adapter tests are the check. Landing this first means the store work
never has to reason about a half-transactional writer.

**2 — The store exists.** `SourceSlot` opens `source.db` at attach and applies
the schema. Nothing writes to it yet. Verifiable on its own: register a source,
see the file, confirm it survives a restart and that removing it costs nothing.

**3 — The writer.** `MemoryAdapter` gains the batch channel and the drain. Both
sinks receive every observation. Uuids still come from the arena. At this point
`source.db` mirrors the arena and is disposable.

**4 — Identity moves.** The ledger loads at attach and becomes the source of
uuids; the arena adopts what it is handed; `entry_uuids` leaves the snapshot
format. This is the step that makes `source.db` authoritative, and it is T2.5.

**5 — The sweep.** Stale-epoch deletion scoped to the walked subtree, plus
watermark resume.

Phase 4 is the one that matters. 1–3 are scaffolding that can be verified in
isolation; 4 is the point at which identity stops living in a cache.

**Acceptance, end to end:** index a filesystem source, tag a file, delete every
snapshot and the arena, re-walk — the record uuid is unchanged and the tag is
still attached. Then rename the file with the daemon stopped, re-walk, and the
tag is still attached.

## Open calls

- **Scratch has no store.** Paths under no registered source get an arena
  partition and no `source.db`, so nothing durable can attach to them. Since
  `volumes.index` registers every mounted volume, scratch is mostly network paths
  and oddities — but "tag a file and it silently does not persist" is not
  acceptable. Either the UI declines to tag outside a source, or tagging promotes
  the containing volume to a registered source. Leaning toward promotion.
- **Batch size and drain interval.** Wants measuring on a real walk rather than
  guessing; the transaction boundary is the thing being tuned.
- **Registry convergence (T2.1).** `sources.json` and `registry.db.sources` still
  both exist after this. The shape that works: `data_type = "filesystem"`,
  `adapter_id = "builtin:filesystem"`, and root/fingerprint/volume_uuid in the
  existing `config` JSON column — kind-specific fields go in `config`, columns
  stay kind-agnostic, no schema fork for a source with no fingerprint. Sequenced
  after this work because it drags T2.0b (volume/source boundary) with it, and
  **the store does not need it**: a source id and a directory both already exist.
