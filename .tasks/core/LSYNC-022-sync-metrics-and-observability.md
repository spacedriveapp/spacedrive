---
id: LSYNC-022
title: Sync Metrics and Observability System
status: Done
assignee: james
priority: High
tags: [sync, metrics, observability, monitoring]
last_updated: 2025-12-02
related_tasks: [LSYNC-010, LSYNC-021]
---

# Sync Metrics and Observability System

## Problem Statement

The sync system operates as a black box with no visibility into its behavior, performance, or health. This makes it impossible to:

- Debug sync issues ("Why didn't my files sync?")
- Monitor sync performance over time
- Identify bottlenecks or network problems
- Track bandwidth usage
- Understand sync state transitions
- Validate that sync is working correctly

Users and developers need comprehensive metrics to understand what the sync system is doing, when it's active, how much data has been synced, and where problems occur.

## Design Goals

1. **Real-time visibility** into sync state and operations
2. **Historical tracking** of sync behavior over time
3. **Performance metrics** for latency, throughput, and efficiency
4. **Error tracking** and debugging information
5. **Per-device and per-model breakdowns** of sync activity
6. **CLI query interface** for accessing metrics
7. **Event-driven updates** for UI integration
8. **Zero-cost when disabled** (minimal overhead)

## Architecture

### Metrics Collection

```
┌─────────────────────────────────────────┐
│         Sync Components                  │
│  (PeerSync, BackfillManager, etc.)      │
└──────────────┬──────────────────────────┘
               │ Record metrics
               ▼
┌─────────────────────────────────────────┐
│       SyncMetricsCollector               │
│  - Aggregates metrics from all sources  │
│  - Maintains atomic counters            │
│  - Tracks time-series data              │
└──────────────┬──────────────────────────┘
               │ Query
               ▼
┌─────────────────────────────────────────┐
│         Metrics Storage                  │
│  - In-memory ring buffer (last N)       │
│  - Persistent snapshots (database)      │
│  - Real-time atomic counters            │
└──────────────┬──────────────────────────┘
               │ Export
               ▼
┌─────────────────────────────────────────┐
│      Query Interfaces                    │
│  - CLI: `sd sync metrics`               │
│  - API: GraphQL/REST endpoint           │
│  - Events: Real-time updates            │
└─────────────────────────────────────────┘
```

## Key Metrics to Track

### 1. Sync State Metrics

```rust
struct SyncStateMetrics {
    // Current state
    current_state: SyncState, // Uninitialized, Backfilling, CatchingUp, Ready
    state_entered_at: DateTime<Utc>,

    // State history (last N transitions)
    state_history: VecDeque<StateTransition>,
    total_time_in_state: HashMap<SyncState, Duration>,
    transition_count: HashMap<(SyncState, SyncState), u64>,
}

struct StateTransition {
    from: SyncState,
    to: SyncState,
    timestamp: DateTime<Utc>,
    reason: Option<String>, // e.g., "peer connected", "backfill complete"
}
```

### 2. Operation Metrics

```rust
struct OperationMetrics {
    // Broadcasts
    broadcasts_sent: AtomicU64,
    state_changes_broadcast: AtomicU64,
    shared_changes_broadcast: AtomicU64,
    broadcast_batches_sent: AtomicU64,
    failed_broadcasts: AtomicU64,

    // Receives
    changes_received: AtomicU64,
    changes_applied: AtomicU64,
    changes_rejected: AtomicU64, // Failed to apply
    buffer_queue_depth: AtomicU64,

    // Backfill
    active_backfill_sessions: AtomicU64,
    backfill_sessions_completed: AtomicU64,
    backfill_pagination_rounds: AtomicU64,

    // Retries
    retry_queue_depth: AtomicU64,
    retry_attempts: AtomicU64,
    retry_successes: AtomicU64,
}
```

### 3. Data Volume Metrics

```rust
struct DataVolumeMetrics {
    // Per-model counters
    entries_synced: HashMap<String, AtomicU64>, // model_type -> count

    // Per-device counters
    entries_by_device: HashMap<Uuid, DeviceMetrics>,

    // Bytes transferred
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,

    // Last sync timestamps
    last_sync_per_peer: HashMap<Uuid, DateTime<Utc>>,
    last_sync_per_model: HashMap<String, DateTime<Utc>>,
}

struct DeviceMetrics {
    device_id: Uuid,
    device_name: String,
    entries_received: AtomicU64,
    last_seen: AtomicU64, // Unix timestamp
    is_online: AtomicBool,
}
```

### 4. Performance Metrics

```rust
struct PerformanceMetrics {
    // Latency tracking
    broadcast_latency_ms: HistogramMetric,
    apply_latency_ms: HistogramMetric,
    backfill_request_latency_ms: HistogramMetric,

    // Watermark tracking
    state_watermark: AtomicU64, // Unix timestamp
    shared_watermark: String,    // HLC string (protected by RwLock)
    watermark_lag_ms: HashMap<Uuid, AtomicU64>, // Per-peer lag

    // HLC drift
    hlc_physical_drift_ms: AtomicI64,
    hlc_counter_max: AtomicU64,

    // Database performance
    db_query_duration_ms: HistogramMetric,
    db_query_count: AtomicU64,
}

struct HistogramMetric {
    count: AtomicU64,
    sum: AtomicU64,
    min: AtomicU64,
    max: AtomicU64,
    // Could add percentiles later
}
```

### 5. Error Metrics

```rust
struct ErrorMetrics {
    // Error counts by type
    total_errors: AtomicU64,
    network_errors: AtomicU64,
    database_errors: AtomicU64,
    apply_errors: AtomicU64,
    validation_errors: AtomicU64,

    // Recent errors (ring buffer)
    recent_errors: Arc<RwLock<VecDeque<ErrorEvent>>>,

    // Conflict resolution
    conflicts_detected: AtomicU64,
    conflicts_resolved_by_hlc: AtomicU64,
}

struct ErrorEvent {
    timestamp: DateTime<Utc>,
    error_type: String,
    message: String,
    model_type: Option<String>,
    device_id: Option<Uuid>,
}
```

## Implementation Plan

### Phase 1: Core Infrastructure (2-3 days)

**Files to create:**
- `core/src/service/sync/metrics/mod.rs` - Main module
- `core/src/service/sync/metrics/collector.rs` - Central collector
- `core/src/service/sync/metrics/types.rs` - Metric types
- `core/src/service/sync/metrics/snapshot.rs` - Point-in-time snapshots
- `core/src/service/sync/metrics/history.rs` - Time-series storage

**Tasks:**
1. Define all metric types with atomic counters
2. Implement `SyncMetricsCollector` with thread-safe access
3. Create snapshot/export functionality
4. Add ring buffer for time-series data

### Phase 2: Integration (2-3 days)

**Files to modify:**
- `core/src/service/sync/peer.rs` - Add metrics recording
- `core/src/service/sync/backfill.rs` - Track backfill metrics
- `core/src/service/sync/state.rs` - Track state transitions
- `core/src/service/network/protocol/sync/handler.rs` - Track message handling

**Tasks:**
1. Add metrics recording to all sync operations
2. Record state transitions
3. Track latency for key operations
4. Record errors and retries

### Phase 3: CLI Interface (1-2 days)

**Files to create:**
- `crates/cli/src/commands/sync/metrics.rs` - CLI command

**Command structure:**
```bash
# Get current metrics snapshot
sd sync metrics

# Get metrics for specific time range
sd sync metrics --since "1 hour ago"
sd sync metrics --since "2025-10-23 10:00:00"

# Watch metrics in real-time
sd sync metrics --watch

# Get metrics for specific peer
sd sync metrics --peer <device-id>

# Get metrics for specific model type
sd sync metrics --model entry

# Export metrics as JSON
sd sync metrics --json

# Show only specific categories
sd sync metrics --state      # State transitions only
sd sync metrics --operations # Operation counters only
sd sync metrics --errors     # Recent errors only
```

**Output format:**
```
Sync Metrics (Library: My Library)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

State
  Current: Ready
  Uptime: 2h 34m 12s
  Last transition: 1h 23m ago (CatchingUp → Ready)

  Time in state:
    ├─ Ready: 1h 23m (58%)
    ├─ CatchingUp: 47m (33%)
    └─ Backfilling: 24m (9%)

Operations (Last hour)
  Broadcasts:
    ├─ Total: 1,234 messages
    ├─ State changes: 547 (44%)
    ├─ Shared changes: 687 (56%)
    └─ Batches: 23 (avg 53.7 items/batch)

  Receives:
    ├─ Changes received: 2,567
    ├─ Changes applied: 2,563 (99.8%)
    ├─ Changes rejected: 4 (0.2%)
    └─ Buffer queue: 0 items

  Backfill:
    ├─ Sessions: 2 completed
    ├─ Pagination rounds: 12
    └─ Avg session duration: 8.4s

Data Volume
  Total synced: 105,432 records

  By model:
    ├─ entries: 105,000 (99.6%)
    ├─ content_identities: 345 (0.3%)
    ├─ locations: 12 (0.01%)
    └─ tags: 75 (0.07%)

  By device:
    ├─ Device A (MacBook): 75,000 records (online)
    ├─ Device B (Desktop): 30,000 records (online)
    └─ This device: 432 records

  Bandwidth:
    ├─ Sent: 45.2 MB
    ├─ Received: 127.8 MB
    └─ Total: 173.0 MB

Performance
  Latency:
    ├─ Broadcast: avg 45ms, max 234ms
    ├─ Apply: avg 12ms, max 89ms
    └─ Backfill request: avg 342ms, max 2.1s

  Sync lag:
    ├─ Device A: 0ms (synced)
    ├─ Device B: 145ms (0.1s behind)
    └─ Watermark age: 2.3s

  HLC:
    ├─ Physical drift: +12ms
    └─ Counter max: 3

Errors (Last hour)
  Total: 3 errors

  By type:
    ├─ Network: 2 (timeout, connection reset)
    └─ Apply: 1 (foreign key constraint)

  Retry queue: 0 pending

  Recent errors:
    [13:45:23] Network timeout: failed to send to device B
    [13:42:10] Apply failed: missing parent entry (entry_id: 12345)
    [13:38:55] Network error: connection reset by peer
```

### Phase 4: API Integration (1 day)

**Files to create:**
- `core/src/ops/sync/get_metrics/mod.rs` - Query for metrics
- `core/src/ops/sync/get_metrics/action.rs` - Action implementation

**Query implementation:**
```rust
// Define the query
pub struct GetSyncMetrics;

impl LibraryQuery for GetSyncMetrics {
    type Input = GetSyncMetricsInput;
    type Output = SyncMetricsSnapshot;

    async fn execute(input: Self::Input, ctx: LibraryQueryContext) -> Result<Self::Output> {
        let metrics = ctx.library().sync_service()?.metrics();

        // Apply filters
        let mut snapshot = metrics.snapshot();

        if let Some(since) = input.since {
            snapshot.filter_since(since);
        }

        if let Some(peer_id) = input.peer_id {
            snapshot.filter_by_peer(peer_id);
        }

        if let Some(model_type) = input.model_type {
            snapshot.filter_by_model(&model_type);
        }

        Ok(snapshot)
    }
}

// Input/Output types
#[derive(Deserialize, Serialize, Type)]
pub struct GetSyncMetricsInput {
    pub since: Option<DateTime<Utc>>,
    pub peer_id: Option<Uuid>,
    pub model_type: Option<String>,
}

// Usage via ApiDispatcher:
let metrics = dispatcher
    .execute_library_query::<GetSyncMetrics>(input, session)
    .await?;
```

**Event emission:**
Emit events on metric updates for UI real-time display via the existing event bus:
```rust
event_bus.emit(Event::SyncMetricsUpdated {
    library_id,
    snapshot: metrics.snapshot(),
});
```

### Phase 5: Persistence (Optional - 1 day)

Store periodic snapshots in database for historical analysis:

```sql
CREATE TABLE sync_metrics_snapshots (
    id INTEGER PRIMARY KEY,
    library_id TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    snapshot_json TEXT NOT NULL  -- JSON of full metrics
);

CREATE INDEX idx_metrics_library_time
ON sync_metrics_snapshots(library_id, timestamp);
```

## Acceptance Criteria

- [ ] All metric types defined with thread-safe atomic counters
- [ ] `SyncMetricsCollector` integrated into sync components
- [ ] State transitions tracked and recorded
- [ ] Operation counters updated in real-time
- [ ] CLI command `sd sync metrics` displays formatted metrics
- [ ] CLI supports time range filtering (`--since`)
- [ ] CLI supports real-time watching (`--watch`)
- [ ] CLI supports per-device and per-model filtering
- [ ] Metrics exported as JSON for programmatic access
- [ ] Error events tracked with ring buffer
- [ ] Latency histograms track min/max/avg
- [ ] Zero measurable performance impact when metrics disabled
- [ ] Documentation added to library-sync.mdx
- [ ] Integration test validates metrics accuracy

## Testing Strategy

### Unit Tests
- Test atomic counter thread-safety
- Test histogram calculations
- Test ring buffer overflow behavior
- Test snapshot serialization

### Integration Tests
```rust
#[tokio::test]
async fn test_sync_metrics_tracking() {
    let (device_a, device_b) = setup_paired_devices().await;

    // Enable metrics
    let metrics = device_a.sync_metrics();

    // Perform sync operations
    create_entries(device_a, 100).await;
    wait_for_sync().await;

    // Verify metrics
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.operations.broadcasts_sent, 100);
    assert_eq!(snapshot.data_volume.entries_synced.get("entry"), Some(&100));
    assert!(snapshot.performance.broadcast_latency_ms.avg() < 100);
}
```

### Performance Tests
- Measure overhead of metrics collection
- Verify zero-cost when disabled
- Test with high sync volume (1M+ operations)

## Migration & Rollout

1. **Week 1**: Implement core infrastructure
2. **Week 1**: Integrate into sync components
3. **Week 2**: CLI interface and basic display
4. **Week 2**: API endpoints and events
5. **Week 3**: Polish, documentation, and testing

## Future Enhancements

- **Prometheus export**: Native Prometheus metrics endpoint
- **Grafana dashboards**: Pre-built monitoring dashboards
- **Alerting**: Configurable alerts for sync issues
- **Cost tracking**: Estimate cloud egress costs for bandwidth usage
- **Comparison mode**: Compare metrics across time periods
- **Anomaly detection**: ML-based detection of unusual sync patterns

## References

- [Indexer Metrics](../../core/src/ops/indexing/metrics.rs) - Similar pattern
- [Watcher Metrics](../../core/src/service/watcher/metrics.rs) - Atomic counter examples
- [LSYNC-010](./LSYNC-010-sync-service.md) - Sync service architecture
- [LSYNC-021](./LSYNC-021-unified-sync-config.md) - Configuration system

## Implementation Files

**New files:**
- `core/src/service/sync/metrics/mod.rs`
- `core/src/service/sync/metrics/collector.rs`
- `core/src/service/sync/metrics/types.rs`
- `core/src/service/sync/metrics/snapshot.rs`
- `core/src/service/sync/metrics/history.rs`
- `crates/cli/src/commands/sync/metrics.rs`

**Modified files:**
- `core/src/service/sync/peer.rs`
- `core/src/service/sync/backfill.rs`
- `core/src/service/sync/state.rs`
- `core/src/service/network/protocol/sync/handler.rs`
- `docs/core/library-sync.mdx`
