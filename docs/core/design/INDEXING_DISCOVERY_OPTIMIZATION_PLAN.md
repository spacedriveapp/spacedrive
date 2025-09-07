### Indexing Discovery Throughput Plan

Author: Core Team
Status: Draft
Last updated: 2025-08-08

---

#### Objective

Increase discovery throughput (NVMe-first) and preserve scalability on large trees by:

- Parallelizing directory traversal
- Reducing per-entry filesystem syscalls
- Bulk inserting batches to the database
- Measuring FS vs DB vs compute costs to target bottlenecks

Scope: Discovery and Processing phases on SQLite. Aggregation already updated to avoid SQLite bind limits.

---

#### Current baseline (NVMe, discovery-only)

Measured on M2 MacBook Pro (16GB, macOS 14.5). Datasets reside on NVMe; dataset names like "hdd\_\*" indicate shape only.

- nvme_small: ~641 files/s (300 files; small sample)
- nvme_mixed: ~575 files/s (dirs/sec ~286; files line missed by parser in one run)
- hdd_medium (NVMe medium-shape): ~543 files/s
- hdd_large (NVMe large-shape): ~350 files/s

Note: Indexer already supports persist-off to isolate FS traversal.

---

#### Measurement plan

Add metrics and emit to job logs and JSON summary:

- discovery.rs
  - fs_read_dir_ms (sum), fs_metadata_ms (sum)
  - dirs_seen, files_seen, entries_per_dir histogram
  - discovery_concurrency in config (for correlation)
  - entries_channel_backpressure_events (count)
- processing.rs
  - db_tx_ms (sum), db_tx_count, db_rows (sum)
  - avg rows/tx, rows/s
- aggregation.rs
  - agg_select_ms (sum), agg_dirs

Scenarios to isolate bottlenecks:

- FS-only ceiling: persist=off, metadata=Fast, concurrency ∈ {1,4,8,16}
- DB write cost: persist=on vs off with metadata=Fast
- Metadata cost: metadata=Full vs Deferred (same persist setting)

---

#### Design changes

1. Parallel discovery traversal (worker pool)

- Implement worker pool: N async workers consume a channel of directory paths and process read_dir + lightweight type checks; push child dirs back to the channel.
- Backpressure: bounded channel and bounded `dirs_in_flight` to cap memory growth.
- Config (new):

  - `discovery_concurrency: usize` (default 8)
  - `dirs_channel_capacity: usize` (default 4096)
  - `entries_channel_capacity: usize` (default 16384)
  - Cancellation: share `Arc<AtomicBool>` with workers; call `ctx.check_interrupt()` frequently
  - Progress updates: throttle to fixed intervals (e.g., every 250ms) to avoid log overhead

  Implementation sketch in `src/operations/indexing/phases/discovery.rs`:

  - Replace the sequential loop with:
    - `mpsc::channel<PathBuf>(dirs_channel_capacity)` seeded with root
    - spawn `discovery_concurrency` workers → each `read_dir` + classify → send subdirs back; send `DirEntry` to `entries_tx`
    - batching task drains `entries_rx`, appends to `pending_entries`, flushes on `should_create_batch()` or time-based flush
  - Respect `should_skip_path` and `seen_paths` as today

2. Deferred metadata mode

- Config (new): `metadata_mode: enum { Full, Fast, Deferred }`

  - Fast: rely on `DirEntry::file_type()` and names; avoid metadata for files where feasible
  - Deferred (default for discovery-only): skip file size/mtime in discovery; compute later in processing in bulk

  Implementation notes:

  - Discovery fills `DirEntry { kind, name, parent, inode? }` without size/mtime when Deferred
  - In `processing.rs`, before inserts, batch-stat files (chunked; optional `spawn_blocking` pool) and populate size/mtime

3. Bulk DB inserts (processing phase)

- Accumulate ActiveModels per batch and use `Entity::insert_many` for `entry`, `directory_paths`, and closure rows.
- Single transaction per batch; configurable `batch_size` (default 2000).
- PRAGMAs at DB open: WAL, `synchronous=NORMAL`, `temp_store=MEMORY`, reasonable negative `cache_size`, set `mmap_size`.

  Implementation notes:

  - Prefer `insert_many` over per-row inserts in `processing.rs`/`entry.rs`
  - Keep one `BEGIN…COMMIT` per batch; measure `db_tx_ms`, `db_rows`, `db_tx_count`

4. Log parser resilience (sd-bench)

- Broaden regex to capture "Files:" lines consistently and attach FS/DB timing to JSON to avoid relying on text.

5. Safety improvements already merged

- Chunk large IN queries (~900 chunk) to avoid SQLite "too many SQL variables" across:
  - `indexing/phases/aggregation.rs`
  - `indexing/persistence.rs`
  - `indexing/path_resolver.rs`
  - `indexing/hierarchy.rs`
  - `operations/addressing.rs`

---

#### Implementation outline

Phase 1: Metrics + JSON + parser

- Add per-phase timers/counters; export to job logs and `--out_json` summary
  - New JSON fields: `{ fs_read_dir_ms, fs_metadata_ms, dirs_seen, files_seen, db_tx_ms, db_tx_count, db_rows, agg_select_ms }`

Phase 2: Discovery concurrency + Deferred metadata

- Replace sequential loop with worker pool + bounded channel
- Introduce `metadata_mode`; by default use Deferred for discovery-only

Phase 3: Bulk inserts

- Switch to `insert_many` for batch persistence; keep single-transaction batches

Phase 4: Tuning + docs

- Run matrix (persist on/off, metadata modes, concurrency sweep)
- Publish medians; update whitepaper with measured NVMe tiny headline and mixed numbers

---

#### Code touchpoints

- `src/operations/indexing/phases/discovery.rs`: replace traversal with worker pool; add metrics; support `metadata_mode`
- `src/operations/indexing/phases/processing.rs`: deferred metadata batcher; `insert_many` bulk inserts; db metrics
- `src/infrastructure/database/` (open/create): apply SQLite PRAGMAs once
- `src/operations/indexing/metrics.rs` (or new): define metrics structs and helpers
- `benchmarks/src/main.rs`: extend `--out_json` to include FS/DB/agg timing; relax Files regex

---

#### Optional: jwalk backend (A/B)

Add an optional traversal backend using `jwalk` (rayon-based) to parallelize `readdir`/metadata:

- Adapter spawns a bounded producer that walks with `jwalk::WalkDir` and sends `DirEntry` over a channel
- Respect `should_skip_path`, cancellation flag, and channel backpressure
- Config: `fs_traversal_backend: enum { AsyncPool (default), Jwalk }`
- Bench both backends on NVMe tiny/mixed to choose defaults per platform

---

#### Expected outcomes

- Parallel discovery (8–16 workers): 2–4× improvement for tiny files on NVMe
- Deferred metadata: ~50–70% fewer metadata syscalls during discovery for mixed trees
- Bulk inserts: 2–5× improvement in DB rows/s during processing

---

#### Notes

- Persist-off already supported; use it for FS ceiling tests
- Datasets may include sparse files/hard links; logical size can exceed physical on-disk usage
