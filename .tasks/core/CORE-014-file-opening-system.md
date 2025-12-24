---
id: CORE-014
title: Cross-Platform File Opening Backend
status: To Do
assignee: jamiepine
priority: High
tags: [core, platform, file-operations]
whitepaper: DESIGN-open-with.md
last_updated: 2025-12-24
related_tasks: [EXPL-004]
---

## Description

Implement the backend infrastructure for opening files with default and specific applications across macOS, Windows, and Linux. This ports v1's sophisticated platform-specific implementation to v2's architecture.

## Implementation Notes

Create platform-specific crates following v1's proven architecture:
- `apps/tauri/crates/file-opening/` - Shared types and traits
- `apps/tauri/crates/file-opening-macos/` - Swift via FFI using NSWorkspace APIs
- `apps/tauri/crates/file-opening-windows/` - COM Shell APIs (SHAssocEnumHandlers)
- `apps/tauri/crates/file-opening-linux/` - GTK/GIO with content type detection

Each platform implementation must:
1. Query OS for applications that can open a file
2. Return intersection of compatible apps for multi-file selection
3. Open file with default application
4. Open file(s) with specific application

See `DESIGN-open-with.md` for complete architecture details.

## Acceptance Criteria

- [ ] Shared `file-opening` crate with `FileOpener` trait and types
- [ ] macOS implementation using Swift FFI + NSWorkspace
  - [ ] Query apps using `urlsForApplications(toOpen:)` API
  - [ ] Filter to `/Applications/` directory
  - [ ] Open with default via NSWorkspace
  - [ ] Open with specific app by bundle ID
- [ ] Windows implementation using COM Shell APIs
  - [ ] Query apps using `SHAssocEnumHandlers`
  - [ ] Thread-local COM initialization
  - [ ] Open with default via ShellExecute
  - [ ] Open with specific app via IAssocHandler
- [ ] Linux implementation using GTK/GIO
  - [ ] Content type detection from file magic bytes
  - [ ] Query apps via `AppInfo::recommended_for_type`
  - [ ] Open with default via `launch_default_for_uri`
  - [ ] Open with specific app via DesktopAppInfo
- [ ] Tauri commands registered:
  - [ ] `get_apps_for_paths(paths)` - returns Vec<OpenWithApp>
  - [ ] `open_path_default(path)` - returns OpenResult
  - [ ] `open_path_with_app(path, app_id)` - returns OpenResult
  - [ ] `open_paths_with_app(paths, app_id)` - returns Vec<OpenResult>
- [ ] Intersection logic for multi-file selections works correctly
- [ ] Error handling returns proper OpenResult variants
- [ ] All commands are async and non-blocking

## Implementation Files

To be created:
- `apps/tauri/crates/file-opening/src/lib.rs`
- `apps/tauri/crates/file-opening/src/types.rs`
- `apps/tauri/crates/file-opening-macos/src/lib.rs`
- `apps/tauri/crates/file-opening-macos/src-swift/FileOpening.swift`
- `apps/tauri/crates/file-opening-windows/src/lib.rs`
- `apps/tauri/crates/file-opening-linux/src/lib.rs`
- `apps/tauri/src-tauri/src/commands/file_opening.rs`

To be modified:
- `apps/tauri/src-tauri/src/main.rs` (register commands and service)
- `apps/tauri/src-tauri/Cargo.toml` (add dependencies)

## Reference Implementation

v1 implementation can be found at:
- `~/Projects/spacedrive_v1/apps/desktop/src-tauri/src/file.rs`
- `~/Projects/spacedrive_v1/apps/desktop/crates/macos/src-swift/files.swift`
- `~/Projects/spacedrive_v1/apps/desktop/crates/windows/src/lib.rs`
- `~/Projects/spacedrive_v1/apps/desktop/crates/linux/src/app_info.rs`

## Testing

- Unit tests for intersection logic
- Platform-specific tests with mock file system
- Manual testing on macOS, Windows, Linux
- Test edge cases: no apps available, permission denied, file not found
