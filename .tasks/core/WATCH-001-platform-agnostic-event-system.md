---
id: WATCH-001
title: Platform-Agnostic Event System
status: Done
assignee: jamiepine
parent: WATCH-000
priority: High
tags: [watcher, events, api]
last_updated: 2025-12-16
---

## Description

Implement the platform-agnostic event system that normalizes filesystem events across macOS, Linux, and Windows. The system provides a clean API for watching paths and receiving events via broadcast channels, with reference counting for shared watches.

## Architecture

### FsWatcher

Main watcher interface with lifecycle management:

```rust
pub struct FsWatcher {
    // Notify backend (platform-specific)
    watcher: Arc<Mutex<RecommendedWatcher>>,
    // Watched paths with reference counts
    watches: Arc<RwLock<HashMap<PathBuf, WatchState>>>,
    // Broadcast channel for events
    event_tx: broadcast::Sender<FsEvent>,
    // Metrics
    events_received: AtomicU64,
    events_emitted: AtomicU64,
}

impl FsWatcher {
    pub fn new(config: WatcherConfig) -> Self;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;
    pub async fn watch(&self, path: impl AsRef<Path>, config: WatchConfig) -> Result<WatchHandle>;
    pub fn subscribe(&self) -> broadcast::Receiver<FsEvent>;
    pub fn events_received(&self) -> u64;
    pub fn events_emitted(&self) -> u64;
}
```

### FsEvent

Normalized event type emitted to consumers:

```rust
pub struct FsEvent {
    pub path: PathBuf,
    pub kind: FsEventKind,
    pub timestamp: SystemTime,
    pub is_directory: Option<bool>,  // Avoids extra metadata calls
}

pub enum FsEventKind {
    Create,
    Modify,
    Remove,
    Rename { from: PathBuf, to: PathBuf },
}

impl FsEvent {
    pub fn is_dir(&self) -> Option<bool>;
    pub fn is_file(&self) -> Option<bool>;
}
```

### WatchConfig

Per-path watch configuration:

```rust
pub struct WatchConfig {
    pub recursive: bool,  // Recursive vs shallow
    pub filters: EventFilters,
}

pub struct EventFilters {
    pub skip_hidden: bool,
    pub skip_system_files: bool,
    pub skip_temp_files: bool,
    pub skip_patterns: Vec<String>,  // Custom patterns (e.g., "node_modules")
    pub important_dotfiles: Vec<String>,  // Preserve important dotfiles
}

impl WatchConfig {
    pub fn recursive() -> Self;  // Default recursive watch
    pub fn shallow() -> Self;    // Shallow watch (for ephemeral browsing)
    pub fn with_filters(self, filters: EventFilters) -> Self;
}
```

### Reference Counting

Multiple watches on the same path share OS resources:

```rust
struct WatchState {
    refcount: usize,
    config: WatchConfig,
    handle: WatchHandle,
}

// When watch() is called:
// 1. Check if path already watched
// 2. If yes, increment refcount
// 3. If no, register with OS watcher
// 4. Return handle that decrements on drop
```

**Benefits**:
- Only one OS watch per path regardless of consumers
- Automatic cleanup when all handles dropped
- Efficient resource usage

## Event Filtering

Default filters skip noise:

```rust
fn should_emit_event(event: &FsEvent, filters: &EventFilters) -> bool {
    let path = &event.path;
    let name = path.file_name()?.to_str()?;

    // Skip temp files
    if filters.skip_temp_files {
        if name.ends_with(".tmp") || name.ends_with(".temp")
           || name.starts_with("~") || name.ends_with(".swp") {
            return false;
        }
    }

    // Skip system files
    if filters.skip_system_files {
        if name == ".DS_Store" || name == "Thumbs.db" || name == "desktop.ini" {
            return false;
        }
    }

    // Skip hidden files (except important dotfiles)
    if filters.skip_hidden && name.starts_with(".") {
        if !filters.important_dotfiles.contains(&name.to_string()) {
            return false;
        }
    }

    // Skip custom patterns
    for pattern in &filters.skip_patterns {
        if name == pattern {
            return false;
        }
    }

    true
}
```

## Backpressure Management

The watcher uses broadcast channels for multiple consumers:

```rust
// Watcher broadcasts events
let (event_tx, _) = broadcast::channel(10_000);

// Each consumer gets its own receiver
let rx1 = watcher.subscribe();  // PersistentIndexService
let rx2 = watcher.subscribe();  // EphemeralIndexService
```

**Important**: Consumers should NOT block in the receiver loop. Use internal batching queues:

```rust
// Good pattern for PersistentIndexService
let mut rx = watcher.subscribe();
let (batch_tx, batch_rx) = mpsc::channel(100_000);

// Fast, non-blocking receiver
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        if is_in_my_scope(&event) {
            let _ = batch_tx.send(event).await;
        }
    }
});

// Worker handles batching and DB writes
tokio::spawn(async move {
    // Batch events, coalesce, write to DB...
});
```

## Implementation Files

- `crates/fs-watcher/src/lib.rs` - Public API exports
- `crates/fs-watcher/src/watcher.rs` - FsWatcher implementation
- `crates/fs-watcher/src/event.rs` - FsEvent and FsEventKind
- `crates/fs-watcher/src/config.rs` - WatchConfig and EventFilters
- `crates/fs-watcher/src/error.rs` - WatcherError types

## Acceptance Criteria

- [x] FsWatcher can be created with WatcherConfig
- [x] start() initializes the watcher
- [x] stop() cleanly shuts down the watcher
- [x] watch() registers a path and returns WatchHandle
- [x] Multiple watch() calls on same path share OS resources (reference counting)
- [x] Dropping WatchHandle decrements refcount
- [x] Dropping last handle unwatches the path
- [x] subscribe() returns broadcast receiver for events
- [x] Events include normalized FsEventKind (Create/Modify/Remove/Rename)
- [x] Events include timestamp and optional is_directory flag
- [x] Recursive vs shallow watch modes work
- [x] Event filtering skips temp files, system files, hidden files
- [x] Important dotfiles are preserved (.gitignore, .env)
- [x] Custom skip patterns work (e.g., "node_modules")
- [x] Metrics track events_received and events_emitted
- [x] Broadcast channel handles multiple concurrent consumers

## Usage Example

```rust
use sd_fs_watcher::{FsWatcher, WatchConfig, WatcherConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Create and start watcher
    let watcher = FsWatcher::new(WatcherConfig::default());
    watcher.start().await?;

    // Subscribe to events
    let mut rx = watcher.subscribe();

    // Watch directory recursively
    let _handle = watcher.watch("/path/to/watch", WatchConfig::recursive()).await?;

    // Process events
    while let Ok(event) = rx.recv().await {
        match event.kind {
            FsEventKind::Create => println!("Created: {:?}", event.path),
            FsEventKind::Modify => println!("Modified: {:?}", event.path),
            FsEventKind::Remove => println!("Removed: {:?}", event.path),
            FsEventKind::Rename { from, to } => {
                println!("Renamed: {:?} -> {:?}", from, to);
            }
        }
    }

    Ok(())
}
```

## Testing

### Unit Tests

Located in `crates/fs-watcher/src/`:
- `test_reference_counting` - Verify watch refcounts
- `test_event_filtering` - Verify filters work
- `test_recursive_vs_shallow` - Verify watch modes
- `test_broadcast_multiple_consumers` - Verify multiple receivers work

### Integration Tests

Located in `crates/fs-watcher/tests/`:
- `test_create_event` - Verify create events emitted
- `test_modify_event` - Verify modify events emitted
- `test_remove_event` - Verify remove events emitted
- `test_rename_event` - Verify rename detection (platform-specific)

## Related Tasks

- WATCH-000 - Filesystem Watcher Epic
- WATCH-002 - Platform-Specific Rename Detection
- INDEX-004 - Change Detection System (consumes these events)
