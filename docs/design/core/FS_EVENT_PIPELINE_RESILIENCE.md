## FS Event Pipeline Resilience and Correctness (Large Bursts)

### Goals
- 100% correctness for large/bursty FS changes (e.g., git clone, massive moves).
- No synthetic IDs; emit canonical `Event::Entry*` only after DB success.
- Deterministic ordering where needed; avoid races and partial state.
- Scale to millions of path changes without O(N) per-child work in DB.

### Current State (as of this PR)
- Watcher normalizes to `Event::FsRawChange { library_id, kind: FsRawEventKind }`.
- `LocationWatcher` spawns `responder::apply(...)` per raw event.
- `notify` callback uses `try_send` on a bounded mpsc (default 1000).
- Platform handlers:
  - Linux often emits a single directory rename for subtree moves (good).
  - macOS/Windows may emit floods of per-path changes.

Risks:
- Event dropping (bounded `try_send`).
- Loss of ordering and interleaving (per-event `tokio::spawn`).
- Duplicate/conflicting child events when a directory move should be a single atomic op.

### Requirements
- R1: No event loss under steady-state; controlled backpressure under extreme bursts.
- R2: Single-source-of-truth: DB reflects final FS state after each applied operation.
- R3: Parent-first application: directory structural changes precede children.
- R4: Idempotency and deduplication within a batch window.
- R5: Atomic structural updates (transactions), bulk path updates for subtrees.

### Proposed Architecture

1) Per-Location Worker and Queue
- Replace per-event `spawn` with a single worker task per watched location (or per location root entry).
- Internal queue: `mpsc::channel(capacity)` with awaited `send` (backpressure) rather than `try_send`.
- Channel ordering preserves intake order; worker ensures serialized application.

2) Short Batching Window + Coalescing (100–250ms)
- Worker aggregates events during a small debounce window into a `Vec<FsRawEventKind>`.
- Deduplicate by path and coalesce patterns:
  - Create+Remove within window → drop (neutralized temp files).
  - Modify after Remove → ignore.
  - Multiple Modify → collapse to one.
  - For Rename chains A→B, B→C → collapse to A→C.
- Directory Rename Collapser:
  - If a rename of a directory `D → D'` is present, suppress child Create/Remove/Rename events under `D/` and `D'/` in that batch. The subtree move will be handled atomically.

3) Parent-First Application Strategy
- Always detect and apply highest-ancestor directory moves first.
- Use `EntryProcessor::move_entry(...)` for the directory:
  - Updates `parent_id` and directory row.
  - Reconnects closure table for entire subtree in a single transaction.
  - After commit, run `PathResolver::update_descendant_paths` (bulk REPLACE) to fix child paths.
- Then apply remaining file-level creates/modifies/deletes.

4) Change Resolution
- For each coalesced item:
  - Resolve directory paths via `directory_paths.path == path`.
  - Resolve files by parent directory path and `entries.name` (+ `extension`).
  - Use `ChangeDetector` where comparing FS vs DB state is beneficial (e.g., for modifies and ambiguous cases).

5) Backpressure and Flow Control
- Awaited `send` to per-location queues; configurable capacity.
- Metrics: queue depth, batch size, coalescing hit rates, latency.
- Fallback strategies when queue is full for extended durations (e.g., trigger a focused re-index of affected subtree).

6) Idempotency & Exactly-Once Semantics
- Within a batch, dedupe events by final intent (see coalescing rules).
- Across batches, rely on DB constraints and idempotent `EntryProcessor` operations.
- No reliance on synthetic IDs; correctness flows from path resolution + DB.

### Data Flow (Revised)

notify → Watcher (per-platform) → `WatcherEvent` → Per-Location Queue (await send)
→ Worker (debounce window) → Coalesce & Dedup → Parent-first Apply via Indexer Responder → Emit canonical `Event::Entry*` with real IDs

### Pseudocode

```rust
// watcher/mod.rs (event loop)
let location_id = map_path_to_location(&watched_locations, &event);
let tx = ensure_worker_for_location(location_id); // creates if missing
tx.send(event).await?; // awaited (backpressure), not try_send

// worker task per location
loop {
    let first = rx.recv().await?;
    let mut batch = vec![first];
    let deadline = Instant::now() + debounce_window;
    while let Ok(ev) = rx.try_recv() {
        batch.push(ev);
        if Instant::now() >= deadline { break; }
    }
    let coalesced = coalesce(batch); // dedupe, fold rename chains, suppress subtree noise
    let ordered = parent_first(coalesced); // directory moves before children
    apply_with_indexer(context, library_id, ordered).await?; // transactional operations
}
```

### Coalescing Rules (Examples)
- Create(X), Remove(X) → ∅
- Modify(X) × N → Modify(X)
- Rename(A→B), Rename(B→C) → Rename(A→C)
- Rename(Dir D→D'), then any events under D/* or D'/* within window → suppressed

### DB Operations
- Create: `EntryProcessor::create_entry` (bulk closure insert outside batching in responder or re-use `create_entry_in_conn` when grouping many creates).
- Modify: `EntryProcessor::update_entry`.
- Move (dir or file): `EntryProcessor::move_entry` (transaction + closure reconnection) and `PathResolver::update_descendant_paths` for directories.
- Delete: subtree deletion with closure cleanup and `directory_paths` removal (as in processing phase delete path).

### Ordering Guarantees
- Per-location FIFO at queue.
- Parent-first ordering enforced in worker.
- Cross-location operations can remain parallel.

### Crash Safety
- Structural changes are transactional; descendant path updates can be retried on boot if interrupted (record last move op in a small table or log and reconcile on start).
- On overflow/backpressure alerts, enqueue a focused re-index job for the affected subtree as a safety net.

### Tuning Knobs
- `debounce_window_ms` (default 150ms).
- `queue_capacity` per location (default 10k; adjust via config/env).
- `max_batch_size` (to bound memory and latency).

### Metrics & Observability
- Per-location queue depth, enqueue latency, batch sizes.
- Coalescing rates: suppressed children, rename chain collapses.
- DB op timings and retry counts.

### Test Plan
- Simulate: git clone (tens of thousands of creates), large directory rename (deep trees), massive deletions.
- Platform parity tests for macOS/Windows/Linux.
- Fault injection: kill during move, verify DB consistency on restart.

### Incremental Implementation Plan
1. Introduce per-location workers and awaited send (remove `try_send`, remove per-event `spawn`).
2. Add debounce window and minimal coalescing (dedupe modifies, neutralize create/remove).
3. Implement directory-rename collapser and parent-first ordering.
4. Wire responder `apply(...)` to process batches (signature change to accept `Vec<FsRawEventKind>`), reuse `EntryProcessor` paths.
5. Add metrics and configuration.
6. Add focused re-index fallback for overflow conditions.

### Code Touch Points
- `core/src/service/watcher/mod.rs`: per-location workers, awaited send, batching.
- `core/src/service/watcher/platform/*`: unchanged aside from event normalization already done.
- `core/src/ops/indexing/responder.rs`: change `apply` to accept batches; implement resolution and DB ops + final event emission.
- `core/src/ops/indexing/entry.rs` and `path_resolver.rs`: leveraged as-is.

---
This design converts flood-y per-file events into a small number of deterministic, parent-first DB operations, ensuring correctness and scalability for very large directory changes.

