# Locations demoted

> **Superseded in part on 2026-09-15:** L0's small source-relative pin row
> landed. Known folders are now computed Places rather than default location
> rows. L1 and L2 are cancelled: filesystem sources retain their configured
> roots and watch them after a successful walk. A location is navigation intent
> and a stable target for future explicit policy, not an implicit retention or
> watcher switch.
>
> **Current handoff:** L0 landed, L1 and L2 are cancelled, and the old L3/L4
> cutover mostly landed. `2026-09-15-entries-final-drop.md` owns the remaining
> schema, assertion, row-sync, test, and naming work. L5's source-scoped media
> work follows the product sequence rather than blocking the entry drop.

Amends `P4` of `2026-08-22-source-convergence.md`. That phase is titled "Delete
entries" and its unwritten second half assumed locations went with them. They
do not. They lose everything that made them an indexing boundary and keep the
one thing they were always good at, which is naming a folder a person cares
about.

## The original rule

**A location is a pin over a path, never an owner of records.**

Everything below follows from that sentence. If a location owns rows, deleting
one has to decide what happens to them, which means it needs its own store,
its own indexer and its own watcher, which is the code this phase deletes.
If it owns nothing, deleting one is a `DELETE` of a single row and the map is
untouched, because the map never knew.

## Why the concept survived the map and the boundary did not

A location existed because adding one was how Spacedrive found out a file
existed. Nothing walked a drive, so a path nobody had named was a path nobody
had seen. That is what put an act of configuration in front of a person who
opened a file manager expecting to see their files, and it is why "location"
reads as an obstacle rather than as a feature.

The storage map removed the premise. `map_attached_volumes` walks every
attached, local, mounted volume at `LOW` with `Notify::Silent`, keeping
structure near the root and counts below `SUMMARY_DEPTH`. `/Applications` is in
the arena whether or not anyone asked for it. So the question a location used
to answer, *do we know about this path*, has no askers left.

The question it answers now is different and still real: **which path did this
person choose as a durable shortcut.** A drive can be mapped at low fidelity,
but a configured source retains and watches its root. Hiding either behavior
behind the existence of a navigation pin makes the runtime harder to explain
and makes unpinning look destructive.

`Retention` has a covered-path parameter:

```rust
pub struct Retention {
    pub summarise_rejected: bool,
    pub depth: Option<usize>,
    pub covered: Vec<PathBuf>,
}
```

`map_attached_volumes` fills `covered` from source roots because those roots are
the retained scopes. Locations do not replace that input. A future policy can
target a location ID, but it must be explicit and evaluated by the source.

## What the row becomes

Today:

```rust
pub struct Model {
    pub id: i32,
    pub uuid: Uuid,
    pub device_id: i32,
    pub volume_id: Option<i32>,
    pub entry_id: Option<i32>,
    pub name: Option<String>,
    pub index_mode: String,      // "shallow", "content", "deep"
    pub scan_state: String,      // "pending", "scanning", "completed", "error"
    pub last_scan_at: Option<DateTimeUtc>,
    pub error_message: Option<String>,
    pub total_file_count: i64,
    pub total_byte_size: i64,
    pub job_policies: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

After:

```rust
pub struct Model {
    pub id: i32,
    pub uuid: Uuid,
    /// The source whose store holds this subtree's records.
    pub source_uuid: Uuid,
    /// Relative to the source root, so the row survives the drive remounting
    /// somewhere else.
    pub relative_path: String,
    pub name: String,
    /// Whether the user put it there. A default the user removed stays removed.
    pub origin: String,          // "default" | "user"
    pub created_at: DateTimeUtc,
}
```

Every dropped field belongs somewhere that already has it, and in most cases
has a better version:

- `entry_id` is the ownership this phase exists to remove.
- `device_id` and `volume_id`: the source carries `volume_uuid`, and a location
  is inside a source.
- `scan_state`, `last_scan_at`, `error_message`: `sources.status` and
  `last_indexed_at`. A walk is a property of the source being walked.
- `total_file_count`, `total_byte_size`: the arena maintains `subtree_bytes`
  and `file_count` along the ancestor chain through `bump_ancestors`, and its
  numbers are the honest ones. A persisted count cannot tell you that 400 GB
  under a rejected directory was summarised rather than absent; a stub can.
- `index_mode`: `Retention` says it, in the vocabulary the walk actually reads.
- `job_policies`: nothing reads it that survives this phase.

There is no `pinned` column. A location row *is* the pin. A folder without a
row is a folder in the map like any other.

## What the row is for

One job: preserve a named, source-relative navigation target across remounts
and devices. Source-relative identity is why this remains a row instead of a
raw client bookmark.

## The policy taxonomy

The old location row bundled every preference a scope could have. The
demotion did not relocate that bundle; it dissolved it, because it was
three different kinds of policy that want different owners:

| policy | question it answers | owner | mechanism |
|---|---|---|---|
| capture | what gets recorded durably | the source | `SourceConfig` in the source row's config column; the walk and the watcher both read it |
| navigation | which path a person pinned | the location | a source-relative row projected under Places |
| display | what a person sees | the lens | rules applied at view time over a store that captured everything |

Two rules keep the boundaries honest. Nothing filters at write time
except capture, and capture defaults to everything: any policy applied
during a write silently loses data that a view-time policy would merely
hide, which is how live changes under `temp/` once vanished from an
archival source while its walks kept them. And capture only widens
implicitly: tracking a root again may turn the rules off, never on, so
a casual re-track cannot demote an archival source. Narrowing is
`sources.update`'s explicit job, and it removes nothing, because the
records stand and the lens decides what shows.

The old `index_mode` does not reappear anywhere in this table. Shallow,
content and deep were capture depths, and capture no longer has depths:
structure is always recorded, identity is tiered automatically (sampled
behind the walk, integrity behind a decision that needs it), and the
LOD is the arena's memory policy rather than a per-scope choice.

## Two rules to fix now

Both are cheap to decide and expensive to discover.

**Innermost source wins.** Sources nest. A location under `/Volumes/Work/Media`
where both `/Volumes/Work` and `/Volumes/Work/Media` are sources belongs to
`/Volumes/Work/Media`, and `relative_path` is relative to that. One owner, no
ambiguity about which store holds the records.

**Policy is explicit.** A future retention, protection, or offline policy may
refer to a location ID. The location row alone cannot enable it.

## The UX this buys

There is no "Add Location" in the primary client vocabulary. Known folders are
computed under Places, so a person opening Spacedrive for the first time sees
their folders with nothing to configure and no rows to clean up later.

Beyond that, a folder inside a source can be pinned, and pinning is what makes
it a location. The person meets Places, not the internal object name. Ordering
and hidden-state preferences belong to the client contribution model.

## Phases

### L0 — The row

1. Migration: the new `location` shape. No data carries over. Existing rows
   point at `entry_id` values in a table this plan deletes, so preserving them
   would preserve exactly the link being removed.
2. Keep explicit user pins as rows. Compute known folders in the client as
   Places instead of writing default rows.
3. Keep `locations/suggested` as compatibility input for setup surfaces until
   those callers converge on the Places contribution model.

### L1 — Retention reads locations

Cancelled. `map_attached_volumes` continues to derive retained coverage from
source roots. Adding or removing a navigation pin cannot change map fidelity.

### L2 — Watching reads locations

Cancelled. A successful filesystem source walk watches that source root.
Adding or removing a navigation pin cannot change freshness.

### L3 — Delete entries

The sweep. Roughly 64 files reference `entities::entry`.

1. The write path: `database_storage.rs` (1230 lines), `handlers/persistent.rs`
   (695), `change_detection/persistent.rs` (711), `phases/processing.rs` (778),
   `hierarchy.rs`, `path_resolver.rs`, `phases/aggregation.rs`,
   `indexing/verify` (518).
2. The read path: `ops/files/query/{directory_listing, file_by_path, file_by_id,
   media_listing, alternate_instances}`, `ops/search/query.rs`,
   `ops/spaces/get_layout`. Each already has a source-store sibling or needs one.
3. The tables: `entry`, `entry_closure`, `directory_paths`, `content_identity`,
   and the entry columns on `collection_entry`, `user_metadata`,
   `user_metadata_tag`.
4. `DatabaseStorage::get_inode` and `extract_metadata` are used by the arena
   path and move to it rather than dying with the file.

### L4 — Rewire the ops

Eleven `locations.*` ops. Most thin out; three need a decision.

| op | after |
|---|---|
| `add` | pin a path. No walk dispatched, no entry root created. |
| `remove` | delete the row. Records are untouched. |
| `list` | read the new table, counts from the arena. |
| `rescan` | re-walk the subtree, which the arena already does. |
| `suggested` | the default set. |
| `update` | rename only. |
| `validate_path` | keep the risk warnings, drop the "use volume indexing instead" suggestion, which is now what always happens. |
| `enable_indexing` | delete. Indexing is not a per-location switch any more. |
| `triggerJob` | needs a call. Its jobs are the entries-era media pipeline. |
| `import` / `export` (1286 lines) | needs a call. Both are SQL dumps of the entry tables. |

`triggerJob`, `import` and `export` are the three that do not survive
mechanically, and they are the three worth deciding on their own merits rather
than as a side effect of this phase.

### L5 — Sidecars

Unblocked by L3. `P2.5` item 4 of the convergence plan, unchanged, except that
the producers are now source-scoped rather than entry-scoped, so the durable
tier is built against the source store the way `ThumbService` already is over
`sources/<id>/thumbs.pvcache`. The 853-line entries-era `SidecarManager` is not
ported.

## What this costs against deleting locations outright

More surface survives, so L4 is real work rather than eleven deletions. What
it buys is that the sidebar keeps working, the first-run experience gets better
instead of getting a hole in it, and pins remain stable across remounts. L3 is
the same size either way, and L3 is the phase that matters.
