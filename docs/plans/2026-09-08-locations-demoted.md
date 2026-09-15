# Locations demoted

Amends `P4` of `2026-08-22-source-convergence.md`. That phase is titled "Delete
entries" and its unwritten second half assumed locations went with them. They
do not. They lose everything that made them an indexing boundary and keep the
one thing they were always good at, which is naming a folder a person cares
about.

## The rule

**A location is a policy over a path, never an owner of records.**

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

The question it answers now is different and still real: **what do we keep in
full, and what do we watch.** A drive is mapped at low fidelity because keeping
2.1M records per drive in memory is what the LOD exists to avoid. A home
directory should be kept whole and should react to a file appearing in it
within the second. That is a per-subtree policy, it has a cost, and it needs
somewhere to live.

`Retention` already has the parameter and is currently guessing at the answer:

```rust
pub struct Retention {
    pub summarise_rejected: bool,
    pub depth: Option<usize>,
    pub covered: Vec<PathBuf>,
}
```

`map_attached_volumes` fills `covered` from `cache.sources()` roots filtered to
the mount point. That stands in for "paths another walk owns" because there is
nothing better to ask. Locations are the better thing to ask. This phase does
not build a mechanism; it gives an existing parameter its proper owner.

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

Two jobs, both mechanical, both currently done by guesswork.

**Retention.** `map_attached_volumes` fills `covered` from location paths on
the volume instead of from source roots. Overlapping locations are a union: a
path is kept in full if any location covers it, so two pins cannot fight and
neither can a pin inside a pin.

**Watching.** The background map is `Notify::Silent` at `LOW`. A location is
watched and runs above that. The row is what decides whether a watcher is
attached to a subtree, which is the job that earns it a table. A bookmark would
not.

## The policy taxonomy

The old location row bundled every preference a scope could have. The
demotion did not relocate that bundle; it dissolved it, because it was
three different kinds of policy that want different owners:

| policy | question it answers | owner | mechanism |
|---|---|---|---|
| capture | what gets recorded durably | the source | `SourceConfig` in the source row's config column; the walk and the watcher both read it |
| attention | what is kept in full and watched live | the location | `Retention.covered` and `Notify::Each` (L1, L2) |
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

**Retention is a union, never a maximum.** Locations do not carry a fidelity
level to compare. A covered path is kept in full; everything else follows the
volume's map policy.

## The UX this buys

There is no "Add Location" anywhere in the client. The five default rows are
written at library creation from the list `locations/suggested/query.rs`
already computes (`Desktop`, `Documents`, `Downloads`, `Pictures`, plus home),
so a person opening Spacedrive for the first time sees their folders already
there with nothing to configure.

Beyond that, any folder in the map can be pinned, and pinning is what makes it
a location. The person never meets a new concept; they star a folder, and the
retention upgrade rides along without being named.

Defaults are persisted as rows rather than derived each launch. Deriving is
tidier until someone removes Downloads and it returns the next morning, at
which point it needs tombstones, and a tombstone costs more than the row it
saves.

## Phases

### L0 — The row

1. Migration: the new `location` shape. No data carries over. Existing rows
   point at `entry_id` values in a table this plan deletes, so preserving them
   would preserve exactly the link being removed.
2. Write the five defaults at library creation, `origin = "default"`, against
   whichever source contains each path. A default whose path does not exist on
   this machine is not written.
3. `locations/suggested` becomes the default set rather than a list of things
   to prompt about.

### L1 — Retention reads locations

1. `map_attached_volumes` fills `Retention.covered` from the location rows on
   the volume, replacing the `cache.sources()` roots stand-in.
2. A location added or removed at runtime re-derives `covered`. The walk in
   flight is not restarted; the next one sees it.
3. Union semantics and the innermost-source rule, with tests for a location
   inside a location and a location inside a nested source.

### L2 — Watching reads locations

1. Watcher attachment is driven by location rows. A subtree with a location is
   watched with `Notify::Each`; everything else stays `Notify::Silent`.
2. `DatabaseAdapter` and `ops/indexing/responder.rs` go. `ArenaWriter` is the
   only change handler, which is what P4 set up and this finishes.

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
instead of getting a hole in it, and the retention parameter stops guessing.
L3 is the same size either way, and L3 is the phase that matters.
