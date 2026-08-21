# Ephemeral Snapshot Format (v2)

> **Status:** Documents the shipped format (`core/src/ops/indexing/ephemeral/snapshot.rs`)
> **Captured:** 2026-08-17
> **Scope:** The internal session-restore format — one file per source, named `<source_id>.snapshot` in the volume-index cache dir. This is *not* the portable snapshot artifact (mtree export, Merkle roots) designed in `cross-location-content.md`; that format is deliberately stable and other-tools-readable, while this one is internal, version-gated, and optimized for "the app opens populated in under two seconds."

## Container

A single standard **zstd frame** (level 6, multithreaded encode) wrapping one **postcard**-serialized `IndexSnapshot` struct. Because the outer layer is plain zstd, `zstd -d < file` yields the raw postcard bytes — useful for size inspection, nothing more (postcard is not self-describing).

Writes follow the full crash-safe protocol, each step earned by a corruption found in real use:

1. **Uniquely named temp file** (`<name>.tmp.<uuid>`) — concurrent saves sharing one tmp path interleaved their writes and published garbage through the rename.
2. **`fsync` the data before the rename, and the directory after** — rename-without-sync persisted the rename across a reboot while the file's blocks were still in the page cache, leaving truncated snapshots at the final names.
3. **Per-source save serialization and a save invariant** (owned by the cache, not this format): saves funnel through the session's restore gate, and a partition that was not seeded from the existing snapshot may not overwrite it — a fresh session's few browsed directories cannot replace a full drive index. Saves with an unchanged entry count skip the rewrite entirely.

Observed density on real data: a ~1.06 MB payload compresses to ~214 KB (≈4.9:1), matching the format's 70–80% expectation. Cost scales with entry count and path lengths.

## Encoding

Postcard: varint (LEB128) integers, length-prefixed strings and collections, **no field names or tags — struct field order is the schema.** Any reordering, insertion, or type change to `IndexSnapshot` is a format break. That fragility is owned by policy rather than avoided:

- The `version` field is decoded and checked first.
- A **version mismatch or any decode failure deletes the file** and reports "no snapshot." Recovery is a clean reindex, never a retry against a dead artifact. Bump `SNAPSHOT_VERSION` for *every* struct change; there is no migration path by design — snapshots are caches, not data.

> **Amended 2026-08-18** (`docs/plans/2026-08-18-storage-consolidation.md`, artifact classes): "caches, not data" holds only while the source is attached. A detached drive's snapshot is an availability-bearing replica — the only local copy of a promised capability — so for a detached source, version mismatch and decode failure **quarantine** the file (`.snapshot.bad`) instead of deleting it; a later decoder recovers or a reattach reindexes. Delete-and-reindex applies only when the root is present. Two hardenings follow from the positional fragility: a golden-bytes test pinning the serialized layout of a fixture snapshot (an unbumped field reorder fails CI, not user data — postcard decodes a swap of two same-typed fields *successfully and wrongly*), and zstd frame checksums explicitly enabled so damage surfaces as a decode failure rather than silent garbage.

## Payload, in serialized order

| # | Field | Type | Notes |
|---|---|---|---|
| 1 | `version` | `u32` | `2`. Gate; nothing else is trusted before it. |
| 2 | `source_id` | `Uuid` | Ties the file to its `SourceRecord`; also the filename stem. |
| 3 | `root_path` | `PathBuf` | The indexed root. On restore with the drive present, a mismatch against the record's current root (remount at a new path) discards the snapshot for reindex — absolute paths inside would be stale. |
| 4 | `created_at_secs` | `u64` | Staleness metadata, surfaced via `SnapshotMeta`. |
| 5 | `path_index` | `HashMap<PathBuf, EntryId>` | Full absolute path per entry — the bulk of the bytes. |
| 6 | `entry_uuids` | `HashMap<EntryId, Uuid>` | v7 uuids, only for entries ever assigned one. These are the identities the frontend sees and the spine will persist. |
| 7 | `content_kinds` | `HashMap<EntryId, ContentKind>` | Detected kinds. |
| 8 | `stats` | `IndexerStats` | files/dirs/symlinks/bytes counters. |
| 9 | `name_cache_strings` | `Vec<String>` | The interning pool, by value. |
| 10 | `name_registry_map` | `Vec<(String, Vec<EntryId>)>` | Name → entries (search-by-name). |
| 11 | `arena_entries` | `Vec<(usize, SerializableFileNode)>` | Per node: `{name, parent, children, meta}` — pointer-free. `meta` is the same packed 16 bytes as in memory (state/type/size in a u64, mtime/ctime as u32 secs). |

## Restore

Replayed strictly in order: re-intern the name pool → rebuild the name registry → rebuild the arena, inserting each node at its recorded slot index — **a slot mismatch aborts the load**, because `EntryId` *is* the arena slot and every map in the file keys on it → reassemble the index, deriving `id_to_path` from `path_index`.

Restore attaches the snapshot to its source's partition (never a shared index), marks `root_path` as indexed, and sets the slot detached when the root is absent — which is what makes an unplugged drive browsable read-only.

## Known redundancy (future v3)

`path_index` stores every full absolute path even though arena parent chains already encode the tree. This mirrors the same redundancy in RAM; the zero-onboarding memory-diet work (`zero-onboarding-startup.md` § change 3) removes the stored path maps in favor of parent-chain resolution, and the format shrinks with it — likely to arena entries + name pool + the small maps, with paths derived on load. That change is a version bump like any other: old files self-delete, sources reindex.

## Integrity

No checksums beyond zstd's frame checksum, no journaling. Cells of this cache are recomputable by definition; corruption manifests as a decode failure, which triggers the delete-and-reindex path. Anything needing stronger guarantees belongs in the portable artifact format, not here.

Note that rollup data (`FileNode.subtree_bytes`) is deliberately **not serialized**: directory sizes are recomputed from live entries during restore, so the format carries no derived totals that could drift from their inputs.
