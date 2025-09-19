# Testing Guide

This document covers testing approaches and frameworks available in Spacedrive Core.

## Overview

Spacedrive Core includes two test frameworks to handle different testing scenarios:

1. **Standard Test Framework** (`test_framework`) - For single-process unit and integration tests
2. **Cargo Test Subprocess Framework** (`test_framework_new`) - For multi-device networking tests

## Cargo Test Subprocess Framework

### When to Use

Use the cargo test subprocess framework when you need:
- Multi-device networking tests (pairing, file transfer, sync)
- Subprocess isolation for Core instances
- Parallel execution of different device roles
- Tests that simulate real network scenarios

### How It Works

The framework uses `cargo test` itself as the subprocess executor:

1. **Main Test**: Orchestrates the overall test scenario
2. **Device Scenarios**: Individual test functions for each device role
3. **Environment Coordination**: Uses env vars to control which role runs
4. **Process Management**: Spawns and monitors cargo test subprocesses

### Basic Structure

```rust
use sd_core::test_framework_new::CargoTestRunner;
use std::env;

// Device scenario - runs when TEST_ROLE matches
#[tokio::test]
#[ignore] // Only run when explicitly called
async fn alice_scenario() {
    // Exit early if not running as Alice
    if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
        return;
    }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR required"));

    // ALL test logic for Alice goes here
    let mut core = Core::new_with_config(data_dir).await?;
    // ... complete test implementation
}

// Main orchestrator
#[tokio::test]
async fn test_multi_device_scenario() {
    let mut runner = CargoTestRunner::new()
        .with_timeout(Duration::from_secs(90))
        .add_subprocess("alice", "alice_scenario")
        .add_subprocess("bob", "bob_scenario");

    runner.run_until_success(|outputs| {
        // Check for success patterns in output
        outputs.get("alice").map(|out| out.contains("SUCCESS")).unwrap_or(false) &&
        outputs.get("bob").map(|out| out.contains("SUCCESS")).unwrap_or(false)
    }).await.expect("Test failed");
}
```

### Environment Variables

The framework uses these environment variables for coordination:

- `TEST_ROLE` - Specifies which device role to run (`alice`, `bob`, etc.)
- `TEST_DATA_DIR` - Provides isolated temporary directory for each process

### Process Communication

Processes coordinate through:
- **File-based**: Temporary files for sharing data (pairing codes, state)
- **Output parsing**: Success/failure patterns in stdout/stderr
- **Timeouts**: Automatic cleanup if tests hang

### Example: Device Pairing Test

```rust
// Alice's role - all logic in test file
#[tokio::test]
#[ignore]
async fn alice_pairing_scenario() {
    if env::var("TEST_ROLE").unwrap_or_default() != "alice" { return; }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR required"));
    let mut core = Core::new_with_config(data_dir).await.unwrap();

    core.init_networking("test-password").await.unwrap();
    let (pairing_code, _) = core.start_pairing_as_initiator().await.unwrap();

    // Share pairing code with Bob
    std::fs::write("/tmp/pairing_code.txt", &pairing_code).unwrap();

    // Wait for Bob to connect
    loop {
        let devices = core.get_connected_devices().await.unwrap();
        if !devices.is_empty() {
            println!("PAIRING_SUCCESS: Alice connected");
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// Bob's role - all logic in test file
#[tokio::test]
#[ignore]
async fn bob_pairing_scenario() {
    if env::var("TEST_ROLE").unwrap_or_default() != "bob" { return; }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR required"));
    let mut core = Core::new_with_config(data_dir).await.unwrap();

    core.init_networking("test-password").await.unwrap();

    // Wait for Alice's pairing code
    let pairing_code = loop {
        if let Ok(code) = std::fs::read_to_string("/tmp/pairing_code.txt") {
            break code.trim().to_string();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };

    core.start_pairing_as_joiner(&pairing_code).await.unwrap();

    // Wait for connection
    loop {
        let devices = core.get_connected_devices().await.unwrap();
        if !devices.is_empty() {
            println!("PAIRING_SUCCESS: Bob connected");
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
```

### Running Tests

**Run the complete test:**
```bash
cargo test test_device_pairing --nocapture
```

**Debug individual scenarios:**
```bash
# Run just Alice's scenario
TEST_ROLE=alice TEST_DATA_DIR=/tmp/test cargo test alice_pairing_scenario -- --ignored --nocapture

# Run just Bob's scenario
TEST_ROLE=bob TEST_DATA_DIR=/tmp/test cargo test bob_pairing_scenario -- --ignored --nocapture
```

### Best Practices

1. **Use `#[ignore]`** on device scenario functions so they only run when explicitly called
2. **Exit early** if TEST_ROLE doesn't match to avoid unintended execution
3. **Use clear success patterns** in output for the condition function to detect
4. **Clean up shared files** in temporary locations to avoid test interference
5. **Set appropriate timeouts** based on the complexity of your test scenario
6. **Use descriptive test names** that clearly indicate the scenario being tested

### Debugging

**Check test output:**
```bash
# See detailed output from both processes
cargo test test_device_pairing --nocapture
```

**Run scenarios individually:**
```bash
# Test Alice's logic in isolation
TEST_ROLE=alice TEST_DATA_DIR=/tmp/debug cargo test alice_scenario -- --ignored --nocapture
```

**Common issues:**
- **Process hangs**: Check timeout settings and success condition logic
- **File conflicts**: Ensure unique temporary file paths for concurrent tests
- **Environment leakage**: Make sure TEST_ROLE guards are working correctly

## Standard Test Framework

For single-process tests, use the standard Rust testing approach:

```rust
#[tokio::test]
async fn test_core_initialization() {
    let core = Core::new().await.unwrap();
    assert!(core.device.device_id().is_ok());
}
```

## Legacy Test Framework

The original `test_framework` with scenarios is still available but deprecated in favor of the cargo test subprocess approach for multi-device tests.

## Writing New Tests

### For Single Device Tests
Use standard `#[tokio::test]` functions.

### For Multi-Device Tests
1. Create device scenario functions with `#[ignore]` and TEST_ROLE guards
2. Create a main orchestrator test using `CargoTestRunner`
3. Define clear success patterns for the condition function
4. Use appropriate timeouts and cleanup

### File Organization
- Put tests in `tests/` directory
- Use descriptive filenames: `test_device_pairing.rs`, `test_file_sync.rs`, etc.
- Keep all test logic in the test files themselves

## Examples

See `tests/core_pairing_test_new.rs` for a complete example of the cargo test subprocess framework in action.
