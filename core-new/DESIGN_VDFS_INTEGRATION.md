# Virtual Distributed File System - Integration Design

## How SdPath and the New Data Model Enable True VDFS

The combination of SdPath and the decoupled data model creates a powerful foundation for Spacedrive's Virtual Distributed File System.

## Core Concepts Working Together

### 1. SdPath: Universal File Addressing
```rust
// Any file, anywhere in your library
let photo = SdPath::new(macbook_id, "/Users/jamie/Photos/sunset.jpg");
let backup = SdPath::new(nas_id, "/backups/photos/sunset.jpg");
```

### 2. Entry: Universal File Representation
```rust
// Both files are Entries with full metadata support
let photo_entry = Entry {
    sd_path: photo.serialize(),
    metadata_id: uuid_1,  // Has tags, notes, etc.
    content_id: Some(content_uuid),  // Same content!
};

let backup_entry = Entry {
    sd_path: backup.serialize(),
    metadata_id: uuid_2,  // Different metadata possible
    content_id: Some(content_uuid),  // Recognized as duplicate
};
```

## Key Scenarios

### Scenario 1: Cross-Device File Management
```rust
// Tag a file on your iPhone from your MacBook
let iphone_file = SdPath::new(iphone_id, "/DCIM/IMG_1234.jpg");
tag_file(iphone_file, "Vacation").await?;
// Works even if the file isn't indexed yet!

// Copy tagged files from multiple devices to NAS
let tagged_files = find_files_with_tag("Vacation").await?;
// Returns Entries from ALL devices

for entry in tagged_files {
    copy_file(entry.sd_path, nas_backup_folder).await?;
    // Preserves tags during copy!
}
```

### Scenario 2: Smart Deduplication
```rust
// Find all copies of a file across devices
let content = get_content_by_cas_id("v2:a1b2c3...").await?;
let all_copies = get_entries_with_content(content.id).await?;

println!("You have {} copies of this file:", all_copies.len());
for entry in all_copies {
    println!("- {} on {}", entry.sd_path.path, entry.sd_path.device_name());
}
// Output:
// You have 3 copies of this file:
// - /Users/jamie/sunset.jpg on MacBook Pro
// - /DCIM/sunset.jpg on iPhone
// - /backups/sunset.jpg on NAS
```

### Scenario 3: Ephemeral File Support
```rust
// Browse and tag files on a USB drive without indexing
let usb_file = SdPath::local("/Volumes/USB/document.pdf");
let entry = discover_entry(usb_file).await?;  // Quick, no content reading
tag_entry(entry, "Review Later").await?;      // Instant tagging!

// Later, even after USB is disconnected
let to_review = find_entries_with_tag("Review Later").await?;
// Shows the file with its USB path, can re-connect to access
```

### Scenario 4: Content Change Handling
```rust
// Edit a tagged document
let doc = SdPath::local("/Documents/report.docx");
edit_document(doc).await?;

// Content changed, but metadata persists
let entry = get_entry_by_sdpath(doc).await?;
assert!(entry.metadata.tags.contains("Important"));  // Tags still there!

// Old content identity updated automatically
// Deduplication still works for the new version
```

## Implementation Benefits

### 1. Unified API Surface
```rust
// These all use the same internal logic
copy_files(local_to_local).await?;
copy_files(local_to_remote).await?;
copy_files(remote_to_local).await?;
copy_files(remote_to_remote).await?;

// Frontend doesn't care about device boundaries
mutation CopyFiles($sources: [SdPath!]!, $destination: SdPath!) {
    copyFiles(sources: $sources, destination: $destination) {
        successful
    }
}
```

### 2. Progressive Enhancement
```rust
// Level 1: Quick discovery (milliseconds)
let entry = discover_entry(sd_path).await?;
tag_entry(entry, "Important").await?;

// Level 2: Content identity (seconds, async)
let content_id = index_entry_content(entry, IndexMode::Content).await?;
// Now have deduplication

// Level 3: Deep indexing (minutes, background)
let full_index = deep_index_entry(entry).await?;
// Now have full-text search, thumbnails, etc.
```

### 3. Offline Resilience
```rust
// Tag files on a device that's offline
let offline_file = SdPath::new(laptop_id, "/Documents/todo.txt");
// This creates a "virtual entry" locally
tag_virtual_entry(offline_file, "Urgent").await?;

// When laptop comes online, tag syncs automatically
on_device_connected(laptop_id, |device| {
    sync_virtual_entries(device).await?;
});
```

## Database Queries Enabled

### Find files across all devices
```sql
-- All PDFs tagged "Important" regardless of device
SELECT e.*, sd.device_name, um.tags
FROM entry e
JOIN user_metadata um ON e.metadata_id = um.id
JOIN spacedrive_devices sd ON e.device_id = sd.id
WHERE e.name LIKE '%.pdf'
  AND 'Important' = ANY(um.tags);
```

### Smart backup detection
```sql
-- Find files that exist on laptop but not on backup drive
SELECT e1.*
FROM entry e1
WHERE e1.device_id = ?  -- laptop_id
  AND NOT EXISTS (
    SELECT 1 FROM entry e2
    WHERE e2.device_id = ?  -- backup_id
      AND e2.content_id = e1.content_id
  );
```

### Cross-device duplicate cleanup
```sql
-- Find duplicate files across devices, keep favorited ones
WITH duplicates AS (
    SELECT content_id, COUNT(*) as copies
    FROM entry
    WHERE content_id IS NOT NULL
    GROUP BY content_id
    HAVING COUNT(*) > 1
)
SELECT e.*, um.favorite, ci.total_size
FROM entry e
JOIN user_metadata um ON e.metadata_id = um.id
JOIN content_identity ci ON e.content_id = ci.id
JOIN duplicates d ON e.content_id = d.content_id
ORDER BY um.favorite DESC, e.created_at ASC;
```

## Future Possibilities

### 1. Global File Search
```rust
// Search across all devices from any device
let results = search_global("sunset photo").await?;
// Returns Entries from MacBook, iPhone, NAS, cloud, etc.
```

### 2. Smart Sync Policies
```rust
// Define rules for automatic file distribution
create_sync_rule(
    "Backup photos",
    When::FileMatchesPattern("*.jpg"),
    When::TaggedWith("Important"),
    Action::CopyTo(nas_device),
).await?;
```

### 3. Virtual Folders
```rust
// Create a folder that aggregates files from multiple devices
let virtual_folder = VirtualFolder::new("All Documents")
    .include(SdPath::new(laptop_id, "/Documents"))
    .include(SdPath::new(desktop_id, "/home/user/Documents"))
    .include(SdPath::new(cloud_id, "/Documents"))
    .with_filter(|entry| entry.name.ends_with(".pdf"));
```

## Conclusion

The combination of:
1. **SdPath** for universal file addressing
2. **Decoupled data model** for flexible metadata
3. **Progressive indexing** for performance
4. **Content identity** for deduplication

Creates a system where device boundaries disappear and files become truly virtual - accessible, manageable, and searchable regardless of their physical location. This is the true promise of a Virtual Distributed File System.