---
id: INDEX-012
title: Ephemeral Cache Parent Path Deduplication
status: To Do
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, ephemeral, cache, deduplication, bug, macos]
last_updated: 2026-02-07
related_tasks: [INDEX-001, INDEX-010]
---

## Description

When a volume is indexed ephemerally (recursive scan), browsing a subdirectory within that volume triggers a redundant ephemeral scan. This creates duplicate registrations in `indexed_paths` and wastes work. The primary cause is a **symlink mismatch on macOS** combined with exact-match path lookups.

## Observed Behavior

```
$ sd index ephemeral-cache

INDEXED PATHS            Children
○ /System/Volumes/Data   11
○ /Users/alice       111
```

User indexed their system volume (registered as `/System/Volumes/Data`), then browsed `/Users/alice` in the Explorer. The cache created a second entry with 111 children from a redundant shallow scan, despite the volume index already containing all 1.8M entries recursively.

## Root Cause: macOS Symlink + Exact Path Lookup

On macOS with APFS, `/Users` is a symlink to `/System/Volumes/Data/Users`. The volume indexer walks from the real mount point `/System/Volumes/Data`, so the arena stores all paths under `/System/Volumes/Data/Users/alice/...`.

When the Explorer browses `/Users/alice`, the chain of events:

1. **`get_for_search("/Users/alice")`** (directory_listing.rs:649) — canonicalizes the path to `/System/Volumes/Data/Users/alice`, finds it starts with the indexed root `/System/Volumes/Data` → returns the index ✅

2. **`list_directory("/Users/alice")`** (directory_listing.rs:659) — does a raw `path_index.get(path)` lookup. The arena has `/System/Volumes/Data/Users/alice` as the key, not `/Users/alice` → returns `None` ❌

3. **Fallthrough** (directory_listing.rs:736+) — concludes the path isn't indexed, calls `create_for_indexing("/Users/alice")`, triggers a redundant shallow scan, registers `/Users/alice` as a second entry in `indexed_paths`

The fix requires addressing both the symlink resolution and the parent-path awareness.

## Implementation

### 1. Path Canonicalization in EphemeralIndex Lookups

The core issue is that `list_directory`, `get_entry_ref`, and other `EphemeralIndex` methods use raw path lookups against `path_index` without resolving symlinks. When the arena was populated via a real path (`/System/Volumes/Data/...`) and the query uses a symlink path (`/Users/...`), the lookup fails.

```rust
// core/src/ops/indexing/ephemeral/index.rs

impl EphemeralIndex {
    /// Resolve a path to its canonical form if it exists in the arena.
    /// Tries the raw path first (fast path), then canonicalizes (slow path).
    fn resolve_path<'a>(&'a self, path: &Path) -> Option<&'a PathBuf> {
        // Fast path: direct lookup
        if self.path_index.contains_key(path) {
            return Some(
                self.path_index.keys().find(|k| k.as_path() == path).unwrap()
            );
        }

        // Slow path: canonicalize and retry
        if let Ok(canonical) = path.canonicalize() {
            if self.path_index.contains_key(&canonical) {
                return Some(
                    self.path_index.keys().find(|k| **k == canonical).unwrap()
                );
            }
        }

        None
    }

    /// Updated list_directory with symlink resolution
    pub fn list_directory(&self, path: &Path) -> Option<Vec<PathBuf>> {
        // Try direct lookup first, then canonical
        let lookup_path = if self.path_index.contains_key(path) {
            path.to_path_buf()
        } else if let Ok(canonical) = path.canonicalize() {
            if self.path_index.contains_key(&canonical) {
                canonical
            } else {
                return None;
            }
        } else {
            return None;
        };

        let id = self.path_index.get(&lookup_path)?;
        let node = self.arena.get(*id)?;

        Some(
            node.children
                .iter()
                .filter_map(|&child_id| self.reconstruct_path(child_id))
                .collect(),
        )
    }
}
```

The same pattern applies to `get_entry_ref`, `get_or_assign_uuid`, and `get_entry_uuid`. Rather than duplicating the canonicalization in every method, a single internal `resolve_to_entry_id` helper handles it:

```rust
/// Internal: resolve a path to its EntryId, handling symlinks.
fn resolve_entry_id(&self, path: &Path) -> Option<EntryId> {
    // Fast path
    if let Some(&id) = self.path_index.get(path) {
        return Some(id);
    }

    // Canonicalize and retry
    path.canonicalize()
        .ok()
        .and_then(|canonical| self.path_index.get(&canonical).copied())
}
```

Then all public methods use `resolve_entry_id` instead of `self.path_index.get(path)`.

### 2. Parent-Aware `is_indexed` and `get_for_path`

Even after fixing symlink resolution, `is_indexed()` and `get_for_path()` should check parent paths. This handles the non-symlink case (e.g., Linux or paths without symlinks) where `/mnt/nas` is indexed and `/mnt/nas/photos/2024` should be considered covered.

```rust
// core/src/ops/indexing/ephemeral/cache.rs

pub fn is_indexed(&self, path: &Path) -> bool {
    let indexed = self.indexed_paths.read();

    if indexed.contains_key(path) {
        return true;
    }

    // Check canonical form
    let canonical = path.canonicalize().ok();
    if let Some(ref canon) = canonical {
        if indexed.contains_key(canon.as_path()) {
            return true;
        }
    }

    // Check if any recursively-indexed parent covers this path
    for (indexed_path, scope) in indexed.iter() {
        if *scope != IndexScope::Recursive {
            continue;
        }

        if path.starts_with(indexed_path) {
            return true;
        }

        if let Some(ref canon) = canonical {
            if canon.starts_with(indexed_path) {
                return true;
            }
        }
    }

    false
}
```

Apply the same pattern to `get_for_path`.

### 3. Track Scope Per Indexed Path

Replace `HashSet<PathBuf>` with `HashMap<PathBuf, IndexScope>` to distinguish shallow browses from recursive scans. Parent coverage only applies to recursive scans.

```rust
indexed_paths: RwLock<HashMap<PathBuf, IndexScope>>,
```

Callers that register paths need to provide the scope. The scope is available at every callsite from `IndexerJobConfig.scope`:

- `mark_indexing_complete` takes scope parameter
- `create_for_indexing` takes scope parameter
- Volume indexing passes `Recursive`
- Directory browsing passes `Current`

### 4. Guard Against Redundant Scans

```rust
pub fn create_for_indexing(
    &self,
    path: PathBuf,
    scope: IndexScope,
) -> Arc<TokioRwLock<EphemeralIndex>> {
    let in_progress = self.indexing_in_progress.read();
    let indexed = self.indexed_paths.read();

    // Check if already covered by a recursive parent (with symlink resolution)
    let canonical = path.canonicalize().ok();
    for (existing_path, existing_scope) in indexed.iter() {
        if *existing_scope != IndexScope::Recursive {
            continue;
        }

        let covered = path.starts_with(existing_path)
            || canonical.as_ref().map_or(false, |c| c.starts_with(existing_path));

        if covered {
            tracing::debug!(
                "Path {} already covered by recursive index at {}, skipping",
                path.display(),
                existing_path.display()
            );
            return self.index.clone();
        }
    }

    drop(indexed);
    drop(in_progress);

    let mut in_progress = self.indexing_in_progress.write();
    let mut indexed = self.indexed_paths.write();

    indexed.remove(&path);
    in_progress.insert(path);

    self.index.clone()
}
```

### 5. Subsume Child Paths on Parent Registration

When a volume index completes, remove individually-browsed child paths that are now redundant.

```rust
pub fn mark_indexing_complete(&self, path: &Path, scope: IndexScope) {
    let mut in_progress = self.indexing_in_progress.write();
    let mut indexed = self.indexed_paths.write();

    in_progress.remove(path);

    // If this is a recursive scan, subsume child paths
    if scope == IndexScope::Recursive {
        indexed.retain(|existing, _| {
            !existing.starts_with(path) || existing == path
        });
    }

    indexed.insert(path.to_path_buf(), scope);
}
```

## Files to Modify

- `core/src/ops/indexing/ephemeral/index.rs` - Add `resolve_entry_id()`, update `list_directory()`, `get_entry_ref()`, `get_or_assign_uuid()`, `get_entry_uuid()` to use symlink-aware lookup
- `core/src/ops/indexing/ephemeral/cache.rs` - Change `indexed_paths` to `HashMap<PathBuf, IndexScope>`, update `is_indexed()`, `get_for_path()`, `create_for_indexing()`, `mark_indexing_complete()` with scope tracking and canonicalization
- `core/src/ops/indexing/job.rs` - Pass scope to `mark_indexing_complete()`
- `core/src/ops/volumes/index/action.rs` - Pass scope to `create_for_indexing()`
- `core/src/ops/files/query/directory_listing.rs` - Pass scope to `create_for_indexing()`

## Acceptance Criteria

- [ ] `list_directory("/Users/alice")` returns children when arena has `/System/Volumes/Data/Users/alice`
- [ ] `is_indexed()` returns true for symlink paths under a recursively-indexed volume
- [ ] `get_for_path()` returns the index for symlink paths under a recursively-indexed volume
- [ ] `create_for_indexing()` is a no-op when the path is covered by a recursive parent (including symlinks)
- [ ] `mark_indexing_complete()` with Recursive scope subsumes child paths
- [ ] Only recursive scans provide parent coverage (shallow browses don't)
- [ ] `indexed_paths` stores scope per path
- [ ] No redundant ephemeral scan triggered when browsing under a volume-indexed path on macOS
- [ ] Volume index + browse child shows single entry in `sd index ephemeral-cache`
- [ ] Existing tests updated, new tests for symlink resolution and parent coverage

## Tests

```rust
#[test]
fn test_symlink_path_resolution() {
    let cache = EphemeralIndexCache::new().expect("failed to create cache");

    // Simulate volume index at real path
    let real_root = PathBuf::from("/System/Volumes/Data");
    let _index = cache.create_for_indexing(real_root.clone(), IndexScope::Recursive);
    cache.mark_indexing_complete(&real_root, IndexScope::Recursive);

    // Symlink path should be considered indexed (on macOS /Users -> /System/Volumes/Data/Users)
    // This test verifies the canonicalization logic
    let symlink_path = PathBuf::from("/Users/alice");
    assert!(cache.is_indexed(&symlink_path)); // canonicalizes to /System/Volumes/Data/Users/alice
}

#[test]
fn test_parent_path_coverage() {
    let cache = EphemeralIndexCache::new().expect("failed to create cache");

    let root = PathBuf::from("/mnt/volume");
    let _index = cache.create_for_indexing(root.clone(), IndexScope::Recursive);
    cache.mark_indexing_complete(&root, IndexScope::Recursive);

    // Child path should be considered indexed
    assert!(cache.is_indexed(&PathBuf::from("/mnt/volume/photos/2024")));
    assert!(cache.get_for_path(&PathBuf::from("/mnt/volume/photos/2024")).is_some());
}

#[test]
fn test_shallow_browse_no_parent_coverage() {
    let cache = EphemeralIndexCache::new().expect("failed to create cache");

    // Shallow browse of root (Current scope)
    let root = PathBuf::from("/mnt/volume");
    let _index = cache.create_for_indexing(root.clone(), IndexScope::Current);
    cache.mark_indexing_complete(&root, IndexScope::Current);

    // Child path should NOT be covered by a shallow scan
    assert!(!cache.is_indexed(&PathBuf::from("/mnt/volume/photos/2024")));
}

#[test]
fn test_no_redundant_scan_under_volume() {
    let cache = EphemeralIndexCache::new().expect("failed to create cache");

    let root = PathBuf::from("/mnt/volume");
    let _index = cache.create_for_indexing(root.clone(), IndexScope::Recursive);
    cache.mark_indexing_complete(&root, IndexScope::Recursive);

    // Attempting to create_for_indexing on a child should be a no-op
    let child = PathBuf::from("/mnt/volume/photos");
    let _index = cache.create_for_indexing(child.clone(), IndexScope::Current);

    // indexed_paths should still only contain the root
    assert_eq!(cache.len(), 1);
}

#[test]
fn test_volume_subsumes_child_paths() {
    let cache = EphemeralIndexCache::new().expect("failed to create cache");

    // Browse individual directories first
    let dir1 = PathBuf::from("/mnt/volume/photos");
    let dir2 = PathBuf::from("/mnt/volume/documents");
    let _index = cache.create_for_indexing(dir1.clone(), IndexScope::Current);
    cache.mark_indexing_complete(&dir1, IndexScope::Current);
    let _index = cache.create_for_indexing(dir2.clone(), IndexScope::Current);
    cache.mark_indexing_complete(&dir2, IndexScope::Current);

    assert_eq!(cache.len(), 2);

    // Now volume index the root (recursive)
    let root = PathBuf::from("/mnt/volume");
    let _index = cache.create_for_indexing(root.clone(), IndexScope::Recursive);
    cache.mark_indexing_complete(&root, IndexScope::Recursive);

    // Child paths subsumed — only root remains
    assert_eq!(cache.len(), 1);
    // Children still covered via parent
    assert!(cache.is_indexed(&dir1));
    assert!(cache.is_indexed(&dir2));
}
```

## Technical Notes

### Canonicalization Cost

`Path::canonicalize()` is a syscall that hits the filesystem. On the hot path (every `list_directory` call), this adds ~1-5μs. For the cache methods (`is_indexed`, `get_for_path`), the `indexed_paths` set is tiny (1-10 entries), so linear scan with one canonicalization is negligible.

For `EphemeralIndex` methods that are called per-entry (like `get_or_assign_uuid` in a listing loop), the canonicalization should be done once per listing call and the canonical path passed down, not re-computed per entry.

### `indexed_paths` Size

Typically 1-10 entries. Linear iteration is faster than any tree structure at this scale. If it grows (unlikely), a sorted vec with binary prefix search would be the upgrade.

### `resolve_entry_id` Caching

Consider adding a small `HashMap<PathBuf, PathBuf>` cache for symlink resolutions on the `EphemeralIndex` to avoid repeated `canonicalize()` calls for the same path prefixes. This is optional and can be added later if profiling shows it matters.

## Related Tasks

- INDEX-001 - Hybrid Indexing Architecture (the cache design this fixes)
- INDEX-010 - Bidirectional UUID Reconciliation (depends on reliable cache behavior)
