---
id: LOC-005
title: "Virtual Locations via Pure Hierarchical Model"
status: To Do
assignee: james
parent: LOC-000
priority: High
tags: [core, vdfs, database, refactor]
whitepaper: "Section 4.1.2, 4.3"
---

## Description

A "Location" should not be a rigid, physical path on a disk. It should be a virtual, named pointer to any directory `Entry` in the VDFS.

## Implementation Notes

- [cite_start]The implementation should follow the detailed plan in the **VIRTUAL_LOCATIONS_DESIGN.md** [cite: 5552-5564] document.
- [cite_start]**Drop `relative_path` column** from the `entries` table[cite: 5555].
- [cite_start]**Create `directory_paths` table** to act as a denormalized cache for directory path strings [cite: 5555-5556].
- [cite_start]**Modify `locations` table schema** to store a reference to an `entry_id` instead of a string path[cite: 5559].
- [cite_start]Update indexing and move logic to populate and maintain the `directory_paths` table transactionally [cite: 5560-5561].
- [cite_start]Create a centralized `PathResolver` service to reconstruct full paths on demand[cite: 5563].

## Acceptance Criteria

- [ ] A user can create a "Location" that points to any directory `Entry`, regardless of its physical path.
- [ ] The `relative_path` column is successfully removed from the database schema.
- [ ] Path reconstruction for files is performant, leveraging the `directory_paths` cache.
- [ ] [cite_start]Moving a directory correctly updates its path in the cache and triggers a background job to update descendant paths[cite: 5562].
