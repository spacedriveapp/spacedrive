# Closure Table Indexing Proposal for Spacedrive

## Executive Summary

This document explores how closure tables could improve Spacedrive's filesystem indexing performance, particularly for hierarchical queries and directory aggregation operations.

## Current Implementation Analysis

### Materialized Path Approach
Spacedrive currently uses a materialized path approach where:
- Each entry stores its `relative_path` (e.g., "Documents/Projects")
- Full paths are reconstructed by combining `location_path + relative_path + name`
- No explicit parent-child relationships in the database

### Performance Bottlenecks
1. **String-based path matching** for finding children/descendants
2. **Sequential directory aggregation** from leaves to root
3. **Inefficient ancestor queries** (finding all parents of a file)
4. **Complex LIKE queries** for subtree operations

## Closure Table Solution

### Concept
A closure table stores all ancestor-descendant relationships explicitly:

```sql
CREATE TABLE entry_closure (
    ancestor_id INTEGER NOT NULL,
    descendant_id INTEGER NOT NULL,
    depth INTEGER NOT NULL,
    PRIMARY KEY (ancestor_id, descendant_id),
    FOREIGN KEY (ancestor_id) REFERENCES entries(id),
    FOREIGN KEY (descendant_id) REFERENCES entries(id)
);

CREATE INDEX idx_closure_descendant ON entry_closure(descendant_id);
CREATE INDEX idx_closure_depth ON entry_closure(ancestor_id, depth);
```

### Example Data
For a path `/Documents/Projects/spacedrive/README.md`:
```
entry_closure:
ancestor_id | descendant_id | depth
----------- | ------------- | -----
1           | 1             | 0     (Documents → Documents)
1           | 2             | 1     (Documents → Projects)
1           | 3             | 2     (Documents → spacedrive)
1           | 4             | 3     (Documents → README.md)
2           | 2             | 0     (Projects → Projects)
2           | 3             | 1     (Projects → spacedrive)
2           | 4             | 2     (Projects → README.md)
3           | 3             | 0     (spacedrive → spacedrive)
3           | 4             | 1     (spacedrive → README.md)
4           | 4             | 0     (README.md → README.md)
```

## Benefits for Spacedrive

### 1. Optimized Queries

**Get all children of a directory:**
```sql
-- Current approach (string matching)
SELECT * FROM entries 
WHERE location_id = ? AND relative_path = ?;

-- Closure table approach (indexed lookup)
SELECT e.* FROM entries e
JOIN entry_closure c ON e.id = c.descendant_id
WHERE c.ancestor_id = ? AND c.depth = 1;
```

**Get entire subtree:**
```sql
-- Current approach (complex LIKE)
SELECT * FROM entries 
WHERE location_id = ? 
AND (relative_path = ? OR relative_path LIKE ?||'/%');

-- Closure table approach (simple join)
SELECT e.* FROM entries e
JOIN entry_closure c ON e.id = c.descendant_id
WHERE c.ancestor_id = ? AND c.depth > 0
ORDER BY c.depth;
```

**Get all ancestors (breadcrumb):**
```sql
-- Current approach (requires application logic)
-- Must parse path and query each component

-- Closure table approach (single query)
SELECT e.* FROM entries e
JOIN entry_closure c ON e.id = c.ancestor_id
WHERE c.descendant_id = ?
ORDER BY c.depth DESC;
```

### 2. Improved Directory Aggregation

The current aggregation phase could be dramatically improved:

```sql
-- Calculate directory sizes in one query
WITH RECURSIVE dir_sizes AS (
    SELECT 
        c.ancestor_id as dir_id,
        SUM(e.size) as total_size,
        COUNT(DISTINCT e.id) as file_count
    FROM entry_closure c
    JOIN entries e ON c.descendant_id = e.id
    WHERE e.kind = 0  -- Files only
    GROUP BY c.ancestor_id
)
UPDATE entries 
SET size = dir_sizes.total_size
FROM dir_sizes
WHERE entries.id = dir_sizes.dir_id AND entries.kind = 1;
```

### 3. Fast Move Operations

Moving a directory and all its contents becomes much simpler:

```sql
-- Update closure table for move operation
-- 1. Remove old relationships
DELETE FROM entry_closure 
WHERE descendant_id IN (
    SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?
) AND ancestor_id NOT IN (
    SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?
);

-- 2. Add new relationships (can be optimized with CTEs)
```

## Implementation Strategy

### Hybrid Approach
Keep the current materialized path system but add closure tables as an optimization:

1. **Maintain both systems** during transition
2. **Use closure tables for**:
   - Directory aggregation
   - Subtree queries
   - Ancestor lookups
   - Move operations
3. **Keep materialized paths for**:
   - Display purposes
   - Simple path construction
   - Backwards compatibility

### Migration Plan

1. **Add closure table** without removing existing structure
2. **Populate closure table** during indexing:
   ```rust
   // In indexing job
   fn process_entry(entry: &Entry, parent_id: Option<i32>) {
       // Insert entry
       let entry_id = insert_entry(entry);
       
       // Build closure relationships
       if let Some(parent) = parent_id {
           // Insert all ancestor relationships
           insert_closure_relationships(entry_id, parent);
       }
   }
   ```

3. **Update queries gradually** to use closure tables
4. **Benchmark performance** improvements
5. **Remove string-based queries** once proven

### Database Impact

**Storage overhead:**
- For a tree with N nodes and average depth D: ~N * D rows
- Example: 1M files, avg depth 5 = ~5M closure rows
- With 3 integers per row = ~60MB additional storage

**Trade-offs:**
- ✅ O(1) child lookups vs O(N) string matching
- ✅ O(1) subtree queries vs O(N) LIKE queries  
- ✅ Parallel aggregation possible
- ❌ More complex inserts/moves
- ❌ Additional storage requirements

## Benchmarking Metrics

Compare before/after implementation:
1. **Directory listing speed** (get children)
2. **Subtree query performance** (get all descendants)
3. **Aggregation phase duration**
4. **Move operation speed**
5. **Memory usage**
6. **Database size**

## Conclusion

Closure tables could significantly improve Spacedrive's indexing performance, especially for:
- Large directory trees
- Deep hierarchies  
- Frequent directory aggregation
- Complex hierarchical queries

The hybrid approach allows gradual migration while maintaining backwards compatibility. The storage overhead (estimated ~6% for typical filesystems) is justified by the performance gains for read-heavy operations.

## Next Steps

1. Create proof-of-concept branch
2. Implement closure table schema
3. Add closure maintenance to indexing job
4. Benchmark with real-world data
5. Make go/no-go decision based on results