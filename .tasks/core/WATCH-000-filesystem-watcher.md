---
id: WATCH-000
title: "Epic: Filesystem Watcher Foundation"
status: Done
assignee: jamiepine
priority: High
tags: [epic, core, watcher, filesystem]
last_updated: 2025-12-16
---

## Description

The `sd-fs-watcher` crate provides a platform-agnostic filesystem watcher that serves as the foundation for Spacedrive's real-time file monitoring. It handles platform-specific quirks internally and emits normalized events to higher-level services.

## Architecture

The watcher is designed to be storage-agnostic - it has no knowledge of databases, libraries, or locations. It just watches paths and emits events.

**Key Components**:

- **FsWatcher**: Main watcher interface with start/stop lifecycle
- **FsEvent**: Normalized event type (Create, Modify, Remove, Rename)
- **WatchConfig**: Per-path configuration (recursive vs shallow, filters)
- **Platform Implementations**: macOS (FSEvents), Linux (inotify), Windows (ReadDirectoryChangesW)
- **Rename Detection**: Inode tracking for platforms that don't provide native rename events

## Features

- **Reference Counting**: Multiple watches on same path share OS resources
- **Event Filtering**: Skip temp files, system files, hidden files (configurable)
- **Metrics**: Track events received vs events emitted
- **Backpressure Management**: Broadcast channel for multiple consumers
- **Watch Modes**: Recursive (full tree) and Shallow (immediate children only)

## Integration with Spacedrive

The watcher is consumed by higher-level services in `sd-core`:

- **PersistentIndexService**: Subscribes to events, writes to database via ChangeHandler
- **EphemeralIndexService**: Subscribes to events, updates in-memory index

These services filter events by scope and route to appropriate storage adapters.

## Implementation Files

- `crates/fs-watcher/src/lib.rs` - Public API
- `crates/fs-watcher/src/watcher.rs` - Core FsWatcher implementation
- `crates/fs-watcher/src/event.rs` - FsEvent types
- `crates/fs-watcher/src/config.rs` - WatchConfig and filters
- `crates/fs-watcher/src/error.rs` - Error types
- `crates/fs-watcher/src/platform/` - Platform-specific implementations

## Related Tasks

- WATCH-001 - Platform-Agnostic Event System
- WATCH-002 - Platform-Specific Rename Detection
- INDEX-004 - Change Detection System (consumes watcher events)
