# Location Watcher Test - Quick Start Guide

## Running the Tests

```bash
# Run the main story test
cd core && cargo test test_location_watcher --test location_watcher_test -- --nocapture

# With custom test directory
SD_TEST_DIR=/tmp/mytest cargo test test_location_watcher --test location_watcher_test -- --nocapture
```

## Adding a New Test Scenario

It's just 3 lines:

```rust
// 1. Perform filesystem operation
harness.create_file("myfile.txt", "content").await?;

// 2. Wait for watcher to detect it
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("myfile.txt") },
    30  // timeout in seconds
).await?;

// 3. Verify database was updated
harness.verify_entry_exists("myfile").await?;
```

## Available Operations

### File Operations
```rust
// Create
harness.create_file("doc.txt", "Hello World").await?;
harness.verify_entry_exists("doc").await?;

// Modify
harness.modify_file("doc.txt", "Updated content").await?;
harness.verify_entry_metadata("doc", Some(15), Some("txt")).await?;

// Rename
harness.rename_file("old.txt", "new.txt").await?;
harness.verify_entry_exists("new").await?;

// Delete
harness.delete_file("doc.txt").await?;
harness.verify_entry_not_exists("doc").await?;
```

### Directory Operations
```rust
// Create directory
harness.create_dir("mydir").await?;
harness.verify_entry_exists("mydir").await?;

// Create nested file
harness.create_file("mydir/nested.txt", "content").await?;
harness.verify_entry_exists("nested").await?;

// Delete directory (recursive)
harness.delete_dir("mydir").await?;
harness.verify_entry_not_exists("mydir").await?;
```

### Verification
```rust
// Check entry exists
harness.verify_entry_exists("filename").await?;

// Check entry doesn't exist
harness.verify_entry_not_exists("filename").await?;

// Check total count
harness.verify_entry_count(5).await?;

// Check metadata
harness.verify_entry_metadata(
    "filename",
    Some(1024),      // expected size in bytes (None to skip)
    Some("txt")      // expected extension (None to skip)
).await?;
```

### Event Waiting
```rust
// Wait for any FsRawChange event
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("file.txt") },
    30  // timeout seconds
).await?;

harness.wait_for_fs_event(
    FsRawEventKind::Modify { path: harness.path("file.txt") },
    30
).await?;

harness.wait_for_fs_event(
    FsRawEventKind::Remove { path: harness.path("file.txt") },
    30
).await?;

harness.wait_for_fs_event(
    FsRawEventKind::Rename {
        from: harness.path("old.txt"),
        to: harness.path("new.txt")
    },
    30
).await?;
```

## Example: Testing a New Feature

Let's say you want to test hidden file detection:

```rust
println!("\n--- Scenario: Hidden Files ---");

// Create hidden file (Unix)
#[cfg(unix)]
{
    harness.create_file(".hidden", "secret").await?;
    harness.wait_for_fs_event(
        FsRawEventKind::Create { path: harness.path(".hidden") },
        30
    ).await?;
    harness.verify_entry_exists(".hidden").await?;

    // Verify it's marked as hidden in the database
    let entry = harness.verify_entry_exists(".hidden").await?;
    // Note: You'd need to add a hidden field check to verify_entry_metadata
}
```

## Performance Testing

```rust
println!("\n--- Performance: Bulk Creates ---");

let start = std::time::Instant::now();

for i in 1..=100 {
    harness.create_file(&format!("bulk-{}.txt", i), "content").await?;
}

// Wait for last one
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("bulk-100.txt") },
    30
).await?;

let duration = start.elapsed();
let throughput = 100.0 / duration.as_secs_f64();

println!("Created 100 files in {:?}", duration);
println!("Throughput: {:.2} files/sec", throughput);

assert!(throughput > 50.0, "Should be able to create >50 files/sec");
```

## Tips

1. **Always wait for events** - Don't assume instant detection
2. **Use proper timeouts** - macOS FSEvents can take 1-3 seconds
3. **Entry names don't include extensions** - Database stores name and extension separately
4. **Check counts after sequences** - Ensure operations completed

## Common Pitfalls

❌ **Don't do this**:
```rust
harness.create_file("test.txt", "content").await?;
harness.verify_entry_exists("test").await?;  // Might fail - didn't wait for event!
```

✅ **Do this**:
```rust
harness.create_file("test.txt", "content").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("test.txt") },
    30
).await?;
harness.verify_entry_exists("test").await?;  // Safe - event confirmed
```

## Current Limitations

- ✅ Rename operations working! (uses database inode lookup)
- ⚠️ Deletion operations cause panic (being investigated)
- ⚠️ Test uses `~/SD_TEST_DIR` (requires real directory for macOS FSEvents)

## File Locations

- **Test**: `core/tests/location_watcher_test.rs`
- **Investigation Doc**: `docs/core/location_watcher_investigation.md`
- **Watcher Service**: `core/src/service/watcher/mod.rs`
- **macOS Handler**: `core/src/service/watcher/platform/macos.rs`
- **Responder**: `core/src/ops/indexing/responder.rs`

