# Test Helpers

Shared utilities for integration tests to reduce duplication and improve maintainability.

## Modules

### `sync_harness.rs` - Two-Device Sync Test Utilities

Provides a comprehensive test harness for sync integration tests that eliminates ~200 lines of boilerplate per test.

**Key Components:**

#### `TwoDeviceHarnessBuilder`

Builder for creating pre-configured two-device test environments.

```rust
let harness = TwoDeviceHarnessBuilder::new("my_test")
    .await?
    .collect_events(true)           // Optional: collect event logs
    .collect_sync_events(true)      // Optional: collect sync events
    .start_in_ready_state(true)     // Optional: skip backfill (default)
    .build()
    .await?;
```

Automatically handles:

- Creating test directories
- Initializing tracing to files
- Setting up cores and libraries
- Registering pre-paired devices
- Configuring mock transports
- Starting sync services
- Setting sync state

#### `TwoDeviceHarness`

The resulting test harness with convenient methods:

```rust
// Add locations
harness.add_and_index_location_alice("/path", "Name").await?;
harness.add_and_index_location_bob("/path", "Name").await?;

// Wait for sync (sophisticated algorithm)
harness.wait_for_sync(Duration::from_secs(60)).await?;

// Capture comprehensive snapshot
harness.capture_snapshot("final_state").await?;

// Access all internals
harness.library_alice;
harness.device_alice_id;
harness.transport_alice;
```

#### Helper Functions

**Configuration:**

- `TestConfigBuilder` - Build test configs with custom filters
- `init_test_tracing()` - Standard tracing setup

**Device Setup:**

- `register_device()` - Register a device in a library
- `set_all_devices_synced()` - Mark devices as synced (prevent auto-backfill)

**Waiting:**

- `wait_for_indexing()` - Wait for indexing job completion
- `wait_for_sync()` - Sophisticated sync completion detection

**Operations:**

- `add_and_index_location()` - Create and index a location

**Snapshots:**

- `create_snapshot_dir()` - Create timestamped snapshot directory
- `SnapshotCapture` - Utilities for capturing databases, logs, events

### `sync_transport.rs` - Mock Network Transport

Mock implementation of `NetworkTransport` for testing sync without real networking.

**Key Features:**

- Immediate message delivery (like production)
- Request/response handling for backfill
- Device blocking/unblocking (simulate offline)
- Message history tracking
- Queue inspection

**Usage:**

```rust
// Single device
let transport = MockTransport::new_single(device_id);

// Paired devices (most common)
let (transport_a, transport_b) = MockTransport::new_pair(device_a, device_b);

// Block/unblock
transport.block_device(device_id).await;
transport.unblock_device(device_id).await;

// Inspect
let queue_size = transport.queue_size(device_id).await;
let total = transport.total_message_count().await;
```

### `test_volumes.rs` - Volume Test Utilities

Helper functions for creating mock volumes in tests (used by `sync_backfill_test.rs`).

## Benefits of Using Shared Utilities

### Code Reduction

- **~200 lines** of boilerplate eliminated per test
- **~2887 lines** saved across 6 sync tests (65% reduction)
- **One source of truth** for test patterns

### Consistency

- Same tracing setup everywhere
- Same config creation
- Same device registration
- Same snapshot format
- Same waiting algorithms

### Maintainability

- Fix bugs in one place
- Add features once, benefit everywhere
- Clear upgrade path for tests
- Easier code reviews

### Reliability

- Battle-tested algorithms
- Sophisticated sync detection
- Comprehensive snapshot capture
- Proper cleanup

## Migration Path

To migrate an existing sync test:

1. **Replace custom harness** with `TwoDeviceHarnessBuilder`
2. **Remove duplicated code** (config, registration, waiting)
3. **Use shared methods** (add_and_index_location_alice/bob)
4. **Test thoroughly** to ensure behavior unchanged

See [`REFACTORING_EXAMPLE.md`](../REFACTORING_EXAMPLE.md) for a detailed before/after comparison.

## When NOT to Use the Shared Harness

The shared harness is optimized for two-device real-time sync tests. Consider custom setup for:

- **Single-device tests** (use `MockTransport::new_single()`)
- **N-device tests** (N > 2)
- **Very specialized scenarios** (custom transport behavior)
- **Non-sync tests** (obviously!)

Even in these cases, you can still use the individual helper functions.

## Writing New Tests

**For new two-device sync tests:**

```rust
use helpers::TwoDeviceHarnessBuilder;

#[tokio::test]
async fn test_my_new_scenario() -> anyhow::Result<()> {
    let harness = TwoDeviceHarnessBuilder::new("my_scenario")
        .await?
        .build()
        .await?;

    // Your test logic here

    harness.capture_snapshot("final").await?;
    Ok(())
}
```

**For other test types:** Use individual helper functions as needed.

## Contributing

When adding new shared utilities:

1. **Add to `sync_harness.rs`** if it's sync-specific
2. **Add to appropriate module** otherwise
3. **Update this README** with usage examples
4. **Update `SYNC_HARNESS_USAGE.md`** if user-facing
5. **Export from `mod.rs`**

Keep utilities:

- **Generic** - Useful for multiple tests
- **Well-documented** - Clear purpose and usage
- **Battle-tested** - Used by actual tests
- **Simple** - Easy to understand and maintain

## Questions?

- See [`SYNC_HARNESS_USAGE.md`](../SYNC_HARNESS_USAGE.md) for detailed usage examples
- See [`REFACTORING_EXAMPLE.md`](../REFACTORING_EXAMPLE.md) for migration examples
- See [`SYNC_TESTS.md`](../SYNC_TESTS.md) for test suite overview
- Check the source code - it's well-commented!
