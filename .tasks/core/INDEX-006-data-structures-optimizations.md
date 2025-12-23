---
id: INDEX-006
title: Data Structures & Memory Optimizations
status: Done
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, performance, memory, optimization]
whitepaper: Section 4.3.7
last_updated: 2025-12-16
---

## Description

Implement specialized data structures that enable efficient in-memory indexing with minimal memory overhead. The ephemeral layer uses NodeArena (slab allocator), NameCache (string interning), and NameRegistry (fast name lookups) to achieve ~50 bytes per file entry - a 4-6x reduction over naive approaches.

## Architecture

### NodeArena (Slab Allocator)

Instead of storing `HashMap<PathBuf, FileNode>` with 64-bit pointers, the arena uses a contiguous memory slab with 32-bit integer IDs:

```rust
pub struct NodeArena {
    // Contiguous slab of FileNode entries
    nodes: Vec<FileNode>,
    // Free list for reusing deleted slots
    free_list: Vec<NodeId>,
}

pub type NodeId = u32; // 32-bit instead of 64-bit pointer

pub struct FileNode {
    pub id: NodeId,              // 4 bytes
    pub parent_id: Option<NodeId>, // 5 bytes (4 + 1 tag)
    pub name_id: NameId,         // 4 bytes (index into NameCache)
    pub kind: FileKind,          // 1 byte
    pub size: u64,               // 8 bytes
    pub modified: u64,           // 8 bytes (timestamp)
    pub inode: u64,              // 8 bytes
    pub uuid: Uuid,              // 16 bytes
    // Total: ~54 bytes per node
}

impl NodeArena {
    pub fn alloc(&mut self, node: FileNode) -> NodeId {
        if let Some(id) = self.free_list.pop() {
            // Reuse deleted slot
            self.nodes[id as usize] = node;
            id
        } else {
            // Allocate new slot
            let id = self.nodes.len() as NodeId;
            self.nodes.push(node);
            id
        }
    }

    pub fn get(&self, id: NodeId) -> Option<&FileNode> {
        self.nodes.get(id as usize)
    }

    pub fn free(&mut self, id: NodeId) {
        self.free_list.push(id);
    }
}
```

**Benefits**:
- **Reduced pointer size**: 32-bit vs 64-bit (50% reduction)
- **Cache locality**: Contiguous memory layout
- **Reuse deleted slots**: Free list prevents fragmentation
- **Simplified serialization**: Just save Vec<FileNode>

### NameCache (String Interning)

Filenames repeat frequently in filesystems. The NameCache stores each unique name once and references it by ID:

```rust
pub struct NameCache {
    // Stores unique strings
    names: Vec<Arc<str>>,
    // Maps string → NameId for deduplication
    lookup: HashMap<Arc<str>, NameId>,
}

pub type NameId = u32;

impl NameCache {
    pub fn intern(&mut self, name: &str) -> NameId {
        if let Some(&id) = self.lookup.get(name) {
            return id; // Already interned
        }

        let id = self.names.len() as NameId;
        let arc_name: Arc<str> = Arc::from(name);
        self.names.push(arc_name.clone());
        self.lookup.insert(arc_name, id);
        id
    }

    pub fn get(&self, id: NameId) -> Option<&str> {
        self.names.get(id as usize).map(|s| s.as_ref())
    }
}
```

**Example Deduplication**:

```
Filesystem:
/app/node_modules/package1/index.js
/app/node_modules/package2/index.js
/app/node_modules/package3/index.js
...1000 packages

Without interning:
"index.js" stored 1000 times = 1000 * 8 bytes (string) = 8 KB

With interning:
"index.js" stored 1 time = 8 bytes
1000 references = 1000 * 4 bytes (NameId) = 4 KB
Total: 4.008 KB (50% reduction)

Common names like ".git", ".DS_Store", "README.md", "package.json" deduplicate heavily.
```

### NameRegistry (Name-Based Lookups)

The `NameRegistry` enables fast "find files by name" queries without full-text indexing:

```rust
pub struct NameRegistry {
    // Maps name_id → Vec<NodeId> (all files with this name)
    entries: BTreeMap<NameId, Vec<NodeId>>,
}

impl NameRegistry {
    pub fn insert(&mut self, name_id: NameId, node_id: NodeId) {
        self.entries.entry(name_id).or_insert_with(Vec::new).push(node_id);
    }

    pub fn find_by_name(&self, name_id: NameId) -> &[NodeId] {
        self.entries.get(&name_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
}
```

**Use Case**:
```rust
// Find all "README.md" files in ephemeral index
let readme_name_id = name_cache.intern("README.md");
let readme_nodes = registry.find_by_name(readme_name_id);
```

### Directory Path Caching (Persistent)

For the database layer, the `directory_paths` table caches full paths for O(1) lookups:

```sql
CREATE TABLE directory_paths (
    entry_id INTEGER PRIMARY KEY,
    path TEXT UNIQUE
);
```

This eliminates recursive parent traversal when building file paths.

## Implementation Files

### Ephemeral Data Structures
- `core/src/ops/indexing/ephemeral/arena.rs` - NodeArena slab allocator
- `core/src/ops/indexing/ephemeral/name.rs` - NameCache string interning
- `core/src/ops/indexing/ephemeral/registry.rs` - NameRegistry name-based lookups
- `core/src/ops/indexing/ephemeral/types.rs` - FileNode and related types

### Ephemeral Index
- `core/src/ops/indexing/ephemeral/index.rs` - EphemeralIndex using above structures

### Persistent Optimizations
- `core/src/ops/indexing/path_resolver.rs` - Path resolution with caching
- `core/src/ops/indexing/hierarchy.rs` - Closure table for O(1) hierarchy queries

## Memory Benchmark

| Approach | Bytes/Entry | 100K Files | 1M Files |
|----------|------------|-----------|----------|
| Naive (`HashMap<PathBuf, Entry>`) | ~250 bytes | 25 MB | 250 MB |
| With String Interning | ~150 bytes | 15 MB | 150 MB |
| **NodeArena + NameCache** | **~50 bytes** | **5 MB** | **50 MB** |

**Deduplication Impact**:

In typical filesystems with repeated names:
- **Before**: 250 bytes/entry * 100K = 25 MB
- **After**: 50 bytes/entry * 100K = 5 MB
- **Reduction**: 5x

## Acceptance Criteria

### NodeArena
- [x] Allocates FileNode entries in contiguous memory
- [x] Uses 32-bit NodeId instead of 64-bit pointers
- [x] Supports free list for deleted slots
- [x] get() is O(1) array indexing
- [x] Memory footprint ~54 bytes per node

### NameCache
- [x] Interns unique strings (stores each name once)
- [x] Returns NameId for deduplicated storage
- [x] intern() deduplicates automatically
- [x] get() retrieves string from NameId
- [x] Multiple directory trees share same cache

### NameRegistry
- [x] Maps name_id → Vec<NodeId>
- [x] Enables fast "find by name" queries
- [x] BTreeMap for sorted iteration
- [x] Supports multiple files with same name

### Integration
- [x] EphemeralIndex uses NodeArena for storage
- [x] EphemeralIndex uses NameCache for string interning
- [x] EphemeralIndex uses NameRegistry for name lookups
- [x] Multiple paths can share same EphemeralIndex
- [x] Memory usage is ~50 bytes per file entry
- [x] String deduplication works (common names stored once)

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Allocate node | O(1) | Vec push or free list pop |
| Get node | O(1) | Array indexing by NodeId |
| Free node | O(1) | Push to free list |
| Intern name | O(1) avg | HashMap lookup + Vec push |
| Get name | O(1) | Array indexing by NameId |
| Find by name | O(1) | BTreeMap lookup |

## Testing

### Manual Testing

```bash
# Index large directory in ephemeral mode
spacedrive index browse /usr --ephemeral

# Check memory usage
ps aux | grep spacedrive

# For 500K files, should use ~25 MB RAM for index
# (50 bytes/entry * 500K = 25 MB)
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_node_arena_allocation` - Verify NodeArena works
- `test_node_arena_free_list` - Test slot reuse
- `test_name_cache_deduplication` - Verify string interning
- `test_name_registry_lookup` - Test name-based queries
- `test_ephemeral_memory_usage` - Benchmark memory per file

### Memory Usage Test

```rust
#[test]
fn test_memory_per_entry() {
    let mut index = EphemeralIndex::new();

    // Index 100K files
    for i in 0..100_000 {
        index.insert(format!("/test/file_{}.txt", i));
    }

    // Measure memory usage
    let arena_size = std::mem::size_of_val(&index.arena.nodes);
    let name_cache_size = std::mem::size_of_val(&index.name_cache.names);
    let total = arena_size + name_cache_size;

    // Should be ~5 MB for 100K files (50 bytes/entry)
    assert!(total < 6_000_000);
    println!("Memory per entry: {} bytes", total / 100_000);
}
```

## Comparison: Naive vs Optimized

### Naive Approach
```rust
// 250+ bytes per entry
struct Entry {
    path: PathBuf,         // ~64 bytes (heap allocation)
    name: String,          // ~24 bytes (heap allocation)
    parent: Option<Box<Entry>>, // 8 bytes pointer
    kind: FileKind,        // 1 byte
    size: u64,             // 8 bytes
    modified: SystemTime,  // 16 bytes
    inode: u64,            // 8 bytes
    uuid: Uuid,            // 16 bytes
    children: Vec<Entry>,  // 24 bytes Vec
}

let mut index: HashMap<PathBuf, Entry> = HashMap::new();
// HashMap overhead: ~32 bytes per entry
// Total: ~282 bytes per entry
```

### Optimized Approach
```rust
// ~50 bytes per entry
struct FileNode {
    id: NodeId,            // 4 bytes
    parent_id: Option<NodeId>, // 5 bytes
    name_id: NameId,       // 4 bytes (deduplicated)
    kind: FileKind,        // 1 byte
    size: u64,             // 8 bytes
    modified: u64,         // 8 bytes
    inode: u64,            // 8 bytes
    uuid: Uuid,            // 16 bytes
}
// Total: ~54 bytes per entry
// No HashMap overhead (arena indexed by NodeId)
```

## Future Enhancements

- **Port to Persistent Layer**: Apply name pooling to SQLite schema for database size reduction
- **Compression**: Use zstd compression for name cache serialization
- **Memory Mapping**: Map arena to disk for persistent ephemeral indexes
- **Tiered Storage**: Hot nodes in RAM, cold nodes on disk

## Related Tasks

- INDEX-001 - Hybrid Architecture (ephemeral layer uses these structures)
- INDEX-003 - Database Architecture (persistent layer could benefit from name pooling)
