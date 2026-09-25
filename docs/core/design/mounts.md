# Mounts

> **Status:** Design — first implementation landed (see "Implementation status")
> **Captured:** 2026-08-19
> **Revised:** 2026-08-20 — frontend ordering, cache policy and renditions
> reworked. This doc previously led with a validation mount and treated the
> native module as a later phase; that ordering is inverted below: a native
> filesystem module is the destination and a loopback server is the fallback,
> for the read-ahead and seek reasons argued under "Mount frontends". The
> cache also moved ahead of the SMB frontend — a mount baseline measured
> without a cache measures re-fetching, not architecture.
> **Companions:** `docs/plans/2026-08-18-storage-consolidation.md` (the four stores and contracts this reads from), `docs/plans/2026-08-17-record-table-transplant.md` (execution context), `docs/core/design/cross-location-content.md` (content identity), `docs/core/design/ephemeral-snapshot-format.md` (the metadata replica format)

## The feature

Any indexed content in a library — a cloud volume, a location, a source on
another paired machine — presented to the operating system as an ordinary
local volume. Files appear in Finder and open in unmodified applications;
bytes stream on demand from wherever the content actually lives and occupy
no local disk until touched. A machine's visible storage becomes the union
of everything its library can reach: its own disks, its peers' disks,
attached clouds, and the read-only ghosts of drives that are in a drawer.

The index is what makes this cheap. A mount's hot path is overwhelmingly
metadata — `readdir`, `stat`, `getattr` — and the living index already
answers those from an in-memory arena in microseconds at million-file scale,
restores from snapshot in seconds, and keeps answering for sources that are
detached. The mount is a third face on data the Explorer and CLI already
read: same arenas, same stores, now speaking the filesystem API.

## The unit problem: volumes, locations, sources

The request "mount any cloud volume or location on any machine" trips over
a real seam in the current model:

- **Clouds are built on volumes.** `volumes.add_cloud` constructs a
  `Volume` whose `mount_point` is a URI (`s3://bucket`), whose I/O runs
  through `VolumeBackend` (OpenDAL), and whose identity is
  `VolumeFingerprint::from_network_volume(service, identifier)`.
- **Locations are built on the index.** Today a `locations` row anchors an
  entry subtree in `library.db` (root `entry_id`, lazy `volume_id` FK);
  under the consolidation plan a Location becomes a subtree-with-policy
  inside a source (contract 1). Location code and the source registry are
  currently disjoint — no `source_id` appears anywhere in the location
  path.
- **Sources are the index.** `SourceRecord` in the ephemeral registry is
  keyed by volume fingerprint, owns the arena/snapshot for a filesystem,
  and is the unit the consolidation plan builds `source.db` around.

These three cannot each be a mountable unit; the design collapses them.

**A mount target is a source subtree plus a byte provider.** Everything
mountable resolves to the triple:

```
(source_id, subtree_root, ByteProvider)
```

- A **cloud volume** resolves through its fingerprint to the source it
  registers when indexed (`volumes.index` already binds
  `SourceRecord.fingerprint` to the volume fingerprint). Its byte provider
  is the volume's existing `VolumeBackend` — which already implements
  `read_range(Range<u64>)`, the exact primitive a mount needs.
- A **location** resolves to a subtree of its volume's source. This is the
  consolidation plan's contract 1 arriving with a use case attached:
  mounting a location is meaningless until locations are subtrees of
  sources, so the mount work consumes that migration rather than competing
  with it.
- A **remote source** is any source whose owning device is another paired
  device in the library. Its byte provider is the peer protocol below.
- A **detached source** (archived drive) resolves with no byte provider:
  metadata serves from the snapshot, reads fail legibly or fault to a
  replica when the content map knows one.

Volumes and locations remain how sources come to exist and how subtrees are
scoped; the mount layer never learns about either. One resolver
(`mount target → triple`) contains all the vocabulary translation.

## Architecture

Four planes, strictly layered:

```
 OS ──► mount frontend        FSKit / WinFsp / FUSE  ·  SMB loopback
            │ readdir/stat/open/read
            ▼
        metadata plane        arena (SourceSlot) → source.db when durable
            │ resolve(external_id) → provider
            ▼
        byte plane            ByteProvider: local | cloud | peer | replica
            │ read_range
            ▼
        block cache           L1 memory / L2 disk, content-addressed,
                              rebuildable, pinnable, read-ahead driven
                              by the frontend
```

### Mount frontends

Read-only first, everywhere. The frontends divide into two classes, and
the distinction is not convenience — it is who owns the read pattern.

**Native, one per platform. The product.**

1. **FSKit** (macOS 15+) — user-space, no kext, distributable inside the
   app.
2. **WinFsp** on Windows, behind the same provider interface.
3. **FUSE** on Linux.

**Loopback server, one for everything else. The fallback.**

4. **SMB on loopback** — the daemon runs an SMB2 server bound to
   127.0.0.1 and the OS mounts it with its own client. No driver, no
   extension, no elevation. Covers macOS 13–14, machines where an
   extension cannot be installed, first-run trials before any install
   prompt, and re-export of a mounted source to other machines on a LAN.

NFS loopback is dropped as a candidate. It offers nothing SMB does not,
and macOS speaks SMB natively and better.

**Why native is the destination and not the fallback.** The workload this
feature exists for is scrubbing multi-gigabyte media, so the whole game is
controlling read-ahead and seek behaviour. Through a loopback server the
kernel's own client decides read sizes and prefetch depth and we can only
negotiate from outside; through a native module the arm-on-seek policy,
block size, and prefetch depth in the cache section below are ours to set.
A loopback mount also presents as a *network* volume, which changes
Spotlight behaviour and how some direct-I/O applications treat it for
media. Native is measurably the harder build and it is the one that makes
the feature feel like a local disk.

File Provider is rejected: dataless-file semantics and the
`~/Library/CloudStorage` container break exactly the direct-I/O
applications (NLEs, DAWs, CAD) this feature exists to serve.

**The WebDAV surface is a measurement baseline, not a path.** The daemon
already serves one (see "Implementation status") and it is worth keeping
as plain interop for clients we do not ship, but the macOS WebDAV client
is the weakest of the three by a distance and no product decision should
rest on its numbers. Where an honest no-install baseline is wanted, it is
the SMB adapter that provides it.

### Metadata plane

`readdir`/`stat` serve from the source's arena — the same `SourceSlot` the
Explorer queries — with `subtree_bytes` rollups backing directory sizes.
Nothing new is built here; the mount is a consumer.

For **remote sources**, metadata is replicated, not fetched per-call: the
mounting machine pulls the source's snapshot (the portable artifact the
ephemeral snapshot format already defines, keyed by source id) and refreshes
it by generation. A `readdir` never crosses the network. This keeps browse
at local-arena speed, makes remote mounts identical to detached-drive
browsing when the peer goes offline, and avoids inventing a chatty remote
listing protocol — one which does not exist today and should not.

### Byte plane

One trait, four implementations:

```rust
trait ByteProvider {
    async fn open(&self, external_id: ..) -> Result<ByteHandle>;  // metadata-only, no hashing
    async fn read_range(&self, handle: .., range: Range<u64>) -> Result<Bytes>;
}
```

- **Local** — passthrough to the real path; the mount adds nothing but
  uniformity.
- **Cloud** — the existing `VolumeBackend` (`volume/backend/cloud.rs`),
  whose `read_range` already streams from OpenDAL. Mounting a cloud volume
  on the device that added it requires no new I/O code.
- **Peer** — a new ALPN (`spacedrive/byterange/1`) beside the existing
  five. The transport, connection cache, slug→endpoint routing, and the
  `is_path_allowed` authorization gate all exist; what is new is range
  semantics. The current file-transfer PULL cannot be bent to this: its
  request carries no offset, and its server hashes the entire file before
  responding — an `open()` that costs a full read. The range protocol
  needs metadata-only open, offset/length requests, pipelining, and
  cancellation of in-flight reads. Cloud volumes reachable only through a
  peer (credentials live on the owning device) chain: peer provider on the
  mounting machine, cloud provider on the owner.
- **Replica** — content-addressed reads for detached sources, resolved
  through the content map to any attached source or sidecar-class replica
  holding the same hash.

### Block cache

Streamed ranges land in a local block cache so scrubbing and repeated reads
do not re-fetch. Keying follows content identity: blocks for records with a
confirmed hash are content-addressed (shareable across sources holding the
same bytes); before identity is known, blocks key by
`(source_id, external_id, generation)` and are invalidated by generation
bump. The cache is a rebuildable artifact class per the consolidation
doc's taxonomy — deletable, size-capped, never authoritative. **Pinning**
is the same cache with an eviction exemption: pin a file or subtree and its
blocks are fetched eagerly and never evicted, which is the offline story.

Two tiers, named so the logs can say which one answered: **L1** in memory,
**L2** on disk, with the provider counting as L3. Read-ahead is a policy
the native frontend owns and must expose: a start depth, a depth that arms
on seek (the scrub case), a cap on bytes in flight, and a cap on
concurrent prefetches — tuned per provider class, because a LAN peer, a
detached-drive replica and a WAN cloud volume have different sweet spots.
This is the half of the streaming experience a loopback mount cannot give
us, and the reason the frontend ordering above is what it is.

### Renditions

A mount can serve something other than the original bytes. A worker
prepares reduced-quality, intra-frame renditions of media ahead of time and
the mount serves those instead, so an editor opens what looks like the
original and reads a fraction of the bytes. Prepared, never transcoded in
the read path: until a rendition exists the mount serves the original at
full quality.

The shape fits here with one inversion. A rendition is a rebuildable
artifact class, like thumbnails and sidecars: keyed by content identity,
deletable, never authoritative, and produced by an ordinary job. The
difference is where the job runs — on whichever device in the library has
the cycles, which may be the machine asking, a server in the chain, or the
device that owns the content. Nothing leaves the library to be processed.

Scope fence: renditions are a phase of their own, after the cache. They do
not change the byte plane — a rendition is just another provider result
for a given (record, quality) — and the mount must degrade to the original
when none exists.

## Why the trust layer is not in this document

Worth stating once, because it keeps the scope honest. A mount over rented
multi-tenant storage needs a large trust layer: a control plane issuing
scoped credentials, mid-session revocation, an edge cache in front of
far-away object storage, a signed metadata ledger with per-write receipts,
and separate channels for invalidation, authorization and block demand.
None of that makes files fast. It makes rented bytes safe.

When the far end is a device the user owns, that layer collapses. We need
the mount, the cache, the renditions, and a server posture — one binary on
a NAS or a VPS that is simply a peer without a display. The transport for
that already exists; what is missing is the deployment story, and it is
tracked outside this document.

The one property that trust layer buys — instant metadata for a source
that is not currently reachable — we already have from the living index.

## Semantics

- **Reads only, initially.** Streaming reads are on-demand byte-range
  fetch — not sync, and they do not reopen the deferred sync work. The
  write design (write-through to the owning device, or write-local and
  drain) is a separate document; multi-writer coherence is out of scope
  until deliberately chosen.
- **Detached mounts are a feature, not an error.** A mounted archived
  drive browses at full speed from its snapshot; `open()` returns a clear
  error naming the drive, or serves from a replica when one exists. The
  mount surfaces the same availability the sidebar already shows.
- **Freshness follows the index.** Local and cloud sources are as fresh as
  their watchers/reindex policy; remote sources are as fresh as the last
  snapshot generation pulled. The mount never presents stronger
  consistency than the index has — and says so (volume-level metadata can
  expose last-refreshed).

## How this lands in the existing model

A mounted target appears inside Spacedrive as a volume like any other —
`VolumeType::Virtual`, registered through an explicit registration path
(the `register_cloud_volume` shape), never through OS detection. Control
surface is ops (`mounts.create`, `mounts.list`, `mounts.remove`), which
makes it scriptable through the CLI passthrough on day one.

Pre-flight register — things the deep dive found that this design must fix
or route around, none of them speculative:

1. **The volume refresh loop evicts anything OS detection doesn't
   re-report.** `refresh_volumes_internal`'s removal pass has no exemption
   for backend-registered volumes; cloud volumes are dropped from the
   in-memory map on the next refresh cycle, after which cloud indexing
   hard-errors. Mount-created volumes need the same exemption cloud needs.
2. **Self-visibility and self-indexing.** `fuse`/`fusectl` are classified
   as virtual filesystems and filtered from detection by default — a mount
   we create is invisible to our own volume manager — while the opposite
   hazard also exists: the whole-volume walk must exclude our own mount
   points or the index indexes a projection of itself.
3. **`backend_for_volume` falls back to `LocalBackend` when a volume's
   backend is missing** — for a cloud or virtual volume that means local
   I/O against a literal `s3://bucket` path. The fallback must be
   provider-aware.
4. **Cloud sources are half-wired.** `volumes.index` registers a cloud
   source rooted at the URI string, but ephemeral queries bail on
   non-`Physical` paths, so cloud browse reads a different store than
   cloud indexing writes. Mounting a cloud volume requires the arena to
   accept URI-rooted sources end to end.
5. **Cloud fingerprints ignore endpoint and account** —
   `(service, bucket)` collides across providers with the same bucket
   name. Identity needs the endpoint in the hash before mounts multiply
   how often cloud fingerprints travel.
6. **Locations don't know their sources.** Mounting a location waits on
   contract 1's subtree-with-policy shape; the interim answer is that only
   volumes and sources are mountable until locations migrate.
7. **No `Range` on any HTTP surface, no auth on the sidecar route.** The
   validation endpoint adds both; the peer ALPN sidesteps HTTP entirely.

## Ordered phases

Phases 1, 2 and 4 landed on 2026-08-19 in a different order than written;
what follows is the revised remainder. The numbering restarts so it can be
read forward.

1. **Byte plane and block cache.** Extract `ByteProvider` out of the WebDAV
   handler, then put L1/L2 caching and read-ahead behind it. Detailed in
   `docs/plans/2026-08-20-byte-plane-and-block-cache.md`. This is under every
   frontend and testable through the mount that already exists, which is why
   it goes before one. Exit: reading the same range twice from a peer source
   issues one network fetch, and a working session can be characterised
   through `mounts.cache_status`.
2. **SMB loopback frontend.** An SMB2 server over the same resolver, bound to
   127.0.0.1 on an OS-picked port, mounted by the host client. Replaces the
   WebDAV surface as the working mount and gives the honest no-install
   baseline the native module must beat — a baseline that only means anything
   once phase 1 exists. Exit: a source browses and streams through the OS SMB
   client on macOS, Windows and Linux, with `ls -R`, Finder browse, preview
   and NLE scrub measured on a 2M-entry source.
3. **Drop takeover.** Intercept the file manager's drag onto a mount and
   hand it to the daemon as an import job instead of letting the OS run a
   recursive copy through a streaming filesystem. Shipping streaming
   filesystems all intercept this — a shell extension inside `explorer.exe`,
   drag hints on macOS — because the naive path behaves badly enough to be a
   product problem. We
   have a job system already; a drop should become a job. Sits here because
   it is the first thing a real user does to a mounted drive, and because
   the native frontend in phase 4 is where the platform hooks live. Exit:
   dragging a folder onto a mounted source returns immediately and the
   import runs as a tracked job that survives the window closing.
4. **Native mount module, macOS first.** FSKit frontend over the same
   metadata and byte planes, owning read-ahead and seek policy. Exit: a
   peer source browses and streams as a local volume with zero resident
   bytes, and beats the SMB baseline on time-to-first-frame and scrub.
   WinFsp and FUSE follow behind the same interface.
5. **Pinning.** The same cache with an eviction exemption: pin a file or
   subtree, fetch its blocks eagerly, never evict them, and report pinned
   bytes against capacity. Exit: a pinned subtree opens with the network
   down. (Size caps and eviction land in phase 1; this adds only the
   exemption and the surface for it.)
6. **Renditions.** Prepared reduced-quality media as a rebuildable
   artifact class, produced by a job on whichever device has the cycles,
   served by the mount with fallback to the original. Exit: a
   multi-gigabyte source scrubs at proxy cost over a WAN link.
7. **Writes** — its own design, gated on the drain/sequencing contracts.

**Deliberately not in the plan.** The signed metadata ledger, the
credential broker, the edge cache and the separate change channels. See
"Why the trust layer is not in this document". Multi-writer convergence
stays behind the write fence: twin-fork merges, delete resurrection and
trash migrations that cannot complete until every client upgrades are the
standing argument for leaving it there.

## Implementation status (2026-08-19)

Built and verified live the same day this design was written. The ordering
held; the boundaries between phases did not — the validation mount grew into
the serving surface, and peer mounts landed immediately behind it.

**What exists:**

- **The WebDAV share** (`core/src/service/mounts/`): read-only, loopback,
  port 7764 (`SD_MOUNT_HTTP_PORT`). One share exposes every registered
  source as a top-level collection. `PROPFIND` serves from the arena with
  sizes, mtimes, and extension-derived content types; `GET`/`HEAD` support
  single ranges. Verified against the macOS kernel WebDAV client: mounted,
  browsed, md5-identical reads, byte-exact 206 responses. Detached sources
  browse from snapshots; reads return 503 naming the source. Byte routing
  per target: local passthrough, the volume backend in bounded chunks for
  URI roots (cloud), the peer protocol for remote shares.
- **The byterange protocol**
  (`core/src/service/network/protocol/byterange.rs`, ALPN
  `spacedrive/byterange/1`): `Stat`, `Read {offset, len}` (capped 8 MiB per
  request), `ListSources`, `FetchSnapshot`. Framing is
  `[u32 BE length][rmp_serde message]`, with `Read`/`FetchSnapshot`
  responses followed by exactly `len` raw bytes on the same stream. One
  request per bi-stream; cancellation is dropping the stream. Served only
  to paired devices; reads only under attached registered sources.
- **Peer replication** (`core/src/service/mounts/peer.rs`): a peer's
  snapshot is fetched (saved fresh from its live arena first), stored under
  `<data>/mounts-remote/<device>/`, and loaded into a local read-only
  arena. Replication runs automatically on peer connection (with a startup
  sweep for connections that beat the event subscription, and a 30s
  per-device debounce), or explicitly via `mounts.sync_peers`. Remote
  shares appear as `<name>@<device>-<id>`; the peer going offline degrades
  to exactly the detached-drive ghost. Verified across two paired daemons:
  auto-replication on connect, LAN range reads byte-exact, full-file md5
  match through the whole chain, browse-after-kill.
- **Ops**: `mounts.status` (share URLs, per-source, with owning device) and
  `mounts.sync_peers`, both registered normally — and reachable through the
  new generic CLI passthrough, `sd op <name> [--json … --library …]`, which
  resolves any registered op (`query:<name>`, then `action:<name>.input`).
- **Fixed en route**: `CoreContext` now builds the ephemeral cache from its
  own data dir instead of the default installation's (previously
  `--data-dir`/`--instance` daemons read and wrote the production source
  registry); share-name suffixes use the uuid tail (v7 leading characters
  are the mint timestamp and collide within an instant).

**Findings for the register**, discovered by the two-daemon test and since
cleared: a failed indexer job left the slot's `indexing_in_progress` set, so
subsequent browses of that path never re-dispatched until restart; and the
CLI did not append `instances/<name>` to `--data-dir` the way the daemon
does, so instance-scoped CLI calls built paths under the wrong device
identity ("Location root path is not local").

## Implementation status (2026-08-21)

Phases 1 and 2 landed. `core/src/service/mounts/` is now `provider.rs` (the
`ByteProvider` trait plus local, cloud and peer implementations), `cache.rs`
(L1/L2 block cache, read-ahead, eviction), `smb.rs`, `trace.rs`, and a
`webdav.rs` reduced to its HTTP half.

- **Byte plane and block cache**, detailed in
  `docs/plans/2026-08-20-byte-plane-and-block-cache.md`. Repeat reads and
  backward scrubs over peer and cloud sources no longer cross the network.
  Read-ahead is constructed per response and aborts when the reader goes
  away. `mounts.cache_status` reports hit rate and bytes by tier. The size
  cap is `AppConfig.mounts.cache_max_bytes` and applies without a restart.
- **SMB loopback frontend** (`smb.rs`): an SMB2 server bound to 127.0.0.1
  over the same resolver and arenas the WebDAV share reads, held read-only
  by `BackendCapabilities::is_read_only`. `mounts.status` carries the mount
  URL and hint.

The measurement phase 1 exists to enable has not been run. Experiments 2 and
5 are still the thing that decides whether SMB is sufficient or FSKit is
required.

**Snapshot refresh by generation** landed 2026-08-22. `RemoteSourceInfo`
carries the snapshot's `source_version(size, mtime)` plus a `dirty` flag
for arena changes the owner has not saved. A replica records the
generation it was built from, so a listing where nothing moved costs one
request and no transfer, and a paced refresh keeps a peer mount current
while both machines stay connected rather than only at reconnect. Polling
is adequate precisely because the comparison is cheap; a push protocol
would still need this comparison underneath it to survive a missed
message.

Its limit: the arena goes fresh, but Finder may not repaint until its own
directory cache expires. SMB2 CHANGE_NOTIFY is the standard way to tell a
client to re-enumerate, and the server crate answers NOT_SUPPORTED —
implementing it needs async-command support (held-open requests,
out-of-band responses, CANCEL) that the crate's sequential dispatcher does
not have. Worth measuring the staleness before forking a crate over it.

**Not built yet**: drop takeover (phase 3), the native FSKit module
(phase 4), pinning (phase 5), renditions (phase 6), writes (phase 7).

## Implementation status (2026-09-18)

**Device facts.** `DeviceFacts` is a byterange request answered with what
the serving device publishes about itself: the hardware its startup
detection recorded, and every mounted, user-visible volume under the id its
sources anchor to. A peer fetches it on every sync pass beside
`DeviceSummary`, stamps it with its own receipt time, and keeps it in the
device's replica manifest, so an offline device still describes itself as it
last did. `volumes.list` lists a paired device's volumes from those facts,
mounted only while the owner is connected, and `devices.list` fills in a
paired device's hardware from them. The overview folds each replica into its
owner's volume bar by `volume_uuid`, the same way a local source folds into
its own.

Only the owner writes its facts, so nothing merges and library sync is not
involved. Volume classifications travel as names, so one a peer's build does
not know reads as unknown instead of failing the response. The facts cross
library boundaries the way replicas and device summaries already do; scoping
them belongs to library membership. An owner that cannot answer the request
leaves its card listing replicas as rows.

## Implementation status (2026-09-19)

**Database generations.** A nested source travels as its own database, and
its generation is its store's revision (`crates/store/src/revision.rs`): a
store id and a count that triggers advance on every committed change to a
tracked row. Rewrites that change nothing and bookkeeping columns leave it
where it was. File size and mtime cannot stand in for it, because SQLite
deletes the WAL when a pool's last connection closes and recreates it on the
next read, so the files move while no row does. A refresh still transfers the
whole database; `docs/plans/2026-09-19-incremental-replication.md` replaces
that with the changes since the replica's revision.

## Implementation status (2026-09-24)

**The SMB share mounts itself.** `service/mounts/attach.rs` mounts the
loopback SMB share at `<data-dir>/mount` through `NetFSMountURLSync` the first
time something needs a path to another device's file, read-only, soft and
hidden from Finder, and unmounts it at shutdown. A daemon that exits without
shutting down leaves a mount with no server behind it, so startup unmounts
whatever is at the mount point, reading the mount table with `MNT_NOWAIT`
rather than asking the dead mount. Volume detection skips mounts inside the
data dir. macOS only; other platforms answer that it is not available yet.

**One resolver in both directions.** `share_path` turns an `SdPath` into its
share-relative path, the inverse of `resolve_target`, and two core queries
render it: `files.stream_url` as the HTTP URL Spacedrive's viewers stream
from, and `files.local_path` as the path inside the mount that another app,
Quick Look included, opens. `docs/plans/2026-09-21-opening-remote-files.md`
has the clients.

**The SMB frontend served the wrong file after a listing.** Every entry
reported file id 0, and `list_dir` ignored its search pattern, although the
server crate leaves filtering to the backend. macOS looks one name up by
searching for it and takes the first entry back as that name's, so it gave
one file another's size and, once the ids differed, another's bytes. Entries
now carry a hash of their share path as their id, and a pattern answers with
the entries it names.

**The HTTP share answers only loopback hosts.** A request whose Host header
names anything else is refused, which stops a web page that points its own
domain at 127.0.0.1 from reading the share, and CORS headers go only to Tauri
web views and pages served from loopback.

## Open questions

- One OS volume per mount target, or one Spacedrive volume with targets as
  top-level directories? Per-target matches user intuition (a drive per
  drive) and keeps detach semantics crisp; single-volume centralizes the
  namespace. Leaning per-target.
- Whether the WebDAV endpoint survives as a permanent interop surface for
  clients we don't ship, once SMB is the working mount.
- FSKit coverage under pro-app I/O patterns — mmap, `F_NOCACHE`, atomic
  saves — needs empirical answers early in phase 3.
- Block size and read-ahead policy per provider class (LAN peer vs cloud
  vs WAN peer have different sweet spots); phase 1 exists to measure this.
- **The extension update tax.** A system-extension mount cannot be updated
  without user friction. The known answer is a root helper that stages and
  commits app bundles silently, which is a permanent privileged surface
  bought to avoid a restart prompt, and it is not a trade this project
  should make at its release cadence. Working assumption: accept
  the prompt. Revisit only if update friction becomes a real complaint.
- Whether remote-source snapshot replication is pull-on-mount only or the
  owning device also pushes generation bumps to interested peers while
  connected.
