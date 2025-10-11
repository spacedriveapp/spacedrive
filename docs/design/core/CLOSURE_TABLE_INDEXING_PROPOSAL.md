<!--CREATED: 2025-07-23-->
'''# Closure Table Indexing Proposal for Spacedrive

## Executive Summary

This document proposes a shift from a materialized path-based indexing system to a hybrid model incorporating a **Closure Table**. This change will dramatically improve hierarchical query performance, address critical scaling bottlenecks, and enhance data integrity, particularly for move operations. The core of this proposal is to supplement the existing `entries` table with an `entry_closure` table and a `parent_id` field, enabling highly efficient and scalable filesystem indexing.

## 1. Current Implementation Analysis

### Materialized Path Approach
Spacedrive currently uses a materialized path approach where:
- Each entry stores its `relative_path` (e.g., "Documents/Projects").
- Full paths are reconstructed by combining `location_path + relative_path + name`.
- There are no explicit, indexed parent-child relationships in the database.

### Performance Bottlenecks
This design leads to significant performance issues that will not scale:
1.  **String-based path matching** for finding children/descendants (`LIKE 'path/%'`). These queries are un-indexable and require full table scans.
2.  **Sequential directory aggregation** from leaves to root, which is slow and complex.
3.  **Inefficient ancestor queries** (e.g., for breadcrumbs), requiring multiple queries and string parsing in the application layer.

## 2. The Closure Table Solution

### Concept
A closure table stores all ancestor-descendant relationships explicitly, turning slow string operations into highly efficient integer-based joins.

### Proposed Schema Changes

**1. Add `parent_id` to `entries` table:**
This provides a direct, indexed link to a parent, simplifying relationship lookups during indexing.

```sql
ALTER TABLE entries ADD COLUMN parent_id INTEGER REFERENCES entries(id) ON DELETE SET NULL;
```

**2. Create `entry_closure` table:**

```sql
CREATE TABLE entry_closure (
    ancestor_id INTEGER NOT NULL,
    descendant_id INTEGER NOT NULL,
    depth INTEGER NOT NULL,
    PRIMARY KEY (ancestor_id, descendant_id),
    FOREIGN KEY (ancestor_id) REFERENCES entries(id) ON DELETE CASCADE,
    FOREIGN KEY (descendant_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE INDEX idx_closure_descendant ON entry_closure(descendant_id);
CREATE INDEX idx_closure_ancestor_depth ON entry_closure(ancestor_id, depth);
```
*Note: `ON DELETE CASCADE` is crucial. When an entry is deleted, all its relationships in the closure table are automatically and efficiently removed by the database.* 

## 3. Critical Requirement: Inode-Based Change Detection

A core prerequisite for the closure table's integrity is the indexer's ability to reliably distinguish between a file **move** and a **delete/add** operation, especially when Spacedrive is catching up on offline changes.

**The Problem:** Without proper move detection, moving a directory containing 10,000 files would be misinterpreted as 10,000 deletions and 10,000 creations, leading to a catastrophic and incorrect rebuild of the closure table.

**The Solution:** The indexing process **must** be inode-aware.
1.  **Initial Scan:** Before scanning the filesystem, the indexer must load all existing entries for the target location into two in-memory maps:
    *   `path_map: HashMap<PathBuf, Entry>`
    *   `inode_map: HashMap<u64, Entry>`
2.  **Reconciliation:** When the indexer encounters a file on disk:
    *   If the file's path is not in `path_map`, it then looks up the file's **inode** in `inode_map`.
    *   If the inode is found, the indexer has detected a **move**. It must trigger a specific `EntryMoved` event/update.
    *   If neither the path nor the inode is found, it is a genuinely new file.

This is the only way to guarantee the integrity of the hierarchy and prevent data corruption in the closure table.

## 4. Implementation Strategy

### Hybrid Approach
We will keep the current materialized path system for display purposes and backwards compatibility but add the closure table as the primary mechanism for all hierarchical operations.

### Implementation Plan

1.  **Schema Migration:**
    *   Create a new database migration file.
    *   Add the `parent_id` column to the `entries` table.
    *   Create the `entry_closure` table and its indexes as defined above.

2.  **Update Indexing Logic:**
    *   Modify the `EntryProcessor::create_entry` function to accept a `parent_id`.
    *   When a new entry is inserted, within the same database transaction:
        1.  Insert the entry and get its new `id`.
        2.  Insert the self-referential row into `entry_closure`: `(ancestor_id: id, descendant_id: id, depth: 0)`.
        3.  If `parent_id` exists, execute the following query to copy the parent's ancestor relationships:
            ```sql
            INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
            SELECT p.ancestor_id, ? as descendant_id, p.depth + 1
            FROM entry_closure p
            WHERE p.descendant_id = ? -- parent_id
            ```

3.  **Refactor Core Operations:**

    '''    *   **Move Operation:** This is the most complex part. When an `EntryMoved` event is handled, the entire operation **must be wrapped in a single database transaction** to ensure atomicity and prevent data corruption.
        1.  **Disconnect Subtree:** Delete all hierarchical relationships for the moved node and its descendants, *except* for their own internal relationships.'''
            ```sql
            DELETE FROM entry_closure
            WHERE descendant_id IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?1) -- All descendants of the moved node
              AND ancestor_id NOT IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?1); -- All ancestors of the moved node itself
            ```
        2.  **Update `parent_id`:** Set the `parent_id` of the moved entry to its new parent.
        3.  **Reconnect Subtree:** Connect the moved subtree to its new parent.
            ```sql
            INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
            SELECT p.ancestor_id, c.descendant_id, p.depth + c.depth + 1
            FROM entry_closure p, entry_closure c
            WHERE p.descendant_id = ?1 -- new_parent_id
              AND c.ancestor_id = ?2; -- moved_entry_id
            ```

    *   **Delete Operation:** With `ON DELETE CASCADE` defined on the foreign keys, the database will handle this automatically. When an entry is deleted, all rows in `entry_closure` where it is an `ancestor_id` or `descendant_id` will be removed.

4.  **Refactor Hierarchical Queries:**
    *   Gradually replace all `LIKE` queries for path matching with efficient `JOIN`s on the `entry_closure` table.
        *   **Get Children:** `... WHERE c.ancestor_id = ? AND c.depth = 1`
        *   **Get Descendants:** `... WHERE c.ancestor_id = ? AND c.depth > 0`
        *   **Get Ancestors:** `... WHERE c.descendant_id = ? ORDER BY c.depth DESC`

## 5. Conclusion

While this is a significant architectural change, it is essential for the long-term performance and scalability of Spacedrive. The current string-based path matching is a critical bottleneck that this proposal directly and correctly addresses using established database patterns. The hybrid approach and phased rollout plan provide a safe and manageable path to implementation.
'''