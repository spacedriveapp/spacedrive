# Byte plane and block cache

> **Status:** Plan — ready to execute
> **Captured:** 2026-08-20
> **Companions:** `docs/core/design/mounts.md` (the design this executes a
> phase of), `docs/plans/2026-08-18-storage-consolidation.md` (artifact
> classes and the stores a mount reads)

## Why this first

Mounts v0 works and re-fetches everything. Open the same file twice through
a peer mount and both reads cross the network; scrub backwards over bytes
that arrived a second ago and they are fetched again. That is the difference
between a demo and a drive.

It is also the layer under every frontend. WebDAV today, SMB and FSKit
later — they all sit on the same byte plane, so nothing built here is thrown
away when the frontend question resolves. And it is testable through the
mount that already exists, with no new plumbing.

The design doc previously ordered the SMB frontend ahead of the cache, on
the argument that a loopback mount gives an honest baseline for the native
module to beat. A baseline measured with no cache measures re-fetching, not
architecture. The phases in `mounts.md` are flipped to match this plan.

## What exists today

Byte routing lives inside the WebDAV handler. `core/src/service/mounts/webdav.rs`
carries three functions that each resolve bytes and build an HTTP response:

- `get_local` — `tokio::fs`, 256 KiB chunks
- `get_via_backend` — `VolumeBackend::read_range` for URI-rooted (cloud)
  sources, 4 MiB chunks, with the source-root prefix stripped to a bucket key
- `get_remote` — `peer::stat` and `peer::read_range` over the byterange ALPN,
  4 MiB chunks

`get` dispatches between them on `Target` plus a `contains("://")` test for
cloud roots. Each function re-implements range parsing, the `206`/`200`
decision, header construction, and its own error-to-status mapping. There is
no `ByteProvider` — the trait in the design doc was never built, and a second
frontend would have to duplicate all of it.

## The shape

One trait, four implementations, one resolver, one cache layer.

```rust
pub struct ByteTarget {
    pub source_id: Uuid,
    /// Absolute within the source's namespace — a real path for local and
    /// peer sources, a URI for cloud roots.
    pub path: PathBuf,
}

pub struct ByteStat {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[async_trait]
pub trait ByteProvider: Send + Sync {
    async fn stat(&self, target: &ByteTarget) -> Result<ByteStat, ByteError>;
    async fn read_range(&self, target: &ByteTarget, range: Range<u64>)
        -> Result<Bytes, ByteError>;
    /// Drives cache and read-ahead policy; a LAN peer, a cloud volume and a
    /// local disk want different block sizes and prefetch depths.
    fn class(&self) -> ProviderClass;
}

pub enum ProviderClass { Local, Cloud, Peer, Replica }
```

`ByteError` carries what the frontends need to answer with, so the mapping
from error to HTTP status (or to an `errno` later) exists once:

```rust
pub enum ByteError {
    NotFound,
    Detached { source: PathBuf },       // 503, names the drive
    PeerUnavailable { device: String }, // 503, names the machine
    NoBackend,                          // 503, cloud credentials not restored
    Io(anyhow::Error),
}
```

Implementations are the three existing functions with their HTTP halves cut
off: `LocalProvider`, `CloudProvider` (keeps the URI-root key stripping),
`PeerProvider`. `Replica` is declared and unimplemented — detached sources
return `Detached` until the content map can resolve a stand-in.

The resolver replaces the dispatch inside `get`:

```rust
async fn resolve_provider(context: &Arc<CoreContext>, rel: &str)
    -> Result<(Arc<dyn ByteProvider>, ByteTarget), ByteError>;
```

It owns the vocabulary translation — share name to source, cloud detection,
detached checks — and it is the only place a frontend needs to call.

**The cache is a decorator, and the resolver decides who gets one.**
`CachedProvider` wraps any inner provider. Local sources are not wrapped:
the bytes are already on a local disk and a second copy is waste. Cloud and
peer providers always are.

## Cache design

**Keying.** The arena's `EntryMetadata` carries path, size and mtime and no
content identity, so blocks key on what we actually have:

```
(source_id, path, source_version, block_index)
```

`source_version` is `pvcache::source_version(size, mtime)`, which already
exists and is the same validator the thumbnail cache uses. A file that
changes gets a new version and its old blocks become unreachable and
evictable — invalidation is a consequence of the key, not a separate
mechanism.

Content-addressed keying, which would let two sources holding the same bytes
share cached blocks, is deliberately not in this phase. It requires content
identity to be available at mount time and the arena does not carry it. When
that changes it is a key swap behind the same interface.

**Tiers.** L1 in memory, byte-capped LRU. L2 on disk under
`<data_dir>/mounts-cache/`, sharded two levels by a hash of the file key,
one file per block. L3 is the provider. Logs name the tier that answered so
the numbers are readable.

**Blocks.** Fixed size per provider class, 1 MiB default. A read of
`[start, end]` touches every block covering it; missing blocks are coalesced
into as few provider calls as possible, capped by the provider's own limit —
the peer protocol refuses a `Read` over 8 MiB per request.

**Eviction.** LRU over L2 by last access, against a configurable byte cap.
The cache is a rebuildable artifact class per the consolidation doc: safe to
delete at any moment, never authoritative, and rebuilt by being read again.
On startup the ledger is rebuilt by walking the directory, which is also how
a partially deleted cache heals.

**Limits and control.** Size cap is a config setting, not an environment
variable — we have a settings system and it should be reachable from the UI.
`mounts.cache_clear` drops L1 only or both tiers.

## Read-ahead

Policy lives with the cache and is expressed per provider class:

```rust
pub struct ReadAheadPolicy {
    pub block_bytes: u64,
    pub start_depth: u32,      // blocks fetched ahead on a cold sequential read
    pub seek_depth: u32,       // blocks fetched ahead when a seek is detected
    pub max_inflight: u32,     // concurrent prefetches
    pub max_bytes: u64,        // ceiling on speculative bytes per handle
}
```

WebDAV cannot pass seek hints, so v1 infers: a request whose start equals the
previous request's end is sequential and arms `start_depth`; a discontinuity
arms `seek_depth`. That inference is enough to measure with and is replaced,
not extended, when a native frontend can tell us directly. This is the half
of the streaming experience a loopback mount cannot give us and the reason
the frontend ordering in `mounts.md` is what it is.

Prefetches are best-effort and cancellable: dropping the response stream
drops the in-flight fetches with it.

## What changes in the WebDAV handler

`webdav.rs` loses its byte-fetching half and keeps its HTTP half. `get`
becomes: resolve the provider, `stat`, parse the range, build the response
headers, and stream from `provider.read_range` in block-aligned chunks.
`parse_range`, `content_type_for` and the `206`/`200` construction are shared
by one code path instead of three. The directory check and traversal refusal
stay where they are.

Net effect on behaviour today: repeat reads and backward scrubs over cloud
and peer sources stop crossing the network, and the three duplicated
range/response implementations collapse into one.

## Observability

Experiments 2 and 5 in the mounts design need numbers we currently cannot
produce. Extend the ops surface with `mounts.cache_status`: per source and in
total, L1 and L2 bytes and block counts, hit and miss counts by tier, bytes
fetched from each provider class, and current in-flight prefetches. It is a
query like any other, so it is reachable from `sd op` on day one and can back
a UI panel later.

## Ordered phases

1. **Extract the byte plane.** `ByteProvider`, `ByteError`, the three
   implementations, the resolver; `webdav.rs` rewritten on top of them with
   one range/response path. No cache yet. Exit: the two-daemon test from
   mounts v0 passes unchanged — byte-exact reads, full-file md5 match,
   detached and offline sources degrading with the same messages.
2. **Block cache.** L1 and L2, keying, eviction, the size cap and
   `mounts.cache_clear`. Wrapping decided by the resolver. Exit: reading the
   same range twice from a peer source issues one network fetch; the cache
   survives a daemon restart; deleting the cache directory mid-session does
   not fault the mount.
3. **Read-ahead.** The policy struct, class defaults, sequential and seek
   inference, inflight caps. Exit: a linear read of a multi-GB file from a
   LAN peer sustains without stalls between blocks, and a backward scrub over
   cached ranges issues no fetches.
4. **`mounts.cache_status`.** Exit: a working session over a peer source can
   be characterised — hit rate, bytes by tier, bytes off the wire — without
   attaching a debugger.

Then measurement, which is the point of all of it: run experiments 2 and 5
against the existing WebDAV mount and get real numbers for time-to-first-frame
and for what a real editing session costs in cache bytes. Those numbers decide
whether SMB is sufficient or FSKit is required, which is currently an argument
rather than a measurement.

## Implementation status (2026-08-20)

All four phases landed, with two changes to what was written above.

**Coalescing arrived with read-ahead, not with the cache.** The first cut of
phase 2 fetched missing blocks one at a time inside a single `read_range`,
so a 4 MiB request over cold blocks cost four serialised round trips. Runs
of consecutive misses now become one provider call each, capped by the
inner provider's `max_read`, and separate runs are fetched concurrently.

**Read-ahead is per response, and cancels with it.** `CachedProvider` is
constructed once per request and lives for the whole response body, which
makes it the handle the policy needed. Depth doubles from `start_depth`
while a reader stays sequential — detected by the next range starting where
the last one ended — and drops back on a discontinuity. Speculative fetches
take a permit from a process-wide semaphore so a guess can never queue
ahead of a reader waiting on bytes, spend against a per-response byte
budget, and are aborted in `Drop` when the reader goes away.

Also landed: `peer::stat` now returns the mtime the byterange protocol was
already sending and discarding, which the cache key needs; and
`mounts.cache_status` came early because phase 2's exit criterion is not
checkable without it.

**The size cap is a setting** (2026-08-21). `AppConfig.mounts.cache_max_bytes`,
read at startup and reachable through `config.app.get` and
`config.app.update`. Unlike the service toggles it applies immediately —
`BlockCache::set_max_bytes` evicts down to a lowered limit on the spot,
because nothing in the cache is authoritative and there is no reason to
make someone restart to reclaim disk.

Two register items were cleared alongside it:

- **A failed index no longer looks like a successful one.** The job already
  cleared `indexing_in_progress` on failure, but did it by calling
  `mark_indexing_complete`, which also recorded the path as *indexed* — so
  a partial arena was served as complete and nothing re-dispatched. Split
  into `mark_indexing_complete` and `mark_indexing_failed`, with the
  failure path clearing both flags. The dispatch-failure branch in
  `files.directory_listing` had the same defect.
- **`--instance` now resolves the same directory everywhere.** The rule
  (`<base>/instances/<name>`) lived in three binaries and was missing from
  the CLI, so instance-scoped CLI calls read the default installation's
  device identity while talking to the instance's socket. It now sits in
  `infra::daemon::addr` next to `daemon_socket_addr` — the module that
  exists precisely so every side agrees — and all four callers use it.
  `apps/server` was also re-deriving the port hash by hand; it uses the
  shared function now.

## Fences

- **Reads only.** Nothing here touches the write path, and the cache is never
  a write buffer.
- **No new authoritative store.** The cache is rebuildable and deletable. If
  something wants to be the source of truth, it is not this.
- **No pinning yet.** Pinning is the same cache with an eviction exemption
  and it is a later phase; do not build half of it here.
- **No content-addressed sharing yet.** Gated on content identity reaching
  the arena.
- **No new frontend.** This phase ships through the WebDAV mount that
  already exists.

## Also fix, while in here

The failed-indexer bug on the register: a failed indexing job leaves the
slot's `indexing_in_progress` set, so subsequent browses of that path never
re-dispatch until the daemon restarts. It is small, it is unrelated to this
work, and it will interrupt every measurement session until it is fixed.

## Open questions

- One file per block on L2, or one sparse file per (file, version) with a
  present-block bitmap? Per-block is simpler to evict and heal; sparse is
  kinder to the filesystem at scale. Leaning per-block until a measurement
  says otherwise.
- Block size per provider class: 1 MiB is a guess for all three. Phase 4
  exists to replace the guess.
- Whether L1 is worth having at all once L2 exists on an SSD, or whether the
  page cache already does that job well enough.
- Whether cloud sources should share a cache namespace with the peer that
  owns the credentials, since the same bytes can be reached both ways.
