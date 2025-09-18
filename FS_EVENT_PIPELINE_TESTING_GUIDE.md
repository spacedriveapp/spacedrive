# FS Event Pipeline Resilience Testing Guide

## Overview
This guide explains how to test the FS Event Pipeline Resilience system with integrated metrics collection.

## What's Been Added

### 1. MetricsCollector Integration
- **Automatic Metrics Logging**: Metrics are now logged every 30 seconds (configurable)
- **Per-Location Metrics**: Each worker tracks its own metrics
- **Global Watcher Metrics**: Overall system metrics
- **Manual Metrics Trigger**: `watcher.log_metrics_now().await` for immediate logging

### 2. Enhanced Logging
- **Batch Processing**: `debug!("Processing batch of {} events for location {}")`
- **Coalescing Details**: Events are tracked and logged
- **Queue Monitoring**: Queue depth and overflow detection
- **Performance Metrics**: Batch sizes, processing times, coalescing rates

## Testing Setup

### 1. Build and Run
```bash
# Build the project
cargo build --release

# Set up detailed logging
export RUST_LOG="sd_core::service::watcher=debug,info"

# Start the daemon
cargo run --bin spacedrive -- start
```

### 2. Add Your Desktop Location
```bash
# Add your desktop as a location
cargo run --bin spacedrive -- location add /Users/jamespine/Desktop --name "Desktop Test"

# Verify it was added
cargo run --bin spacedrive -- location list
```

## What You'll See in Logs

### 1. Startup Logs
```
[timestamp] INFO sd_core::service::watcher: Starting location watcher service
[timestamp] INFO sd_core::service::watcher: Metrics collector started with 30000ms interval
[timestamp] INFO sd_core::service::watcher: Location watcher service started
```

### 2. Worker Creation
```
[timestamp] INFO sd_core::service::watcher::worker: Starting location worker for location abc-123-def
```

### 3. Batch Processing (Key Logs)
```
[timestamp] DEBUG sd_core::service::watcher::worker: Processing batch of 150 events for location abc-123-def
[timestamp] DEBUG sd_core::ops::indexing::responder: Create: /Users/jamespine/Desktop/test/file_1.txt
[timestamp] DEBUG sd_core::ops::indexing::responder: Create: /Users/jamespine/Desktop/test/file_2.txt
...
```

### 4. Periodic Metrics (Every 30 seconds)
```
[timestamp] INFO sd_core::service::watcher::metrics: Watcher metrics: locations=1, events_received=150, workers_created=1, workers_destroyed=0
[timestamp] INFO sd_core::service::watcher::metrics: Location abc-123-def metrics: processed=150, coalesced=25, batches=3, avg_batch_size=50.00, coalescing_rate=16.67%, max_queue_depth=5, max_batch_duration=45ms
```

### 5. Overflow Handling (if triggered)
```
[timestamp] WARN sd_core::service::watcher::worker: Queue depth 50001 exceeds threshold 50000 for location abc-123-def, triggering focused re-index
[timestamp] INFO sd_core::service::watcher::worker: Triggering focused re-index for location abc-123-def
```

## Test Scenarios

### 1. Basic File Operations
```bash
# Create test directory
mkdir -p ~/Desktop/fs-pipeline-test
cd ~/Desktop/fs-pipeline-test

# Create files (should see batch processing)
for i in {1..100}; do
    echo "Test $i" > "file_$i.txt"
done

# Watch for:
# - "Processing batch of X events" logs
# - Individual file operation logs
# - Metrics showing batch processing
```

### 2. Event Coalescing Test
```bash
# Test create + remove neutralization
touch ~/Desktop/fs-pipeline-test/temp.tmp
rm ~/Desktop/fs-pipeline-test/temp.tmp

# Test multiple modifies
for i in {1..10}; do
    echo "Modified $i" >> ~/Desktop/fs-pipeline-test/file_1.txt
done

# Watch for:
# - Neutralized events in metrics
# - Reduced number of actual operations
```

### 3. Burst Processing Test
```bash
# Create many files quickly (simulate git clone)
for i in {1..5000}; do
    echo "Burst test $i" > "burst_$i.txt"
done

# Watch for:
# - Large batch sizes in metrics
# - Efficient processing times
# - No queue overflow
```

### 4. Directory Operations Test
```bash
# Create nested structure
mkdir -p ~/Desktop/fs-pipeline-test/deep/nested/structure
touch ~/Desktop/fs-pipeline-test/deep/nested/structure/file.txt

# Move entire directory
mv ~/Desktop/fs-pipeline-test/deep ~/Desktop/fs-pipeline-test/moved_deep

# Watch for:
# - Parent-first ordering in logs
# - Efficient directory processing
```

## Manual Metrics Inspection

### 1. Trigger Immediate Metrics
```bash
# In a separate terminal, you can trigger metrics logging
# (This would require adding a CLI command or using the API)
```

### 2. Database Inspection
```bash
# Check entries in database
sqlite3 ~/.local/share/spacedrive/database.db

# Query recent entries
SELECT id, name, parent_id, entry_kind, created_at
FROM entries
WHERE created_at > datetime('now', '-1 hour')
ORDER BY created_at DESC LIMIT 20;

# Check directory paths
SELECT entry_id, path
FROM directory_paths
WHERE path LIKE '%fs-pipeline-test%'
ORDER BY path;
```

## Key Metrics to Monitor

### 1. Batch Processing Efficiency
- **Average Batch Size**: Should be large (50-100+ events per batch)
- **Processing Time**: Should be reasonable per batch
- **Batch Count**: Should be much less than individual events

### 2. Coalescing Effectiveness
- **Coalescing Rate**: Percentage of events that were coalesced
- **Neutralized Events**: Create+remove pairs that were neutralized
- **Rename Chains**: Collapsed rename operations

### 3. System Health
- **Queue Depth**: Should stay low (under 1000 typically)
- **No Overflow**: Should not trigger focused re-index
- **Worker Count**: Should match number of locations

## Troubleshooting

### 1. No Metrics Logging
- Check that `enable_metrics: true` in config
- Verify logging level includes `info`
- Look for "Metrics collector started" message

### 2. No Batch Processing
- Check that `debounce_window_ms` is set (default 150ms)
- Look for "Processing batch of X events" debug logs
- Verify events are being received

### 3. High Queue Depth
- Check if system is overloaded
- Look for overflow warnings
- Consider increasing `event_buffer_size`

## Configuration Options

### 1. Metrics Logging Frequency
```rust
let config = LocationWatcherConfig {
    metrics_log_interval_ms: 5000, // Log every 5 seconds
    enable_metrics: true,
    ..Default::default()
};
```

### 2. Batch Processing Tuning
```rust
let config = LocationWatcherConfig {
    debounce_window_ms: 100, // Shorter window for testing
    max_batch_size: 5000,    // Larger batches
    event_buffer_size: 100000, // Larger buffer
    ..Default::default()
};
```

### 3. Debug Mode
```rust
let config = LocationWatcherConfig {
    debug_mode: true, // Enable detailed debug logging
    ..Default::default()
};
```

## Expected Results

When working correctly, you should see:

1. **Efficient Batching**: Events grouped into batches of 50-100+ items
2. **Smart Coalescing**: Create+remove neutralized, multiple modifies collapsed
3. **Parent-First Ordering**: Directory operations before file operations
4. **No Event Loss**: All files end up in database correctly
5. **Good Performance**: Processing thousands of events efficiently
6. **Stable Queues**: No overflow or backpressure issues

The metrics will show you exactly how well the system is performing and help you tune the configuration for your specific use case.

