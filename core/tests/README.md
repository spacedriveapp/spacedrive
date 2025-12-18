# Sync Integration Tests

This directory contains integration tests for Spacedrive's sync system.

## Quick Links

- **[SYNC_TESTS.md](./SYNC_TESTS.md)** - Complete documentation of all sync test files
- **[SYNC_HARNESS_USAGE.md](./SYNC_HARNESS_USAGE.md)** - How to use shared test utilities
- **[REFACTORING_SUMMARY.md](./REFACTORING_SUMMARY.md)** - Refactoring impact summary
- **[helpers/README.md](./helpers/README.md)** - Helper module documentation

## Writing New Tests

Use the shared test harness for two-device sync tests:

```rust
use helpers::TwoDeviceHarnessBuilder;

#[tokio::test]
async fn test_my_scenario() -> anyhow::Result<()> {
    let harness = TwoDeviceHarnessBuilder::new("my_scenario")
        .await?
        .build()
        .await?;

    // Alice indexes a location
    harness.add_and_index_location_alice("/path", "Name").await?;
    
    // Wait for sync to complete
    harness.wait_for_sync(Duration::from_secs(60)).await?;
    
    // Capture snapshot
    harness.capture_snapshot("final").await?;

    Ok(())
}
```

## Running Tests

```bash
# Run all sync tests
cargo test -p sd-core --test 'sync_*' -- --test-threads=1 --nocapture

# Run specific test file
cargo test -p sd-core --test sync_realtime_test -- --test-threads=1 --nocapture

# Run specific test
cargo test -p sd-core --test sync_realtime_test test_realtime_sync_alice_to_bob -- --nocapture
```

**Important:** Use `--test-threads=1` to prevent tests from interfering with each other.

## Test Snapshots

All tests capture comprehensive snapshots to:
```
~/Library/Application Support/spacedrive/sync_tests/snapshots/{test_name}_{timestamp}/
```

Each snapshot includes:
- `test.log` - Complete trace output
- `alice/database.db` - Alice's database
- `alice/sync.db` - Alice's sync database
- `alice/logs/` - Alice's library logs
- `bob/database.db` - Bob's database
- `bob/sync.db` - Bob's sync database
- `bob/logs/` - Bob's library logs
- `summary.md` - Test results summary

## Current Test Files

### Core Sync Tests
- `sync_realtime_test.rs` - Real-time sync between pre-paired devices
- `sync_backfill_test.rs` - Initial backfill when devices first connect
- `sync_backfill_race_test.rs` - Race condition between backfill and live events
- `sync_metrics_test.rs` - Metrics tracking validation
- `sync_event_log_test.rs` - Event logging system tests
- `sync_setup_test.rs` - Sync setup with subprocess framework

### Helper Infrastructure
- `helpers/sync_harness.rs` - Shared test utilities
- `helpers/sync_transport.rs` - Mock network transport
- `helpers/test_volumes.rs` - Volume testing utilities
- `helpers/mod.rs` - Module exports

## Refactoring Stats

**55% code reduction** across refactored tests:
- Eliminated **2046 lines** of duplicated code
- Added **600 lines** of shared infrastructure
- Net savings: **~1446 lines**

See [REFACTORING_SUMMARY.md](./REFACTORING_SUMMARY.md) for details.
