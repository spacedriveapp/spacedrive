# The Storage Map

> A machine needs full local search over every file on it and a size figure the
> analyser can trust, and neither is the same question as what someone chose to
> keep. `2026-08-22-source-convergence.md` P2.7 drew that line and left the
> completeness half open. This is the completeness half.
>
> **Related.** P2.7 in the convergence plan owns the volume index and the
> source. `2026-08-25-filesystem-intelligence.md` is what the map is for.

## What this is for

The Analyzer renders a live sunburst of a volume: rings of directories sized by
what is under them, updating as the disk changes. Every tool that does this
today takes a snapshot of a moment. DaisyDisk and Disk Space Analyzer have no
resident process and no watcher, so their picture is true when you press scan
and drifts from then on.

Spacedrive has both. `EntryNode.subtree_bytes` is maintained incrementally:
every add and remove calls `bump_ancestor_bytes`, which walks the parent chain
to the root. So a file landing in `~/Desktop` corrects every ring from Desktop
outward with no rescan. The map being live is not a feature to build, it is a
property of the arena that nothing currently surfaces.

What is missing is the map.

## Measured, 2026-08-27

One 990 GB drive, 54.2 GB free.

| | |
|---|---|
| files and directories on the volume | 11,131,151 (`df -i`) |
| bytes in use | 866 GB |
| **mapped today** | **271 GB, 1,678,592 entries, 759 MB resident** |
| arena cost per entry | 452 B |
| snapshot cost per entry | 61 B (103 MB for 1.68M, zstd) |

What is missing, and why:

| | |
|---|---|
| `~/Library` | 231 GB, excluded by rule |
| `/Applications` | 55 GB, never visited |
| `/Library` | 18 GB, never visited |
| dev directories and gitignored files under home | ~190 GB, excluded by rule |
| `/System`, `/private`, `/usr` | the remainder, never visited |

Two causes, and they are independent. **Nothing walks the volume at all**: the
bootstrap walks the source root, and no job maps the drive, so the top-level
directories beside `Users` have never been enumerated. **Rules prune the rest**:
an internal volume walks with `RuleToggles::default()`, which drops `~/Library`,
`node_modules`, caches and anything gitignored.

Home alone is 691 GB on disk and 271 GB mapped. Even the folder that was walked
is 40% covered.

## Traversal is cheap, retention is not

There is no way to learn a directory's size without enumerating it. APFS offers
no per-directory total, which is why `du` takes seconds. So a walk visits
everything whatever the policy, and the policy is only ever about **what to
keep**.

That reframing is what makes this affordable. The cost is not the walk, it is
the entries the walk retains.

## Level of detail

A sunburst needs structure and totals at the rings it draws, not every leaf.
Retain accordingly, the way a renderer picks a mesh: full detail where someone
is looking, a single summarised shape where they are not.

Measured on the same drive:

```
directories within 7 levels of the volume root:  118,859   ≈ 54 MB
every entry on the volume:                    11,131,151   ≈ 5.0 GB
```

So a complete and accurate seven-ring structure costs 54 MB. Home at full
fidelity plus that structure is roughly 810 MB, which is about what the daemon
holds today for two rings and a fifth of the disk.

**Depth is a proxy; size is the signal.** Fixed depth is crude in both
directions: `~/Projects/…/core/src/ops` is depth 8 and matters, while a depth-3
cache directory does not. What the sunburst needs is to keep a directory's
children while its slice is large enough to see, and to collapse it below that.
Disk Space Analyzer shows the same instinct in its own outline, where everything
under a threshold becomes a single *Small items* row. The difference here is
that the collapsed row carries a true total and can be expanded live.

The retention rule, then:

- Inside a registered source, retain everything. Fidelity is the point of
  keeping something.
- Outside one, retain a directory's children while its subtree is worth a
  visible slice. Below that, keep the directory and summarise what is under it.

## Stubs

A summarised directory is one entry carrying the totals of everything beneath
it and no children.

The arena already maintains `subtree_bytes` up the ancestor chain, so a stub's
total propagates exactly as a real subtree would and every outer ring is correct
without knowing anything was elided. What a stub needs beyond that is
`file_count`, which the old library had as `entries.file_count` and the record
table has not carried since. `2026-08-22-source-convergence.md` P2.5 wants it
anyway.

A stub must be **distinguishable from an empty directory**. Listing one and
seeing nothing reads as a bug rather than as detail that has not loaded yet.
`EntryNode` carries `collection_flags`, a per-entry bitfield with room for it.

**Drilling in refines.** `directory_listing` indexes the directory it lists, so
clicking into a summarised region materialises it in place and it stays. The
pop-in already works; nothing needs building for it. That is a good sign the
shape is right.

## Keeping a stub honest

A stub's total is true when it is counted and stale afterwards. Someone
downloads 4 GB into `~/Library/Caches` and the ring does not move.

Delta tracking cannot fix this. `FsEvent` carries a path and a kind, not a size,
so a delete under a stub is a change of unknown magnitude and the arithmetic
would be a guess.

So: **a change under a stub marks it dirty, and a dirty stub is recounted**,
debounced until activity settles. A count-only pass over `~/Library` takes
seconds and holds nothing, so the ring snaps back to the truth shortly after the
disk does. Walked regions stay exactly live; summarised regions are live within
a few seconds of a change.

This needs one change to routing. `EphemeralIndexCache::watched_root_for_change`
currently drops a change whose parent is not indexed, which is correct while
every unindexed path is genuinely unknown and wrong once some of them are
deliberate summaries. It should climb to the nearest indexed ancestor and, if
that ancestor is a stub, mark it dirty.

## The order things run in

Priorities already exist (`JobPriority::{LOW, NORMAL, HIGH, CRITICAL}`), so this
is a matter of dispatching in the right order rather than new machinery.

```
1.  source walk       NORMAL   full fidelity, ~30s, the files someone kept
2.  volume map        LOW      background, LOD, fills the analyser
3.  content identity  LOW      after both
```

The count climbs immediately on the files that matter, the map fills in behind
it, and hashing comes last. A person watching the first job sees their own files
arrive rather than waiting on `/System`.

## The setting

Per volume, because a boot drive and an archival drive want opposite answers.

| | retains | resident |
|---|---|---|
| **Summarised** (default) | sources in full, structure elsewhere, stubs below the threshold | ~810 MB |
| **Complete** | every entry on the volume | ~5.0 GB |

Complete is what someone picks when they want to search every file on the
machine and are willing to pay for it. Summarised answers *what is on this
drive* exactly, and *where is this file* for everything anyone is likely to
search.

Expanding a stub in the analyser is the third mode and needs no setting: it is
an on-demand walk of one subtree, which is what browsing already does.

## Phases

1. **Map the volume.** A job that walks a volume index and dispatches at `LOW`
   after the source walk. Nothing does this today, which is the actual bug: it
   is why `/Applications` has never been enumerated.
2. **Count without keeping.** A rule that rejects a directory currently prunes
   the subtree. It should instead descend in count-only mode and leave a stub
   carrying `subtree_bytes` and `file_count`.
3. **Rollups on the record.** `file_count` beside `subtree_bytes`, in the arena
   and in `facet_file`, which P2.5 already wants.
4. **Dirty and recount.** Route a change under a stub to its nearest indexed
   ancestor and mark it; recount debounced.
5. **The retention threshold and the setting.** Size-weighted retention outside
   sources, per-volume Summarised and Complete.

Phases 1 and 2 are what the analyser needs to draw anything true. 4 is what
makes it live, which is the part no other tool has.

## What this is not

Not a second index. The map is the arena, which already exists and is already
the read path for every listing. This adds coverage and a retention policy to
it.

Not a persistence decision. Nothing here writes a `data.db`. A volume index is
memory plus a snapshot, and what gets kept durably is still exactly what someone
registered as a source.
