# Location Watcher Integration Test

## Overview

This test validates the location watcher's real-time filesystem monitoring through a comprehensive "story" of file operations.

## Test Harness

The `TestHarness` struct provides a DRY way to test filesystem operations:

```rust
let mut harness = TestHarness::setup().await?;

// Create a file and verify it's detected & indexed
harness.create_file("document.txt", "content").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("document.txt") },
    30
).await?;
harness.verify_entry_exists("document").await?;
harness.verify_entry_metadata("document", Some(7), Some("txt")).await?;
```

### Helper Methods

- `create_file(name, content)` - Create a file
- `modify_file(name, new_content)` - Modify file content  
- `delete_file(name)` - Delete a file
- `rename_file(from, to)` - Rename/move a file
- `create_dir(name)` - Create a directory
- `delete_dir(name)` - Delete a directory (recursive)
- `wait_for_fs_event(kind, timeout)` - Wait for specific FsRawChange event
- `verify_entry_exists(name)` - Assert entry exists in database
- `verify_entry_not_exists(name)` - Assert entry doesn't exist
- `verify_entry_count(expected)` - Assert total entry count
- `verify_entry_metadata(name, size, ext)` - Verify entry details

## Test Directory

Tests use `~/SD_TEST_DIR` (cross-platform home directory detection):
- **macOS/Linux**: `$HOME/SD_TEST_DIR`
- **Windows**: `%USERPROFILE%\SD_TEST_DIR`

Directory is cleared before each test run and cleaned up after.

## Bugs Fixed

### 1. Location Watcher Not Detecting New Locations
**Problem**: Watcher only loaded locations on startup, missed new locations added later.  
**Solution**: Added event listener for `LocationAdded` events in `LocationWatcher::start_location_event_listener()`.

### 2. macOS Platform Handler Not Emitting Events  
**Problem**: macOS handler emitted `EntryCreated` instead of `FsRawChange` for creates, and nothing for modifies.  
**Solution**: Updated `MacOSHandler::process_event()` to emit proper `FsRawChange` events for Create, Modify, and Remove.

### 3. FsRawChange Events Not Reaching EventBus
**Problem**: Events were created but not emitted to subscribers.  
**Solution**: Added `events.emit()` call in `LocationWatcher::start_event_loop()`.

### 4. Duplicate Entries on File Modification (macOS)
**Problem**: macOS reports modifications as Create events, responder blindly created duplicates.  
**Solution**: Modified `handle_move_by_inode()` to detect same-path, same-inode events and update instead of create.

## Currently Working Scenarios

Initial indexing  
File creation  
File modification (no duplicates!)  
Directory creation  
Nested file creation  

## TODO Scenarios

️ File renaming (needs inode lookup from database for deleted paths)  
️ File/directory deletion (investigate task panic)  
️ Bulk operations  

## Usage

```bash
# Run all location watcher tests
cargo test --test location_watcher_test

# Run specific test
cargo test test_location_watcher --test location_watcher_test -- --nocapture

# With custom test directory
SD_TEST_DIR=/tmp/my_test cargo test test_location_watcher
```

## Adding New Scenarios

Adding new test scenarios is trivial with the harness:

```rust
// Scenario X: Your New Test
harness.create_file("test.txt", "content").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("test.txt") },
    30
).await?;
harness.verify_entry_exists("test").await?;
```

That's it! 3 lines per operation.
