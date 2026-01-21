---
id: CORE-015
title: "Windows File ID Tracking for Stable File Identity"
status: Done
assignee: jamiepine
priority: High
tags: [core, windows, indexing, platform]
last_updated: 2025-12-29
---

## Description

Implement Windows File ID support in the indexer to enable stable file identification across renames on Windows. This brings Windows to feature parity with Unix/Linux/macOS for change detection and UUID persistence.

**Problem:**
Currently, Windows files don't have stable identifiers across renames because `get_inode()` returns `None`. This means:
- Renamed files are treated as delete + create
- UUIDs are not preserved across renames
- Tags, metadata, and relationships are lost
- Files must be re-indexed and re-hashed unnecessarily

**Solution:**
Use Windows NTFS File IDs (64-bit file index) as the equivalent of Unix inodes for stable file identification.

## Background

### Platform Differences

**Unix/Linux/macOS:**
- Files identified by inode number (stable across renames)
- `std::os::unix::fs::MetadataExt::ino()` provides stable API
- Change detection: inode match + path change = file moved

**Windows (current):**
- Returns `None` for inode → falls back to path-only matching
- Renamed files treated as new files
- UUID and metadata lost on rename

**Windows (with File IDs):**
- NTFS provides 64-bit File ID (similar to inode)
- Stable across renames within a volume
- Enables proper move/rename detection

### What Are Windows File IDs?

Windows NTFS File IDs are unique identifiers exposed via the Win32 API:

```c
typedef struct _BY_HANDLE_FILE_INFORMATION {
    DWORD nFileIndexHigh;  // Upper 32 bits
    DWORD nFileIndexLow;   // Lower 32 bits
    // ... other fields
} BY_HANDLE_FILE_INFORMATION;

// Combined: 64-bit unique identifier
uint64_t file_id = ((uint64_t)nFileIndexHigh << 32) | nFileIndexLow;
```

**Properties:**
- ✅ Unique per file within a volume
- ✅ Stable across file renames
- ✅ Stable across reboots
- ⚠️ Changes when file copied to different volume (expected)
- ⚠️ Not available on FAT32/exFAT
- ⚠️ Theoretically can change during defragmentation (rare)

### Why Currently Disabled

```rust
// core/src/ops/indexing/database_storage.rs:145-152
#[cfg(windows)]
pub fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
    // Windows file indices exist but are unstable across reboots and
    // volume operations, making them unsuitable for change detection.
    None
}
```

**Reasons:**
1. Rust's `std::os::windows::fs::MetadataExt::file_index()` is unstable (requires nightly)
2. Conservative assumption about stability (outdated - File IDs are actually stable)
3. No Windows-specific dependencies currently in codebase

**Reality:**
Modern NTFS File IDs are stable and reliable. The comment is outdated and overly conservative.

## User Impact

### Without File IDs (current behavior)
```
User action: Rename "Project.mp4" → "Final Project.mp4"

Spacedrive sees:
- DELETE: Project.mp4 (UUID: abc-123)
- CREATE: Final Project.mp4 (UUID: def-456) ← New UUID!

Result:
- All tags lost
- All metadata lost
- Relationships broken
- File re-indexed from scratch
- Content re-hashed (expensive for large files)
```

### With File IDs (desired behavior)
```
User action: Rename "Project.mp4" → "Final Project.mp4"

Spacedrive sees:
- MOVE: File ID 0x123ABC from "Project.mp4" to "Final Project.mp4"
- UUID: abc-123 (preserved)

Result:
- Tags preserved
- Metadata intact
- Relationships maintained
- No re-indexing needed
- No re-hashing needed
```

## Acceptance Criteria

### Core Implementation
- [ ] Add `windows-sys` dependency for File ID access
- [ ] Implement `get_inode()` for Windows using `GetFileInformationByHandle`
- [ ] Extract 64-bit File ID from `nFileIndexHigh` and `nFileIndexLow`
- [ ] Return `None` gracefully for non-NTFS filesystems (FAT32, exFAT)
- [ ] Add tracing/logging for File ID extraction success/failure

### Change Detection
- [ ] File renames detected as moves (not delete + create)
- [ ] UUIDs preserved across renames within a volume
- [ ] Tags and metadata preserved across renames
- [ ] Cross-volume copies create new UUIDs (expected behavior)

### Error Handling
- [ ] Handle FAT32/exFAT gracefully (return `None`, fall back to path matching)
- [ ] Handle permission errors (return `None`, log debug message)
- [ ] Handle invalid handles (return `None`, log debug message)
- [ ] No panics or crashes on unsupported filesystems

### Documentation
- [ ] Update code comments to reflect actual File ID stability
- [ ] Document NTFS requirement for File ID support
- [ ] Document known limitations (cross-volume, FAT32, defrag edge case)
- [ ] Add platform comparison table to developer docs

## Implementation Plan

### Option 1: Use `windows-sys` Crate (Recommended)

**Add dependency:**
```toml
# core/Cargo.toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = ["Win32_Storage_FileSystem"] }
```

**Implement File ID extraction:**
```rust
// core/src/ops/indexing/database_storage.rs

#[cfg(windows)]
pub fn get_inode(path: &Path) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION
    };

    // Open file to get handle
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!("Failed to open file for File ID extraction: {}", e);
            return None;
        }
    };

    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };

    unsafe {
        if GetFileInformationByHandle(file.as_raw_handle() as isize, &mut info) != 0 {
            // Combine high and low 32-bit values into 64-bit File ID
            let file_id = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);

            tracing::trace!(
                "Extracted File ID: 0x{:016X} for {:?}",
                file_id,
                path.file_name().unwrap_or_default()
            );

            Some(file_id)
        } else {
            // GetFileInformationByHandle failed
            // Common reasons: FAT32/exFAT filesystem, permission denied
            tracing::debug!(
                "GetFileInformationByHandle failed for {:?} (likely FAT32 or permission issue)",
                path.file_name().unwrap_or_default()
            );
            None
        }
    }
}
```

**Why `windows-sys`:**
- Official Microsoft-maintained bindings
- Minimal overhead (only includes what you use)
- Safe Rust wrappers where possible
- Future-proof and actively maintained

### Option 2: Wait for Rust Stabilization (Not Recommended)

**Track:** https://github.com/rust-lang/rust/issues/63010

```rust
// Would be ideal, but unstable since 2019
#[cfg(windows)]
pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::windows::fs::MetadataExt;
    metadata.file_index()  // ← requires #![feature(windows_by_handle)]
}
```

**Why not recommended:**
- Unstable since 2019, no timeline for stabilization
- Requires nightly Rust
- Blocks production use
- No guarantee it will ever stabilize

## Implementation Files

**Files to modify:**
1. `core/Cargo.toml` - Add `windows-sys` dependency
2. `core/src/ops/indexing/database_storage.rs` - Implement `get_inode()` for Windows
3. `core/src/volume/backend/local.rs` - Implement `get_inode()` for Windows (same code)

**Total changes:** ~30 lines of code across 3 files

## Known Limitations

### 1. Cross-Volume Operations
File IDs are volume-specific. When files are **copied** between volumes:
- Source file keeps original File ID
- Destination file gets new File ID (correct behavior)
- Spacedrive creates new UUID for destination (expected)

### 2. Non-NTFS Filesystems
FAT32 and exFAT don't support File IDs:
- `GetFileInformationByHandle` returns all zeros or fails
- Implementation returns `None`
- Falls back to path-only matching (same as current behavior)

### 3. Defragmentation Edge Case
File IDs can theoretically change during defragmentation:
- Extremely rare with modern NTFS
- If it happens, file treated as delete + create
- Acceptable trade-off for 99.9% reliability

### 4. Hard Links
NTFS supports hard links for files (not directories):
- Multiple paths → same File ID (correct behavior)
- Spacedrive treats as same file with multiple locations (desired)

## Success Metrics

- [ ] File renames preserve UUIDs on Windows NTFS volumes
- [ ] Tags and metadata survive renames on Windows
- [ ] No crashes or errors on FAT32/exFAT volumes
- [ ] File ID extraction success rate > 99% on NTFS
- [ ] No performance regression (File ID extraction is O(1))

## Platform Comparison

| Feature | Unix/Linux | macOS | Windows (current) | Windows (after) |
|---------|-----------|-------|-------------------|-----------------|
| Stable file identity | ✅ inode | ✅ inode | ❌ None | ✅ File ID |
| UUID preserved on rename | ✅ Yes | ✅ Yes | ❌ No | ✅ Yes |
| Tags preserved on rename | ✅ Yes | ✅ Yes | ❌ No | ✅ Yes |
| Implementation | `ino()` | `ino()` | `None` | `GetFileInformationByHandle` |
| Stability | ✅ Stable | ✅ Stable | N/A | ✅ Stable |

## Code Comment Updates

### Old comment (incorrect):
```rust
// Windows file indices exist but are unstable across reboots and
// volume operations, making them unsuitable for change detection.
```

### New comment (accurate):
```rust
// Windows NTFS File IDs provide stable file identification across renames
// and reboots within a volume. They use a 64-bit index similar to Unix inodes.
// File IDs are not available on FAT32/exFAT - we return None and fall back
// to path-based matching in those cases.
```

## References

- **Windows File Information API:**
  https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle

- **NTFS File System Architecture:**
  https://docs.microsoft.com/en-us/windows/win32/fileio/file-management-functions

- **Rust Issue #63010** (file_index unstable):
  https://github.com/rust-lang/rust/issues/63010

- **windows-sys crate:**
  https://crates.io/crates/windows-sys

## Timeline Estimate

- **Implementation:** 2-3 hours (add dependency, write 30 lines of code)
- **Testing:** 1-2 hours (manual testing on NTFS, FAT32, edge cases)
- **Documentation:** 1 hour (update comments, add developer notes)

**Total:** 4-6 hours

## Priority Justification

**Medium Priority** because:
- ✅ System works without it (path-only fallback)
- ⚠️ Significant UX degradation on Windows (lost metadata on rename)
- ⚠️ Windows is a major platform for Spacedrive users
- ⚠️ Competitive gap (competitors handle this correctly)

**Should be elevated to High if:**
- User reports increase about lost tags/metadata on Windows
- Preparing major Windows release
- Windows becomes primary platform
