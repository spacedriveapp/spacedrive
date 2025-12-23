---
id: INDEX-003
title: Database Architecture (Closure Table & Directory Paths Cache)
status: Done
assignee: jamiepine
parent: INDEX-000
priority: High
tags: [indexing, database, closure-table, performance]
whitepaper: Section 4.3.5
last_updated: 2025-12-16
related_tasks: [CORE-004]
---

## Description

Implement the specialized database schema optimizations that enable fast hierarchy queries and path lookups. Instead of recursive queries, use precomputed closure tables for O(1) "find all descendants" operations and a directory paths cache for instant absolute path resolution.

## Architecture

### Closure Table

The `entry_closure` table stores all transitive ancestor-descendant relationships with precomputed depths:

```sql
CREATE TABLE entry_closure (
    ancestor_id INTEGER,
    descendant_id INTEGER,
    depth INTEGER,
    PRIMARY KEY (ancestor_id, descendant_id)
);
```

#### Example Hierarchy

For `/home/user/docs/report.pdf`:

```
home/ (id=1)
└─ user/ (id=2)
   └─ docs/ (id=3)
      └─ report.pdf (id=4)
```

#### Closure Table Entries

```sql
-- Self-references (depth 0)
(1, 1, 0)  -- home → home
(2, 2, 0)  -- user → user
(3, 3, 0)  -- docs → docs
(4, 4, 0)  -- report.pdf → report.pdf

-- Direct relationships (depth 1)
(1, 2, 1)  -- home → user
(2, 3, 1)  -- user → docs
(3, 4, 1)  -- docs → report.pdf

-- Transitive relationships
(1, 3, 2)  -- home → docs
(2, 4, 2)  -- user → report.pdf
(1, 4, 3)  -- home → report.pdf
```

#### Query Benefits

```sql
-- Find all descendants of "home" (O(1) regardless of depth)
SELECT descendant_id, depth
FROM entry_closure
WHERE ancestor_id = 1 AND depth > 0;

-- Find all ancestors of "report.pdf"
SELECT ancestor_id, depth
FROM entry_closure
WHERE descendant_id = 4 AND depth > 0;

-- Find direct children only
SELECT descendant_id
FROM entry_closure
WHERE ancestor_id = 1 AND depth = 1;
```

#### Move Operations

When moving a subtree, rebuild closures for entire moved branch:

```rust
// Moving /home/user/docs to /home/archive/docs
// Affects thousands of rows for large directories
async fn rebuild_closure_for_subtree(entry_id: i32, db: &DatabaseConnection) -> Result<()> {
    // 1. Delete old closures for moved subtree
    // 2. Recompute closures based on new parent_id
    // 3. Insert new closure rows
}
```

**Cost**: O(N²) worst-case for deeply nested trees, but acceptable for typical hierarchies.

### Directory Paths Cache

The `directory_paths` table caches full absolute paths for O(1) lookups:

```sql
CREATE TABLE directory_paths (
    entry_id INTEGER PRIMARY KEY,
    path TEXT UNIQUE
);
```

#### Example Entries

```sql
INSERT INTO directory_paths VALUES
  (1, '/home'),
  (2, '/home/user'),
  (3, '/home/user/docs');
```

#### Benefits

- **O(1) Path Resolution**: No recursive parent traversal needed
- **Instant Child Path Construction**: `parent_path + "/" + child_name`
- **Fast Path-Based Queries**: Direct lookup by full path

#### Maintenance

- **Create**: Insert on directory creation
- **Move**: Update path and all descendant paths
- **Delete**: Remove on directory deletion

### Entries Table

Core filesystem metadata storage:

```sql
CREATE TABLE entry (
    id INTEGER PRIMARY KEY,
    uuid UUID UNIQUE,
    parent_id INTEGER,
    name TEXT,
    extension TEXT,
    kind INTEGER,
    size BIGINT,
    inode BIGINT,
    content_id INTEGER,
    aggregate_size BIGINT,  -- Calculated in Phase 3
    child_count INTEGER,     -- Calculated in Phase 3
    file_count INTEGER       -- Calculated in Phase 3
);
```

## Implementation Files

### Closure Table Management
- `core/src/ops/indexing/hierarchy.rs` - Closure table insert/update/delete operations
- `core/src/ops/indexing/database_storage.rs` - Low-level CRUD with closure updates

### Directory Path Caching
- `core/src/ops/indexing/path_resolver.rs` - Path resolution and caching
- `core/src/ops/indexing/database_storage.rs` - Directory path cache updates

### Database Operations
- `core/src/ops/indexing/database_storage.rs` - DatabaseStorage with closure integration
- `core/src/ops/indexing/phases/processing.rs` - Closure creation during Phase 2
- `core/src/ops/indexing/phases/aggregation.rs` - Closure queries for aggregation

## Acceptance Criteria

- [x] Closure table stores all ancestor-descendant pairs
- [x] Self-references included (depth 0)
- [x] Depth correctly calculated for all relationships
- [x] Find descendants query is O(1) regardless of nesting depth
- [x] Find ancestors query is O(1)
- [x] Move operations correctly rebuild closures for moved subtree
- [x] Directory paths cache stores full absolute paths
- [x] Path lookups are O(1) (no recursive traversal)
- [x] Moving directories updates descendant paths in cache
- [x] Deleting directories removes from cache
- [x] Aggregates (aggregate_size, child_count, file_count) calculated via closure table
- [x] Phase 2 creates closure entries for new files
- [x] Phase 3 uses closure table for bottom-up aggregation

## Performance Impact

| Operation | Without Closure Table | With Closure Table |
|-----------|---------------------|-------------------|
| Find all descendants | O(N) recursive | O(1) single query |
| Calculate directory size | O(N) traversal | O(1) precomputed |
| Find ancestors | O(depth) | O(1) single query |
| Move directory | O(1) update | O(subtree) rebuild |

**Trade-off**: Storage cost (N² worst-case) for query speed (O(1) reads).

## Storage Cost

For a typical hierarchy:
- **Flat directory (100 files)**: 100 + 100 = 200 closure rows
- **Deep nesting (10 levels, 10 items/level)**: ~5,000 closure rows
- **Pathological (1 file, 1000 levels deep)**: ~500,000 closure rows

In practice, filesystem hierarchies are relatively balanced, keeping storage overhead reasonable.

## Testing

### Manual Testing

```bash
# Index a deep directory
spacedrive index location ~/Documents --mode shallow

# Check closure table populated
spacedrive db query "SELECT COUNT(*) FROM entry_closure"

# Verify O(1) descendant query
spacedrive db query "
  SELECT COUNT(*)
  FROM entry_closure
  WHERE ancestor_id = (SELECT id FROM entry WHERE name = 'Documents')
"

# Test move operation
mv ~/Documents/Work ~/Documents/Archive/Work

# Verify closures rebuilt correctly
spacedrive db query "
  SELECT * FROM entry_closure
  WHERE descendant_id = (SELECT id FROM entry WHERE name = 'Work')
"
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_closure_table_creation` - Verify closures created during indexing
- `test_closure_table_queries` - Test O(1) descendant queries
- `test_move_rebuilds_closures` - Verify move updates closures
- `test_directory_path_cache` - Test O(1) path lookups
- `test_aggregation_uses_closures` - Verify Phase 3 uses closure table

## Related Tasks

- INDEX-002 - Five-Phase Pipeline (Phase 2 creates closures, Phase 3 uses them)
- INDEX-004 - Change Detection (Move detection triggers closure rebuild)
- CORE-004 - Closure Table (base implementation)
