# Cargo Test Subprocess Framework Design

## Overview

This design proposes a new test framework architecture that allows test logic to remain in test files while still supporting subprocess-based testing for multi-device scenarios. The key innovation is using `cargo test` itself as the subprocess executor, eliminating the need for function serialization or separate scenario modules.

## Current Problem

The existing test framework forces all test logic into the `scenarios` module because:
1. Tests need subprocess isolation for multi-device networking
2. Current approach requires pre-compiled binary (`test_core`)
3. Test logic is separated from actual test files
4. Makes tests harder to write, debug, and maintain

## Findings from Function Serialization Approach

During initial implementation, we discovered that function serialization in Rust is extremely complex and impractical:

1. **Rust Function Serialization Challenges**:
   - Functions are not serializable by default in Rust
   - Dynamic compilation requires complex proc macro infrastructure
   - Dependency management across process boundaries is non-trivial
   - Error handling and debugging becomes much more difficult

2. **Implementation Complexity**:
   - Would require a custom build system or proc macros
   - Cross-platform compatibility issues
   - Performance overhead from serialization/deserialization
   - Maintenance burden for a relatively simple use case

## Proposed Solution: Cargo Test Subprocess Pattern

### Core Architecture

```rust
// Test framework components
pub struct CargoTestRunner {
    processes: Vec<TestProcess>,
    global_timeout: Duration,
}

pub struct TestProcess {
    name: String,
    data_dir: TempDir,
    child: Option<Child>,
    output: String,
}
```

### Test File Structure

```rust
// tests/device_pairing_test.rs
use sd_core::test_framework_new::CargoTestRunner;
use sd_core::Core;
use std::path::PathBuf;
use std::env;

// Alice scenario - runs when TEST_ROLE=alice
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_pairing_scenario() {
    // Exit early if not running as Alice
    if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
        return;
    }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR not set"));
    let device_name = "Alice's Test Device";

    println!("Alice: Starting Core pairing test");

    // All Alice-specific test logic here - stays in the test file!
    let mut core = Core::new_with_config(data_dir).await.unwrap();
    core.device.set_name(device_name.to_string()).unwrap();

    core.init_networking("test-password").await.unwrap();

    let (pairing_code, _) = core.start_pairing_as_initiator().await.unwrap();

    // Write pairing code for Bob to read
    std::fs::create_dir_all("/tmp/spacedrive-pairing-test-cargo").unwrap();
    std::fs::write("/tmp/spacedrive-pairing-test-cargo/pairing_code.txt", &pairing_code).unwrap();

    // Wait for Bob to connect
    loop {
        let connected_devices = core.get_connected_devices().await.unwrap();
        if !connected_devices.is_empty() {
            println!("PAIRING_SUCCESS: Alice connected to Bob successfully");
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// Bob scenario - runs when TEST_ROLE=bob
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_pairing_scenario() {
    // Exit early if not running as Bob
    if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
        return;
    }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR not set"));
    let device_name = "Bob's Test Device";

    println!("Bob: Starting Core pairing test");

    // All Bob-specific test logic here - stays in the test file!
    let mut core = Core::new_with_config(data_dir).await.unwrap();
    core.device.set_name(device_name.to_string()).unwrap();

    core.init_networking("test-password").await.unwrap();

    // Wait for Alice's pairing code
    let pairing_code = loop {
        if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-pairing-test-cargo/pairing_code.txt") {
            break code.trim().to_string();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };

    core.start_pairing_as_joiner(&pairing_code).await.unwrap();

    // Wait for connection
    loop {
        let connected_devices = core.get_connected_devices().await.unwrap();
        if !connected_devices.is_empty() {
            println!("PAIRING_SUCCESS: Bob connected to Alice successfully");
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// Main test orchestrator
#[tokio::test]
async fn test_device_pairing() {
    println!("Testing device pairing with cargo test subprocess framework");

    let mut runner = CargoTestRunner::new()
        .with_timeout(Duration::from_secs(90))
        .add_subprocess("alice", "alice_pairing_scenario")
        .add_subprocess("bob", "bob_pairing_scenario");

    runner.run_until_success(|outputs| {
        let alice_success = outputs.get("alice")
            .map(|out| out.contains("PAIRING_SUCCESS: Alice connected to Bob successfully"))
            .unwrap_or(false);
        let bob_success = outputs.get("bob")
            .map(|out| out.contains("PAIRING_SUCCESS: Bob connected to Alice successfully"))
            .unwrap_or(false);

        alice_success && bob_success
    }).await.expect("Pairing test failed");

    println!("Device pairing test successful!");
}
```

## Implementation Strategy

### CargoTestRunner Implementation

```rust
impl CargoTestRunner {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            global_timeout: Duration::from_secs(60),
        }
    }

    pub fn add_subprocess(mut self, name: &str, test_function_name: &str) -> Self {
        let process = TestProcess {
            name: name.to_string(),
            test_function_name: test_function_name.to_string(),
            data_dir: TempDir::new().expect("Failed to create temp dir"),
            child: None,
            output: String::new(),
        };

        self.processes.push(process);
        self
    }

    pub async fn run_until_success<C>(&mut self, condition: C) -> Result<(), String>
    where
        C: Fn(&HashMap<String, String>) -> bool
    {
        // Spawn all subprocesses
        for process in &mut self.processes {
            let mut cmd = Command::new("cargo");
            cmd.args(&[
                "test",
                &process.test_function_name,
                "--",
                "--nocapture",
                "--ignored" // Run ignored tests
            ])
            .env("TEST_ROLE", &process.name)
            .env("TEST_DATA_DIR", process.data_dir.path().to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

            process.child = Some(cmd.spawn()?);
        }

        // Monitor until condition is met
        // ... rest of monitoring logic
    }
}
```

### Key Advantages of Cargo Test Approach

1. **No Function Serialization**: Uses cargo's built-in test runner
2. **Native Rust Support**: Leverages existing test infrastructure
3. **Simple Coordination**: Environment variables control test behavior
4. **Easy Debugging**: Can run individual test functions directly
5. **Parallel Execution**: Cargo handles subprocess management

## Technical Implementation Details

### Environment Variable Coordination

```rust
// Each test checks its role and exits early if not relevant
#[tokio::test]
#[ignore]
async fn alice_scenario() {
    if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
        return; // Exit early - not running as Alice
    }

    let data_dir = PathBuf::from(env::var("TEST_DATA_DIR").expect("TEST_DATA_DIR required"));

    // All Alice logic here - no external scenarios!
    // ...
}
```

### Process Spawning

```rust
// CargoTestRunner spawns cargo test with specific test names
let mut cmd = Command::new("cargo");
cmd.args(&[
    "test",
    "alice_scenario", // Specific test function name
    "--",
    "--nocapture",
    "--ignored"
])
.env("TEST_ROLE", "alice")
.env("TEST_DATA_DIR", data_dir_path);
```

### Communication Between Processes

- **File-based**: Temporary files for pairing codes, shared state
- **Environment**: TEST_ROLE and TEST_DATA_DIR for coordination
- **Output parsing**: Success patterns in stdout for completion detection

## Migration Plan

### Step 1: Build CargoTestRunner Framework
- Implement `CargoTestRunner` alongside existing framework
- Create process management and output monitoring
- No complex serialization infrastructure needed

### Step 2: Create New Test Structure
- Convert existing pairing test to cargo test approach
- All test logic moves into test functions with environment guards
- Update `core_pairing_test_cargo.rs` as proof of concept

### Step 3: Gradual Migration
- Keep existing framework intact during transition
- Migrate tests one by one to new approach
- Eventually remove old framework when all tests are converted

### Step 4: Remove Old Framework (Future)
- Delete `test_core` binary
- Remove `scenarios.rs` module
- Clean up unused infrastructure

## Benefits

1. **Test Logic Co-location**: All test code stays in test files where it belongs
2. **Better Developer Experience**: Easier to write, debug, and maintain tests
3. **No Serialization Complexity**: Uses native cargo test infrastructure
4. **Easy Debugging**: Can run individual test functions directly with env vars
5. **Simple Implementation**: Much simpler than function serialization approach
6. **Native Rust Support**: Leverages existing test tooling and conventions

## Comparison with Function Serialization Approach

| Aspect | Cargo Test Approach | Function Serialization |
|--------|-------------------|------------------------|
| **Complexity** | Simple - uses cargo test | Complex - custom serialization |
| **Debugging** | Easy - run test directly | Hard - requires special tooling |
| **Dependencies** | None - uses existing tools | Custom proc macros, serialization |
| **Performance** | Fast - native cargo | Slower - serialization overhead |
| **Maintenance** | Low - standard patterns | High - custom infrastructure |
| **Cross-platform** | Works everywhere cargo works | Potential platform issues |

## Potential Challenges

1. **Environment Variable Management**: Need clear conventions for env vars
2. **Test Isolation**: Ensure tests don't interfere when run separately
3. **Coordination Complexity**: File-based communication can be fragile
4. **Output Parsing**: Need robust patterns for success detection

## Alternative Approaches Considered

### A. Function Serialization (Initial Attempt)
- Extremely complex in Rust
- Requires custom build infrastructure
- Maintenance burden too high

### B. Container-Based Isolation
- Use Docker for process isolation
- Adds external dependencies
- Overkill for current needs

### C. Shared Library Approach
- Compile scenarios as dynamic libraries
- Platform-specific complications
- More complex than needed

## Success Criteria

1. Test logic remains in test files
2. No pre-compilation of test binaries required
3. Subprocess isolation maintained for networking tests
4. Easy to add new test scenarios
5. Good debugging experience
6. Simple implementation without complex infrastructure
7. Uses standard Rust/cargo tooling

## Timeline

- **Day 1**: Implement CargoTestRunner framework
- **Day 2**: Create proof of concept with pairing test
- **Day 3**: Test and refine the approach
- **Future**: Gradually migrate existing tests to new framework

## Conclusion

The cargo test subprocess approach is significantly simpler and more maintainable than function serialization while achieving the same goals. It leverages existing Rust tooling and conventions, making it easier to understand, debug, and maintain. All test logic stays exactly where it belongs - in the test files - while still providing the subprocess isolation needed for multi-device networking tests.
