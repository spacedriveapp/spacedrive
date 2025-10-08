# Additional Test Scenarios for Location Watcher

## Currently Passing ✅
1. Initial indexing
2. File creation
3. File modification
4. Directory creation
5. Nested file creation
6. File renaming (same directory)
7. File moving (different directory)

## Next Priority Tests

### Tier 1: Critical Operations (Should work next)

#### Scenario 8: File Deletion
```rust
println!("\n--- Scenario 8: File Deletion ---");

// Delete a file
let entry_before = harness.verify_entry_exists("initial").await?;
let entry_id = entry_before.id;

harness.delete_file("initial.txt").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Remove { path: harness.path("initial.txt") },
    30
).await?;

// Verify entry no longer exists
harness.verify_entry_not_exists("initial").await?;
harness.verify_entry_count(6).await?; // One less entry

// Verify entry is actually deleted (not just orphaned)
let entry_still_exists = entities::entry::Entity::find_by_id(entry_id)
    .one(harness.library.db().conn())
    .await?;
assert!(entry_still_exists.is_none(), "Entry should be deleted from database");
```

**Known Issue**: Currently causes task panic - needs investigation

#### Scenario 9: Directory Deletion (With Contents)
```rust
println!("\n--- Scenario 9: Directory Deletion ---");

// Create directory with multiple files
harness.create_dir("temp").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Create { path: harness.path("temp") },
    30
).await?;

harness.create_file("temp/file1.txt", "content 1").await?;
harness.create_file("temp/file2.txt", "content 2").await?;
harness.create_file("temp/file3.txt", "content 3").await?;

// Wait for all files to be indexed
tokio::time::sleep(Duration::from_secs(1)).await;
let count_before = count_location_entries(&harness.library, harness.location_id).await?;

// Delete entire directory
harness.delete_dir("temp").await?;
harness.wait_for_fs_event(
    FsRawEventKind::Remove { path: harness.path("temp") },
    30
).await?;

// Wait for cascade deletion
tokio::time::sleep(Duration::from_millis(500)).await;

// Verify all entries are gone (directory + 3 files = 4 entries)
let count_after = count_location_entries(&harness.library, harness.location_id).await?;
assert_eq!(count_after, count_before - 4, "Should delete directory and all contents");
```

### Tier 2: Complex Rename Operations

#### Scenario 10: Bulk Renames
```rust
println!("\n--- Scenario 10: Bulk Renames ---");

// Create 10 files
for i in 1..=10 {
    harness.create_file(&format!("bulk-{}.txt", i), "content").await?;
}
tokio::time::sleep(Duration::from_secs(1)).await;
harness.verify_entry_count(count_before + 10).await?;

// Rename all files rapidly
let start = Instant::now();
for i in 1..=10 {
    harness.rename_file(
        &format!("bulk-{}.txt", i),
        &format!("renamed-bulk-{}.txt", i)
    ).await?;
}

// Wait for all rename events
tokio::time::sleep(Duration::from_secs(1)).await;

// Verify all renamed correctly
for i in 1..=10 {
    harness.verify_entry_exists(&format!("renamed-bulk-{}", i)).await?;
    harness.verify_entry_not_exists(&format!("bulk-{}", i)).await?;
}

let duration = start.elapsed();
println!("✓ Renamed 10 files in {:?}", duration);
assert!(duration.as_millis() < 2000, "Bulk renames should complete in <2s");
harness.verify_entry_count(count_before + 10).await?; // Same count!
```

**Success Metric**: 10 renames in <2s, no duplicates

####  Scenario 11: Rename Chain (A→B, B→C)
```rust
println!("\n--- Scenario 11: Rename Chain ---");

harness.create_file("step1.txt", "content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

let entry_start = harness.verify_entry_exists("step1").await?;
let entry_id = entry_start.id;

// Rapid renames: step1 → step2 → step3
harness.rename_file("step1.txt", "step2.txt").await?;
tokio::time::sleep(Duration::from_millis(50)).await; // Small delay
harness.rename_file("step2.txt", "step3.txt").await?;

tokio::time::sleep(Duration::from_secs(1)).await;

// Verify final state
let entry_end = harness.verify_entry_exists("step3").await?;
assert_eq!(entry_id, entry_end.id, "Entry ID should be preserved through rename chain");
harness.verify_entry_not_exists("step1").await?;
harness.verify_entry_not_exists("step2").await?;
```

**Tests**: Worker's rename chain collapsing logic

### Tier 3: Edge Cases

#### Scenario 12: Hidden Files
```rust
println!("\n--- Scenario 12: Hidden Files ---");

#[cfg(unix)]
{
    harness.create_file(".hidden-file", "secret").await?;
    tokio::time::sleep(Duration::from_millis(600)).await;

    // Hidden files should still be indexed
    harness.verify_entry_exists(".hidden-file").await?;

    // Rename hidden file
    harness.rename_file(".hidden-file", ".hidden-renamed").await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    harness.verify_entry_exists(".hidden-renamed").await?;
}
```

#### Scenario 13: Symlink Handling
```rust
println!("\n--- Scenario 13: Symlink Handling ---");

harness.create_file("target.txt", "target content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

#[cfg(unix)]
{
    std::os::unix::fs::symlink(
        harness.path("target.txt"),
        harness.path("link.txt")
    )?;

    tokio::time::sleep(Duration::from_millis(600)).await;

    // Verify symlink is detected
    let entries = get_location_entries(&harness.library, harness.location_id).await?;
    let symlink_entry = entries.iter().find(|e| e.name == "link");
    assert!(symlink_entry.is_some(), "Symlink should be indexed");
}
```

#### Scenario 14: Rapid File Modifications (Debouncing Test)
```rust
println!("\n--- Scenario 14: Rapid Modifications ---");

harness.create_file("rapidly-changing.txt", "v1").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

let start_count = count_location_entries(&harness.library, harness.location_id).await?;

// Modify the same file 20 times rapidly
for i in 2..=20 {
    harness.modify_file("rapidly-changing.txt", &format!("v{}", i)).await?;
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// Wait for debouncing to settle
tokio::time::sleep(Duration::from_secs(2)).await;

// Should NOT create 20 entries - should coalesce into updates
let end_count = count_location_entries(&harness.library, harness.location_id).await?;
assert_eq!(end_count, start_count, "Rapid modifications should not create duplicates");

// Verify final content
let entry = harness.verify_entry_exists("rapidly-changing").await?;
// Size should reflect last modification
```

**Tests**: Debouncing and coalescing logic

#### Scenario 15: Same Name After Delete (Recreate)
```rust
println!("\n--- Scenario 15: Recreate Same Filename ---");

harness.create_file("temp-file.txt", "first version").await?;
tokio::time::sleep(Duration::from_millis(600)).await;
let entry_first = harness.verify_entry_exists("temp-file").await?;
let first_id = entry_first.id;
let first_inode = entry_first.inode;

// Delete it
harness.delete_file("temp-file.txt").await?;
tokio::time::sleep(Duration::from_millis(600)).await;
harness.verify_entry_not_exists("temp-file").await?;

// Create new file with same name
harness.create_file("temp-file.txt", "second version different content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

let entry_second = harness.verify_entry_exists("temp-file").await?;

// Should be a DIFFERENT entry (different ID and inode)
assert_ne!(entry_second.id, first_id, "Should create new entry, not reuse old one");
assert_ne!(entry_second.inode, first_inode, "Should have different inode");
```

**Tests**: Entry identity vs filename - ensures we don't reuse deleted entries

### Tier 4: Performance & Stress Tests

#### Scenario 16: Bulk Create Performance
```rust
println!("\n--- Scenario 16: Bulk Create Performance ---");

let start = Instant::now();

// Create 100 files rapidly
for i in 1..=100 {
    harness.create_file(&format!("perf-{}.txt", i), "test content").await?;
}

// Wait for all to be indexed
tokio::time::sleep(Duration::from_secs(3)).await;

let duration = start.elapsed();
let throughput = 100.0 / duration.as_secs_f64();

println!("✓ Created 100 files in {:?}", duration);
println!("  Throughput: {:.2} files/sec", throughput);

// Verify all indexed
for i in 1..=100 {
    harness.verify_entry_exists(&format!("perf-{}", i)).await?;
}

// Performance assertions
assert!(throughput > 20.0, "Should handle >20 files/sec even with debouncing");
assert!(duration.as_secs() < 10, "Should complete 100 files in <10s");
```

**Success Metrics**: >20 files/sec, <10s total, >50% coalescing rate

#### Scenario 17: Mixed Operations Chaos Test
```rust
println!("\n--- Scenario 17: Mixed Operations Chaos ---");

// Rapid mixed operations
harness.create_file("chaos1.txt", "1").await?;
harness.create_file("chaos2.txt", "2").await?;
tokio::time::sleep(Duration::from_millis(100)).await;

harness.rename_file("chaos1.txt", "chaos1-renamed.txt").await?;
harness.modify_file("chaos2.txt", "2 modified").await?;
tokio::time::sleep(Duration::from_millis(100)).await;

harness.create_file("chaos3.txt", "3").await?;
harness.delete_file("chaos2.txt").await?; // If deletion works
tokio::time::sleep(Duration::from_millis(100)).await;

harness.rename_file("chaos3.txt", "chaos-final.txt").await?;

// Wait for all operations to settle
tokio::time::sleep(Duration::from_secs(2)).await;

// Verify final state
harness.verify_entry_exists("chaos1-renamed").await?;
harness.verify_entry_exists("chaos-final").await?;
harness.verify_entry_not_exists("chaos1").await?;
harness.verify_entry_not_exists("chaos2").await?; // If deletion works
harness.verify_entry_not_exists("chaos3").await?;
```

**Tests**: Correctness under complex interleaved operations

### Tier 5: Platform-Specific Edge Cases

#### Scenario 18: Special Characters in Filenames
```rust
println!("\n--- Scenario 18: Special Characters ---");

let special_names = vec![
    "file with spaces.txt",
    "file-with-dashes.txt",
    "file_with_underscores.txt",
    "file (with parens).txt",
    "file[with brackets].txt",
    "file{with braces}.txt",
    "日本語.txt", // Japanese
    "émojis🎉.txt", // Unicode emoji
];

for name in special_names {
    harness.create_file(name, "content").await?;
}

tokio::time::sleep(Duration::from_secs(1)).await;

// Verify all were indexed correctly
for name in special_names {
    let stem = Path::new(name).file_stem().unwrap().to_str().unwrap();
    harness.verify_entry_exists(stem).await?;
}
```

#### Scenario 19: Very Long Filenames
```rust
println!("\n--- Scenario 19: Long Filenames ---");

// Create file with 200-character name
let long_name = format!("{}.txt", "a".repeat(200));
harness.create_file(&long_name, "content").await?;

tokio::time::sleep(Duration::from_millis(600)).await;

let stem = "a".repeat(200);
harness.verify_entry_exists(&stem).await?;
```

#### Scenario 20: Deep Nesting
```rust
println!("\n--- Scenario 20: Deep Directory Nesting ---");

// Create 20-level deep directory structure
let mut path = String::new();
for i in 1..=20 {
    if !path.is_empty() {
        path.push('/');
    }
    path.push_str(&format!("level{}", i));
}

harness.create_dir(&path).await?;
tokio::time::sleep(Duration::from_millis(600)).await;

// Create file at deepest level
harness.create_file(&format!("{}/deep-file.txt", path), "deep content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

harness.verify_entry_exists("deep-file").await?;
```

### Tier 6: Rename Edge Cases

#### Scenario 21: Rename While Modifying
```rust
println!("\n--- Scenario 21: Rename During Modification ---");

harness.create_file("busy.txt", "initial").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

let entry_before = harness.verify_entry_exists("busy").await?;

// Modify and rename almost simultaneously
harness.modify_file("busy.txt", "modified content").await?;
tokio::time::sleep(Duration::from_millis(50)).await; // Very short delay
harness.rename_file("busy.txt", "busy-renamed.txt").await?;

tokio::time::sleep(Duration::from_secs(1)).await;

// Verify final state
let entry_after = harness.verify_entry_exists("busy-renamed").await?;
assert_eq!(entry_after.id, entry_before.id, "Should preserve entry ID");
assert_eq!(entry_after.size, 16, "Should have updated size");
```

#### Scenario 22: Rename to Existing Name (Overwrite)
```rust
println!("\n--- Scenario 22: Rename Overwrite ---");

harness.create_file("source.txt", "source content").await?;
harness.create_file("target.txt", "target content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

let source_entry = harness.verify_entry_exists("source").await?;
let target_entry = harness.verify_entry_exists("target").await?;

// Rename source to target (overwrites target on filesystem)
harness.rename_file("source.txt", "target.txt").await?;

tokio::time::sleep(Duration::from_secs(1)).await;

// target.txt should now have source's content and ID
let final_entry = harness.verify_entry_exists("target").await?;
// On macOS/Unix, the source file's inode is kept
assert_eq!(final_entry.inode, source_entry.inode, "Should keep source's inode");
harness.verify_entry_not_exists("source").await?;
```

#### Scenario 23: Move to Non-Existent Directory
```rust
println!("\n--- Scenario 23: Move to Non-Existent Parent ---");

harness.create_file("orphan.txt", "content").await?;
tokio::time::sleep(Duration::from_millis(600)).await;

// Try to move to directory that doesn't exist
// macOS will fail the rename operation, should handle gracefully
let result = tokio::fs::rename(
    harness.path("orphan.txt"),
    harness.path("nonexistent/orphan.txt")
).await;

assert!(result.is_err(), "Should fail to move to non-existent directory");

// Entry should still exist at original location
harness.verify_entry_exists("orphan").await?;
```

### Tier 7: Performance Validation

#### Scenario 24: Event Coalescing Rate
```rust
println!("\n--- Scenario 24: Coalescing Validation ---");

// Create 100 files in same directory (should heavily coalesce)
for i in 1..=100 {
    harness.create_file(&format!("coalesce-test-{}.txt", i), "c").await?;
}

tokio::time::sleep(Duration::from_secs(2)).await;

// Get worker metrics
if let Some(metrics) = harness.core.services.location_watcher
    .get_location_metrics(harness.location_id)
    .await
{
    let coalescing_rate = metrics.coalescing_rate();
    println!("  Coalescing rate: {:.2}%", coalescing_rate * 100.0);
    println!("  Events processed: {}", metrics.events_processed());
    println!("  Batches sent: {}", metrics.batches_sent());
    println!("  Avg batch size: {:.2}", metrics.avg_batch_size());

    assert!(coalescing_rate > 0.3, "Should coalesce >30% for bulk creates in same dir");
}
```

#### Scenario 25: Memory Leak Detection
```rust
println!("\n--- Scenario 25: Memory Stability ---");

// Create and delete 1000 files in a loop
for round in 1..=10 {
    for i in 1..=100 {
        harness.create_file(&format!("temp-{}.txt", i), "content").await?;
    }

    tokio::time::sleep(Duration::from_millis(600)).await;

    for i in 1..=100 {
        harness.delete_file(&format!("temp-{}.txt", i)).await?;
    }

    tokio::time::sleep(Duration::from_millis(600)).await;

    println!("  Round {} complete", round);
}

// Check that buffers were properly cleaned up
if let Some(metrics) = harness.core.services.location_watcher
    .get_location_metrics(harness.location_id)
    .await
{
    // Queue should be empty or near-empty
    assert!(metrics.current_queue_depth() < 100, "Queue should be drained");
}
```

### Tier 8: Failure Recovery

#### Scenario 26: Filesystem Race Conditions
```rust
println!("\n--- Scenario 26: File Disappears During Processing ---");

// Create file
harness.create_file("disappearing.txt", "content").await?;

// Immediately delete it (before watcher processes)
tokio::time::sleep(Duration::from_millis(50)).await;
harness.delete_file("disappearing.txt").await?;

// Wait for processing
tokio::time::sleep(Duration::from_secs(1)).await;

// Should handle gracefully (no crash, no stale entry)
let entries = get_location_entries(&harness.library, harness.location_id).await?;
assert!(!entries.iter().any(|e| e.name == "disappearing"), "Should not have stale entry");
```

#### Scenario 27: Permission Changes
```rust
println!("\n--- Scenario 27: Permission Changes ---");

#[cfg(unix)]
{
    harness.create_file("restricted.txt", "content").await?;
    tokio::time::sleep(Duration::from_millis(600)).await;

    // Make file read-only
    let path = harness.path("restricted.txt");
    let mut perms = tokio::fs::metadata(&path).await?.permissions();
    perms.set_readonly(true);
    tokio::fs::set_permissions(&path, perms).await?;

    tokio::time::sleep(Duration::from_millis(600)).await;

    // Verify file is still indexed (permission change should be detected)
    let entry = harness.verify_entry_exists("restricted").await?;
    // Check if permissions field is updated in database
}
```

## Testing Strategy

### Quick Smoke Test Suite (Run Often)
- Scenarios 1-9: Core functionality
- Run time: ~20 seconds

### Full Functional Test Suite (Run Before Commit)
- Scenarios 1-23: All functionality + edge cases
- Run time: ~2 minutes

### Performance Benchmark Suite (Run Weekly)
- Scenarios 24-25: Performance validation
- Compare metrics against baseline
- Run time: ~5 minutes

### Stress Test Suite (Run Before Release)
- Scenarios 26-27: Failure recovery
- 10,000+ file operations
- Multi-hour soak tests
- Run time: Hours

## Implementation Priority

1. **Immediate**: Scenario 8 (File Deletion) - Currently broken, high priority
2. **Next**: Scenario 9 (Directory Deletion) - Related to #8
3. **Then**: Scenarios 10-11 (Bulk renames, chains) - Validate the rename fix
4. **Later**: Scenarios 12-23 (Edge cases) - Comprehensive coverage
5. **Eventually**: Scenarios 24-27 (Performance, stress) - Quality assurance

## Success Criteria

For the watcher to be considered **production-ready**:

- ✅ All Tier 1-2 scenarios pass (critical operations)
- ✅ 90% of Tier 3 scenarios pass (edge cases)
- ✅ Performance metrics within 20% of v1
- ✅ No memory leaks in 24-hour soak test
- ✅ No crashes or panics under normal load
- ✅ Cross-platform validation (macOS, Linux, Windows)

## Current Score: 7/27 Scenarios Implemented ✅

That's 26% coverage. Let's get to 100%! 🚀

