# Spacedrive Core Integration Tests

This directory contains integration tests for Spacedrive Core v2, focusing on end-to-end functionality and real-world usage scenarios.

## Test Files

### `cli_pairing_integration.rs`
**Complete CLI pairing workflow testing**

Tests the full device pairing functionality that would be used by the CLI:
- Two Core instances simulating different devices
- Pairing code generation and joining
- Automatic device registration
- Persistent connections across restarts
- Error handling and edge cases
- Session management APIs

**Key Tests:**
- `test_cli_pairing_full_workflow` - Complete pairing between two devices
- `test_cli_pairing_error_conditions` - Error handling and invalid inputs
- `test_cli_pairing_session_management` - Session lifecycle management

### `integration_networking.rs`
**Basic networking functionality**

Tests core networking initialization and basic functionality:
- Core networking initialization
- Device pairing integration
- Spacedrop API integration
- Networking service features

### Other Integration Tests
- `cas_generation_test.rs` - Content addressable storage testing
- `file_copy_integration_test.rs` - File operations testing
- `job_registration_test.rs` - Job system testing
- `job_system_test.rs` - Advanced job system functionality
- `library_test.rs` - Library management testing
- `volume_test.rs` - Volume detection and management

## Running Tests

### Run All Integration Tests
```bash
cargo test --tests
```

### Run Specific Test File
```bash
# CLI pairing tests
cargo test cli_pairing_integration

# Networking tests
cargo test integration_networking

# CAS tests
cargo test cas_generation_test
```

### Run Specific Test Function
```bash
# Full CLI pairing workflow
cargo test test_cli_pairing_full_workflow

# Error condition testing
cargo test test_cli_pairing_error_conditions
```

### Debug Mode with Logging
```bash
# Show all output and enable debug logging
RUST_LOG=debug cargo test cli_pairing_integration -- --nocapture

# Show output for specific networking components
RUST_LOG=sd_core_new::networking=debug cargo test cli_pairing_integration -- --nocapture

# Show libp2p debug logs
RUST_LOG=libp2p_swarm=debug,sd_core_new::networking::pairing::protocol=debug cargo test cli_pairing_integration -- --nocapture
```

### Single-threaded Testing
```bash
# Run tests one at a time (useful for networking tests)
cargo test cli_pairing_integration -- --test-threads=1
```

## Test Environment Notes

### CLI Pairing Tests
- Creates temporary directories for test data
- Tests real libp2p networking (may be slow in CI)
- Includes timeout handling for network operations
- Tests persistence across Core restarts
- Cleans up temporary files automatically

### Network-dependent Tests
Some tests require actual network functionality:
- May be slower in CI environments
- May timeout if network discovery fails
- Include graceful fallbacks for network issues

### Local Development
For faster iteration during development:
```bash
# Run only fast tests
cargo test --tests --exclude cli_pairing_integration

# Run CLI pairing tests with shorter timeouts
RUST_LOG=info cargo test test_cli_pairing_error_conditions -- --nocapture
```

## Test Data

Tests create temporary directories in the system temp directory:
- Pattern: `/tmp/test-{test-name}-{uuid}`
- Automatically cleaned up after tests
- Safe to delete manually if tests are interrupted

## Debugging Failed Tests

### Network Issues
```bash
# Check if networking is working
RUST_LOG=libp2p=debug cargo test test_cli_pairing_full_workflow -- --nocapture

# Test with extended timeouts
RUST_TEST_TIMEOUT=120 cargo test cli_pairing_integration
```

### Permission Issues
```bash
# Ensure temp directory is writable
ls -la /tmp/

# Check if ports are available
netstat -ln | grep 52063
```

### Cleanup Issues
```bash
# Clean up any remaining test directories
rm -rf /tmp/test-*
```

## Contributing

When adding new integration tests:

1. **Follow the naming convention**: `{feature}_integration.rs`
2. **Include comprehensive documentation** in the file header
3. **Add cleanup code** for any resources created
4. **Handle timeouts gracefully** for network operations
5. **Test both success and error conditions**
6. **Update this README** with new test descriptions

### Example Test Structure
```rust
#[tokio::test]
async fn test_new_feature_integration() {
    // Setup
    let temp_dir = std::env::temp_dir().join(format!("test-feature-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    // Test logic
    // ...
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}
```