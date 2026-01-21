# TypeScript Integration Tests with Rust Bridge

This directory contains end-to-end integration tests that bridge Rust and TypeScript, enabling real testing of TypeScript React hooks (`useNormalizedQuery`) against an actual Spacedrive daemon with indexed files.

## Architecture

The testing bridge works as follows:

1. **Rust Test Harness** (`core/tests/typescript_bridge_test.rs`)
   - Sets up a real Spacedrive daemon with RPC server
   - Indexes a test location with files
   - Writes connection config to JSON file
   - Spawns `bun test` to run TypeScript tests
   - Validates TypeScript test exit code

2. **Bridge Configuration** (JSON passed via `BRIDGE_CONFIG_PATH`)
   - `socket_addr`: TCP address of daemon (e.g., "127.0.0.1:41234")
   - `library_id`: UUID of test library
   - `location_db_id`: Database ID of indexed location
   - `location_path`: Physical filesystem path to test location
   - `test_data_path`: Temporary directory for test data

3. **TypeScript Test** (e.g., `useNormalizedQuery.test.ts`)
   - Reads bridge config from environment variable
   - Connects to daemon via `TcpSocketTransport`
   - Uses React Testing Library to test hooks
   - Performs filesystem operations (move, rename)
   - Asserts cache updates correctly via WebSocket events

## Running the Tests

### Setup (First Time Only)

Make sure dependencies are installed:

```bash
# From workspace root
bun install
```

This installs:

- `happy-dom` + `@happy-dom/global-registrator` - Fast, lightweight DOM environment (5-10x faster than jsdom)
- `@testing-library/react` - React hook testing utilities
- `@tanstack/react-query` - Query cache management
- All other required dependencies

### Full End-to-End Test (Rust → TypeScript)

```bash
cd core
cargo test --package sd-core --test typescript_bridge_test -- --nocapture
```

This will:

- Build Rust code
- Start daemon with test data
- Run TypeScript tests via Bun
- Display output from both Rust and TypeScript
- Fail if either side fails

### TypeScript Tests Only (Manual)

If you need to debug the TypeScript side independently:

```bash
# Terminal 1: Start a daemon manually
cd core
cargo run --bin sd-daemon

# Terminal 2: Run TypeScript tests with manual config
export BRIDGE_CONFIG_PATH=/path/to/bridge/config.json
bun test packages/ts-client/tests/integration/useNormalizedQuery.test.ts
```

## Test Scenarios

### File Move Test (`useNormalizedQuery.test.ts`)

Tests that `useNormalizedQuery` correctly updates cache when files move between folders:

1. Query `folder_a` directory listing
2. Query `folder_b` directory listing
3. Move `file1.txt` from `folder_a` to `folder_b` (filesystem operation)
4. Wait for watcher to detect change
5. Assert `file1` removed from `folder_a` cache
6. Assert `file1` added to `folder_b` cache with same UUID (move detection)

### Folder Rename Test (`useNormalizedQuery.folder-rename.test.ts`)

Tests that `useNormalizedQuery` correctly updates cache when folders are renamed:

1. Query root directory listing (contains `original_folder`)
2. Rename `original_folder` to `renamed_folder` (filesystem operation)
3. Wait for watcher to detect change
4. Assert `original_folder` removed from cache
5. Assert `renamed_folder` appears in cache with same UUID (identity preserved)

## Test Environment Setup

The integration tests use a DOM environment provided by **Happy DOM** (not jsdom) to support React Testing Library. Happy DOM is:

- **5-10x faster** than jsdom for React hook tests
- **Lighter weight** - optimized specifically for testing
- **Easier to configure** - one-liner setup with automatic global registration

### How It Works

- **`setup.ts`** - Imports `@happy-dom/global-registrator` and registers DOM globals
  ```typescript
  import { GlobalRegistrator } from "@happy-dom/global-registrator";
  GlobalRegistrator.register();
  ```
- **Test files** - Import `./setup` as the first line to initialize DOM before React imports
- **Cleanup** - Each test calls `cleanup()` from `@testing-library/react` after tests

This provides `document`, `window`, `HTMLElement`, and all other browser globals needed by React hooks.

## How It Works

### Daemon Setup (Rust Side)

The `IndexingHarnessBuilder` has been extended with `.enable_daemon()`:

```rust
let harness = IndexingHarnessBuilder::new("test_name")
    .enable_daemon()  // Starts RPC server on random port
    .build()
    .await?;

let socket_addr = harness.daemon_socket_addr().unwrap();
// socket_addr = "127.0.0.1:41234" (random available port)
```

The daemon runs in a background task, sharing the same `Core` instance with the test harness.

### TypeScript Connection

```typescript
import { SpacedriveClient } from "@sd/ts-client";

// Read config from Rust bridge
const bridgeConfig = JSON.parse(await readFile(process.env.BRIDGE_CONFIG_PATH));

// Connect via TCP
const client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);

// Set library context
await client.setLibrary(bridgeConfig.library_id);

// Now use hooks normally!
const { data } = useNormalizedQuery({
	wireMethod: "query:files.directory_listing",
	input: { path: { Physical: { path: "/some/path" } } },
	resourceType: "file",
	// ...
});
```

### Event Flow

```
Filesystem Change (rename/move)
    ↓
Watcher Detection (Rust)
    ↓
Indexing Update (Database)
    ↓
ResourceChanged Event (WebSocket)
    ↓
SubscriptionManager Filters (TypeScript)
    ↓
useNormalizedQuery Event Handler
    ↓
Cache Update (TanStack Query)
    ↓
React Re-render (Hooks)
```

## Benefits

1. **True End-to-End Testing**: Tests the entire stack from filesystem to React hooks
2. **Real Watcher Integration**: Tests actual file system watcher behavior, not mocks
3. **Cross-Language Validation**: Ensures Rust and TypeScript stay in sync
4. **Regression Detection**: Catches breaking changes in event emission, caching, or filtering
5. **Documentation by Example**: Tests serve as living examples of how the system works

## Adding New Tests

1. Create a new test in `typescript_bridge_test.rs`:

```rust
#[tokio::test]
async fn test_typescript_my_new_feature() -> anyhow::Result<()> {
    let harness = IndexingHarnessBuilder::new("my_test")
        .enable_daemon()
        .build()
        .await?;

    // Set up test data
    let test_location = harness.create_test_location("test").await?;
    test_location.write_file("test.txt", "content").await?;

    let location = test_location.index("Test", IndexMode::Shallow).await?;

    // Write bridge config (see other tests for example)
    let bridge_config = TestBridgeConfig { /* ... */ };
    // ...

    // Spawn TypeScript test
    let output = tokio::process::Command::new("bun")
        .arg("test")
        .arg("packages/ts-client/tests/integration/my-feature.test.ts")
        .env("BRIDGE_CONFIG_PATH", config_path.to_str().unwrap())
        .output()
        .await?;

    assert!(output.status.success());
    Ok(())
}
```

2. Create corresponding TypeScript test:

```typescript
import { SpacedriveClient } from "@sd/ts-client";
import { renderHook } from "@testing-library/react";

test("my feature works", async () => {
    const bridgeConfig = JSON.parse(/* read from env */);
    const client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);

    // Test your feature!
    const { result } = renderHook(() => useMyFeature(...));
    // ...
});
```

## Debugging

### Enable Debug Logging

```bash
# Rust side
RUST_LOG=debug cargo test typescript_bridge -- --nocapture

# TypeScript side (in test file)
const query = useNormalizedQuery({
    // ...
    debug: true,  // Enables console.log for event processing
});
```

### Common Issues

**"Connection refused"**: Daemon didn't start or took too long. Increase sleep duration in Rust test.

**"No events received"**: Watcher may be disabled or buffering. Check:

- Watcher is enabled in harness (default)
- Wait time is sufficient (8+ seconds for folder renames due to buffering)
- Path is correct and indexing completed

**"Cache not updating"**: Check:

- Event subscription filter matches query scope
- Resource type matches ("file", "location", etc.)
- pathScope is set correctly for file queries

## Future Enhancements

- [ ] Add tests for batch operations
- [ ] Test error handling and retry logic
- [ ] Test network interruption scenarios
- [ ] Add performance benchmarks
- [ ] Test concurrent operations
- [ ] Add tests for content identification events
- [ ] Test tag/label updates
