# sd-fs-watcher

Platform-agnostic filesystem watcher for Spacedrive.

## Overview

`sd-fs-watcher` provides a clean, storage-agnostic interface for watching filesystem changes. It handles platform-specific quirks (like macOS rename detection) internally and emits normalized events.

This crate is designed to be the foundation of Spacedrive's filesystem event system, but it has no knowledge of:

- Databases or ORM entities
- Libraries or locations
- UUIDs or entry IDs

It just watches paths and emits events.

## Usage

```rust
use sd_fs_watcher::{FsWatcher, WatchConfig, WatcherConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create watcher with default config
    let watcher = FsWatcher::new(WatcherConfig::default());
    watcher.start().await?;

    // Subscribe to events
    let mut rx = watcher.subscribe();

    // Watch a directory recursively
    let _handle = watcher.watch("/path/to/watch", WatchConfig::recursive()).await?;

    // Process events
    while let Ok(event) = rx.recv().await {
        match event.kind {
            sd_fs_watcher::FsEventKind::Create => {
                println!("Created: {}", event.path.display());
            }
            sd_fs_watcher::FsEventKind::Modify => {
                println!("Modified: {}", event.path.display());
            }
            sd_fs_watcher::FsEventKind::Remove => {
                println!("Removed: {}", event.path.display());
            }
            sd_fs_watcher::FsEventKind::Rename { from, to } => {
                println!("Renamed: {} -> {}", from.display(), to.display());
            }
        }
    }

    Ok(())
}
```

## Watch Modes

### Recursive (default)

Watch a directory and all its subdirectories:

```rust
let _handle = watcher.watch("/path", WatchConfig::recursive()).await?;
```

### Shallow

Watch only immediate children of a directory (for ephemeral browsing):

```rust
let _handle = watcher.watch("/path", WatchConfig::shallow()).await?;
```

## Event Filtering

By default, the watcher filters out:

- Temporary files (`.tmp`, `.temp`, `~`, `.swp`)
- System files (`.DS_Store`, `Thumbs.db`)
- Hidden files (starting with `.`)

Important dotfiles like `.gitignore`, `.env`, etc. are preserved.

```rust
// Custom filtering
let config = WatchConfig::recursive()
    .with_filters(EventFilters {
        skip_hidden: false,  // Include hidden files
        skip_system_files: true,
        skip_temp_files: true,
        skip_patterns: vec!["node_modules".to_string()],
        important_dotfiles: vec![".env".to_string()],
    });
```

## Platform-Specific Behavior

### macOS

macOS FSEvents doesn't provide native rename tracking. When a file is renamed, we receive separate create and delete events. This crate implements rename detection via inode tracking:

1. When a file is created, we record its inode
2. When a file is removed, we buffer it briefly
3. If a create with the same inode arrives within 500ms, we emit a rename event
4. Otherwise, we emit separate create/remove events

### Linux

Linux inotify provides better rename tracking. We handle rename events directly when both paths are provided, with a small stabilization buffer for modify events.

### Windows

Windows ReadDirectoryChangesW provides reasonable tracking. We implement rename detection by buffering remove events and matching with subsequent creates.

## Reference Counting

Multiple calls to `watch()` on the same path share resources:

```rust
let handle1 = watcher.watch("/path", WatchConfig::recursive()).await?;
let handle2 = watcher.watch("/path", WatchConfig::recursive()).await?;

// Only one actual watch is registered with the OS
// Dropping both handles will unwatch
drop(handle1);
// Still watching (handle2 exists)

drop(handle2);
// Now actually unwatched
```

## Metrics

```rust
let received = watcher.events_received();  // Raw events from notify
let emitted = watcher.events_emitted();    // Processed events broadcast
```

## Event Metadata

Each `FsEvent` includes an optional `is_directory` flag:

```rust
pub struct FsEvent {
    pub path: PathBuf,
    pub kind: FsEventKind,
    pub timestamp: SystemTime,
    pub is_directory: Option<bool>,  // Avoids extra fs::metadata calls downstream
}
```

Check directory status without filesystem calls:

```rust
if let Some(true) = event.is_dir() {
    // Handle directory event
} else if let Some(false) = event.is_file() {
    // Handle file event
} else {
    // Unknown - check filesystem if needed (e.g., for Remove events)
}
```

## Integration with Spacedrive

This crate is designed to be consumed by higher-level services:

- **PersistentIndexService**: Subscribes to events, filters by location scope, writes to database
- **EphemeralIndexService**: Subscribes to events, filters by session scope, writes to memory

These services are not part of this crate - they live in `sd-core` and consume events from `FsWatcher`.

### Backpressure Management

The `FsWatcher` uses a broadcast channel for event distribution. To avoid backpressure issues:

1. **Don't block in the receiver loop**: Avoid synchronous database writes directly in the broadcast receiver
2. **Use internal batching queues**: The `PersistentIndexService` should receive events and immediately push them to its own internal batching queue (like the existing `LocationWorker` logic)
3. **Keep the broadcast clear**: This ensures the `EphemeralIndexService` (UI updates) receives events promptly

```rust
// Good pattern for PersistentIndexService
let mut rx = watcher.subscribe();
let (batch_tx, batch_rx) = mpsc::channel(100_000);

// Receiver task - fast, non-blocking
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        if is_in_my_scope(&event) {
            let _ = batch_tx.send(event).await;  // Push to internal queue
        }
    }
});

// Worker task - handles batching and DB writes
tokio::spawn(async move {
    // Batch events, coalesce, write to DB...
});
```

### Database-Backed Inode Lookup

For enhanced rename detection on macOS, the `PersistentIndexService` can maintain an inode cache. When a Remove event is received, check if the inode exists in your database to detect if it's actually a rename where the "new path" hasn't arrived yet.

