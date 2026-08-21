# Building before the teardown — brief

> For work landing on the mounts, byte plane, block cache and hot tier while the
> entries world is still standing. Read once before starting. Companion to
> `2026-08-20-entries-teardown-execution.md`, which is the register of what dies.
>
> The short version: the sources path is the only substrate. Everything below is
> either a hazard that will corrupt what you build, a decision already taken, or
> a primitive that already exists and must not be reinvented.

## The snapshot is a cache holding one durable field

`ephemeral.snapshot` serializes seven things: arena nodes, name cache, name
registry, `path_index`, `content_kinds`, stats, and `HashMap<EntryId, Uuid>`.

Six of those are pure cache — re-walk the disk and you get them back identically.
The seventh is not. Record uuids are minted `Uuid::now_v7()` at first sight, so a
re-walk produces *different* ones. The snapshot is the only artifact in the
filesystem lineage holding state that cannot be regenerated.

And it is governed by cache rules. Five paths discard it — version mismatch,
restore failure, `source_id` mismatch, and this, at `cache.rs:414`:

```rust
// A drive that came back at a different mount point has absolute
// paths from the old mount baked into the snapshot. Reindexing the
// present drive is cheaper than being subtly wrong; the stale
// snapshot goes so the next save is clean.
if record.root.exists() && meta.root_path != record.root {
    let _ = std::fs::remove_file(&snapshot_path);
```

Correct for a cache. Wrong for identity. A drive that returns at a different
mount point has every uuid on it regenerated.

This is nearly harmless today, because tags key to `entry.uuid` in `library.db`.
It stops being harmless the moment anything durable keys to a record uuid — see
the ordering constraint on T4.9 in the execution doc.

The archive lineage does not have this problem: `record_overlay` lives in
`registry.db` specifically so that deleting a source index cannot touch it. The
filesystem lineage does not lack persistence; it lacks separation.

**If you are working near this:** treat arena uuids as cache until bindings move
out of the snapshot. Do not key anything durable to them, and do not add a sixth
discard path without noticing what it takes with it.

## Known failing before you start

`ephemeral_watcher_test::test_ephemeral_watcher` fails on a clean tree, and has
nothing to do with the gate fixes — it fails identically with them stashed. It is
the top open hazard for mount work, because it means a renamed file leaves the
index and does not come back under its new name.

What the run shows: the fs-watcher emits one rename, and every batch arriving at
`change_detection::apply_batch` reports `0 renames, 1 removes`. macOS FSEvents
usually delivers a rename as two single-path events rather than one paired event,
and `crates/fs-watcher/src/platform/macos.rs:475` turns each half into a create or
a remove depending on whether the path currently exists. Inode-based pairing
exists to rejoin them (`pending_removes`, and the rename match inside
`process_create`) but the pair is not being rebuilt here.

For a share, that reads as a file disappearing on rename until the directory is
re-indexed. Worth fixing before a frontend depends on the arena reflecting the
filesystem, and worth its own investigation rather than a patch on the way past.

## Already fixed — what changed under you

The P1 gate fixes are in. They are listed here because they changed behaviour the
byte plane will rely on, not as work to do.

- **Repeat sightings update metadata.** `add_entry` used to take a duplicate-path
  early return and discard the metadata it was handed, so a modified file's size
  and mtime never moved. Both caches key on `source_version(size, mtime)`; with
  the old behaviour the key never changed and a modified file was answered with
  its old bytes while reporting fresh. A repeat add now adopts the new size and
  timestamps and carries the delta up the ancestor chain.
- **Removal takes the whole subtree.** `clear_directory_children` removed only
  direct children, leaving descendants resident with a severed parent chain,
  where the next add found them and took the duplicate branch. Removal now
  recurses, drops the name from the search registry, and releases the arena slot.
  `EntryId`s are still never reused.
- **A mount point is not an identity.** `SourceRegistry::register` used to fall
  through to exact-root matching when a fingerprint matched nothing, and then
  overwrite the stored fingerprint — so a different drive at a familiar path
  inherited the previous drive's source id and snapshot. Since blocks key on
  `source_id`, that was one drive's cached bytes served for another drive's file.
  A fingerprint that matches nothing now registers a new source. Two records can
  therefore share a root, and `resolve` breaks the tie on `last_seen_secs`.
- **Snapshots track a dirty flag, not an entry count.** A rename leaves the count
  identical, so rename-only changes never reached disk.
- **Registry writes are fallible.** `register_source` returns `Result`. The
  no-persistence mode is now explicit (`SourceRegistry::in_memory`) rather than a
  write to a `/nonexistent` sentinel path, so a failure from `save` always means
  something is actually wrong.
- **Snapshot restore checks `source_id`,** not just the root path.

**Still outstanding from that register:** attachment is decided by
`root.exists()` at `cache.rs:169, 210, 408, 178`. Presence is not identity. The
structural fix is T2.0b, where attachment derives from the volume's `is_online`
plus a root resolving under its current mount point — it needs the volume manager
and belongs with that change rather than a half version now.

## The invariant

The mounts stack references the dying schema **zero** times:

```
core/src/service/mounts/mod.rs:0
core/src/service/mounts/peer.rs:0
core/src/service/mounts/webdav.rs:0
```

Keep it at zero. No `entities::entry`, `content_identity`, `location`,
`sidecar`, or anything under `ops/locations`, `ops/indexing/database_storage`,
`change_detection/persistent`, `service/sync`, `infra/sync`.

If something appears to need one of those, the substitution is:

| reaching for | use instead |
|---|---|
| a location root | the source registry root (longest-prefix `resolve`) |
| an entry uuid | the arena's record uuid (`get_or_assign_uuid`) |
| `content_identity` for cache keys | `source_version(size, mtime)` until P2 lands |
| a sidecar row | the per-source hot tier |
| an entry row for stat | `EntryMetadata` from the arena |

This is not only hygiene. The mounts stack having never needed the entries world
is the strongest evidence the teardown is safe, and that evidence is worth
preserving.

## Decisions already taken

- **One file per source.** `source.db` holds the rebuildable generation, the
  identity ledger and the assertions together. The deciding fact is that source
  pools run in WAL (`crates/archive/src/source.rs:31`) and SQLite's atomic
  commit across `ATTACH`ed databases needs rollback mode. `SourceDirs` already
  names it `source.db`; `crates/archive` still says `data.db` and has to move —
  do not propagate `data.db` into new code.
- **Content ids are convergent**: `uuid_for(hash) = v5(CONTENT_NAMESPACE, hash)`,
  derived and never assigned. Not `deterministic_uuid`, which names the
  mechanism rather than the thing.
- **Assertions are append-only, ordered by HLC.** Not mutate-in-place. This is
  what keeps metadata history reachable later without a retrofit; converting a
  mutate-in-place store into an append-only one afterwards is the expensive
  version of this decision. `infra/sync/hlc.rs` survives the sync deletion for
  exactly this reason.
- **Volume is the medium, source is the index.** Capacity, filesystem, speed,
  removable and online belong to `volume`; root, record count and snapshot state
  belong to `source`; `source` carries a nullable `volume_uuid`. Attachment
  derives from the volume being online plus a root that resolves under its
  current mount point.

## Primitives that already exist

**`source_version(size, mtime)`** validates both tiles and blocks and now lives at
`core/src/infra/source_version.rs`. It used to sit in `crates/pvcache`, which
never called it — the cache takes an opaque `u64` — so it was a caller-side helper
in a crate that had no opinion about it. Import it from core. One definition,
ever: two that drifted would show up as one cache serving stale data while the
other refreshed.

It is a validator, not a content identity. It says "these bytes may have changed",
cheaply and without reading the file. It cannot say two files hold the same
content — that is the convergent content uuid, and it costs a hash.

**`SourceDirs`** (`core/src/infra/source_dirs.rs`) owns the per-source layout:
`sources/<id>/{ephemeral.snapshot, thumbs.pvcache, source.db}`. Put the block
cache's L2 at `sources/<id>/blocks/` rather than a sibling tree. Then forgetting
a source is one directory removal that takes its snapshot, tiles, database and
blocks with it, and a detached drive's cache scopes itself.

**The rebuildable artifact class.** Snapshots, tiles and blocks are the same
kind of thing: keyed by identity plus a size/mtime version, safe to delete at
any moment, never authoritative, rebuilt by being read again. New caches join
that class rather than inventing terms.

**`ByteProvider` and its resolver** are the only entry point a frontend calls.
Range parsing, error mapping and detached checks live there once. A frontend
that reimplements any of them is the bug the trait exists to prevent.

## Not yet

- **Content-addressed block keying.** Deferred until content identity is
  available at mount time, which happens when T2.7 puts convergent uuids in
  `source.db`. It is a key swap behind the same interface — leave the seam.
- **Writes.** Read-only across every frontend.
- **The ledger, credential broker, edge cache, change channels.** These are the
  cost of renting storage from a stranger. When the far end is a device you own,
  the layer collapses.
- **Deleting anything from the entries world.** That is the teardown's job and
  it happens after this work lands. Do not start it opportunistically; the
  register in the execution doc exists so the deletion is one reviewed pass.

## Baseline

```
cargo check                                         # default-members
cargo check -p sd-core --all-targets --features wasm # tests and examples
cargo check -p sd-native                            # gpui tree, not a default member
```

`--features wasm` is not optional on the second command: `wasm_job_execution_test`
reads `Core::plugin_manager`, which is `#[cfg(feature = "wasm")]`. Without it that
one test fails to compile, and it is easy to mistake for a regression.

Do not use `--workspace --all-targets`: it pulls
`apps/tauri/crates/file-opening-windows` into the graph, which does not compile
on macOS. That failure is pre-existing and unrelated.

Disk filled up once during this work — `target/` reached 43 GB. If a build dies
with `No space left on device`, `target/debug/incremental` is the thing to
remove. Not `cargo clean`.
