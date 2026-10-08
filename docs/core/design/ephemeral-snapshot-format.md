# Arena Snapshot Format (v2)

> **Status:** Documents the shipped format (`core/src/ops/indexing/snapshot.rs`)
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
- A **version mismatch or any decode failure quarantines the file** (`<name>.corrupt-<unix seconds>`, one copy kept) and reports "no snapshot." Recovery is a rebuild from the source stores, or a walk for a source with no store records, never a retry against a dead artifact. Bump `SNAPSHOT_VERSION` for *every* struct change; there is no migration path by design — snapshots are caches, not data.

> **Amended 2026-08-18** (`docs/plans/2026-08-18-storage-consolidation.md`, artifact classes): "caches, not data" holds only while the source is attached. A detached drive's snapshot is an availability-bearing replica — the only local copy of a promised capability — so for a detached source, version mismatch and decode failure **quarantine** the file (`.snapshot.bad`) instead of deleting it; a later decoder recovers or a reattach reindexes. Delete-and-reindex applies only when the root is present. Two hardenings follow from the positional fragility: a golden-bytes test pinning the serialized layout of a fixture snapshot (an unbumped field reorder fails CI, not user data — postcard decodes a swap of two same-typed fields *successfully and wrongly*), and zstd frame checksums explicitly enabled so damage surfaces as a decode failure rather than silent garbage.

## Payload, in serialized order

| # | Field | Type | Notes |
|---|---|---|---|
| 1 | `version` | `u32` | `4`. Gate; nothing else is trusted before it. |
| 2 | `source_id` | `Uuid` | Ties the file to its `SourceRecord`; also the filename stem. |
| 3 | `root_path` | `PathBuf` | The indexed root. On restore with the drive present, a mismatch against the record's current root (remount at a new path) discards the snapshot for reindex — absolute paths inside would be stale. |
| 4 | `created_at_secs` | `u64` | Staleness metadata, surfaced via `SnapshotMeta`. |
| 5 | `path_index` | `HashMap<PathBuf, EntryId>` | Full absolute path per entry — the bulk of the bytes. |
| 6 | `entry_uuids` | `HashMap<EntryId, Uuid>` | v7 uuids, only for entries ever assigned one. These are the identities the frontend sees and the record table will persist. |
| 7 | `content_kinds` | `HashMap<EntryId, ContentKind>` | Detected kinds. |
| 8 | `stats` | `IndexerStats` | files/dirs/symlinks/bytes counters. |
| 9 | `name_cache_strings` | `Vec<String>` | The interning pool, by value. |
| 10 | `name_registry_map` | `Vec<(String, Vec<EntryId>)>` | Name → entries (search-by-name). |
| 11 | `arena_entries` | `Vec<SerializableFileNode>` | Live nodes only, in slot order; a node's position is its slot and every id in the file names a position here (v4; v3 wrote `(slot, node)` pairs for every slot ever allocated). Per node: `{name, parent, children, meta}` — pointer-free. `meta` is the same packed 16 bytes as in memory (state/type/size in a u64, mtime/ctime as u32 secs). |
| 12 | `stubs` | `Vec<(EntryId, u64, u32)>` | Summarised directories and the totals standing in for them (v3). |

## Restore

Replayed strictly in order: re-intern the name pool → rebuild the name registry → rebuild the arena, inserting each node in order so it lands on the slot its position names — **a slot mismatch aborts the load**, because `EntryId` *is* the arena slot and every map in the file keys on it → reassemble the index, deriving `id_to_path` from `path_index`, then recomputing rollups and collection flags.

The writer renumbers through the arena's live projection (`Arena::snapshot_projection`): vacant slots on the free list are skipped and every id in every map is rewritten to the dense numbering, so the artifact follows the live tree whether or not the session's arena has compacted (R4, `docs/plans/2026-09-15-source-runtime-reliability.md`).

Restore attaches the snapshot to its source's partition (never a shared index), marks `root_path` as indexed, and sets the slot detached when the root is absent — which is what makes an unplugged drive browsable read-only.

### When the snapshot cannot be used

A partition whose snapshot is missing, unreadable, or stamped with another format version does not re-walk its sources. `VolumeIndex::ensure_restored` runs one restore attempt per drive per session; when the snapshot load yields nothing and the drive is attached, it starts one background task that refills the arena from every registered source's store (`fill_arena_from_store`, the same fill a library restore uses through `rebuild_from_store`). The gate resolves as soon as the task is started, because every reader on the drive funnels through it; each source root is marked in progress until its fill lands, so the coverage heal dispatches no walk over it and a directory listing serves the source's store meanwhile. The fill reads `rebuild_entries_page` (the record, path and facet columns, no content join) in pages of 2,000 rows and takes the arena's write lock one page at a time. When a source lands it keeps the uuids its store holds, is marked restored and indexed, and announces its root, which flips its routing to the arena and arms its filesystem watch. The unusable artifact stays beside the slot as `<name>.corrupt-<unix seconds>` (see Integrity) and the slot is clear for the next save, which writes the rebuilt arena in the current format. A partition a library restore already refilled (`rebuild_quiesced`) resolves as restored without reading its stores again.

The rebuild runs for a source whose registry row carries a record count. The count is written when a snapshot is saved, so it is the evidence that a map existed and was lost; a source never walked to completion has no count and keeps answering from its store without an arena, which is the R6 routing (`docs/plans/2026-09-15-source-runtime-reliability.md`).

What a store rebuild does not restore: the rest of the drive map outside the registered sources, and directory stubs. The discovery pass still walks a drive whose snapshot is gone when it maps whole drives (`VolumeIndex::restored_from_snapshot` is its test), and that walk keeps every uuid the rebuilt arena already holds. A source whose store has no records is the only one the coverage heal walks.

Measured on a 4 vCPU cloud machine (release build, `a_million_record_store_rebuilds_in_seconds` in `volume_index.rs`, run with `--ignored`): a store of 1,010,100 records (1M files in 10,100 directories) rebuilds in 18.9 s, of which 13.9 s is reading the pages and the rest is arena inserts; 101,010 records take 1.7 s. Reading through the listing select (`all_entries_page`, with the content join and its six columns) took 28.2 s of a 33.1 s rebuild, which is why the rebuild has its own select.

## Known redundancy (future v3)

`path_index` stores every full absolute path even though arena parent chains already encode the tree. This mirrors the same redundancy in RAM; the zero-onboarding memory-diet work (`zero-onboarding-startup.md` § change 3) removes the stored path maps in favor of parent-chain resolution, and the format shrinks with it — likely to arena entries + name pool + the small maps, with paths derived on load. That change is a version bump like any other: old files are quarantined and the arena is rebuilt from the stores.

## Integrity

No checksums beyond zstd's frame checksum, no journaling. Cells of this cache are recomputable by definition; corruption manifests as a decode failure, which quarantines the artifact and rebuilds the arena from the source stores (see Restore). Anything needing stronger guarantees belongs in the portable artifact format, not here.

Note that rollup data (`FileNode.subtree_bytes`) is deliberately **not serialized**: directory sizes are recomputed from live entries during restore, so the format carries no derived totals that could drift from their inputs.
