<!--CREATED: 2025-08-06-->
Guidance Document: Evolving to a Pure Hierarchical Model with
Virtual Locations

Objective: To refactor the Spacedrive VDFS core from the
current hybrid model (closure table + materialized paths) to
a "pure" hierarchical model. This will enable fully virtual
locations, significantly reduce database size, and improve
data integrity by eliminating path string redundancy.

Starting Point: This guide assumes the changes from the
previous implementation are complete: the entries table has a
parent_id, and the entry_closure table is being correctly
populated for all new and moved entries.

---

1. Architectural Principles & Rationale

This refactoring is based on several key insights we've
developed:

1.  The Goal is Virtual Locations: A "Location" should not be a
    rigid, physical path on a disk. It should be a virtual,
    named pointer to any directory Entry in the VDFS. This
    allows users to create locations that match their mental
    model (e.g., a "Projects" location that points to
    /Users/me/work/projects) without being constrained by the
    filesystem's physical layout.

2.  Eliminating `relative_path`: The primary obstacle to virtual
    locations and the main source of data bloat is the
    relative_path column in the entries table. By removing it, we
    achieve a "pure" model where the hierarchy is defined only
    by the parent_id and entry_closure tables. This is the single
    source of truth for the hierarchy, making the system more
    robust and easier to maintain.

3.  Solving the Path Reconstruction Problem: We identified that
    removing relative_path entirely would create a major
    performance bottleneck when displaying lists of files from
    multiple directories (e.g., search results), as it would
    require thousands of recursive queries to reconstruct their
    paths.

4.  The "Directory-Only Path Cache" Solution: The optimal
    solution is to introduce a new, dedicated table named
    directory_paths.
    - Purpose: This table acts as a permanent, denormalized
      cache. Its sole function is to store the pre-computed,
      full path string for every directory.
    - Efficiency: By only storing paths for directories (which
      are far less numerous than files), we reduce the storage
      overhead by ~90% compared to caching all paths, while
      retaining almost all the performance benefits.
    - How it Works: A file's full path is constructed
      on-the-fly with near-zero cost by fetching its parent
      directory's path from this new table and appending the
      file's name. This is an extremely fast operation.

---

2. Step-by-Step Implementation Plan

Phase 1: Database Schema Changes

This phase modifies the database to support the new
architecture. This must be done in a new migration file.

1.  Action: Drop the relative_path column from the entries
    table.
    - File: New migration file in
      src/infrastructure/database/migration/.
    - Instruction:

1 -- In the `up` function of the migration
2 manager.alter_table(
3 Table::alter()
4 .table(Entry::Table)
5 .drop_column(Alias::new
("relative_path"))
6 .to_owned(),
7 ).await?;

2.  Action: Create the new directory_paths table.
    - File: Same new migration file.
    - Instruction:


    1         -- In the `up` function of the migration
    2         manager.create_table(
    3             Table::create()
    4                 .table(DirectoryPaths::Table)
    5                 .if_not_exists()
    6                 .col(
    7                     ColumnDef::new
      (DirectoryPaths::EntryId)
    8                         .integer()
    9                         .primary_key(),

10 )
11 .col(ColumnDef::new
(DirectoryPaths::Path).text().not_null())
12 .foreign_key(
13 ForeignKey::create()
14
.name("fk_directory_path_entry")
15 .from(DirectoryPaths::Table
, DirectoryPaths::EntryId)
16 .to(Entry::Table,
Entry::Id)
17
.on_delete(ForeignKeyAction::Cascade), // Critical
for auto-cleanup
18 )
19 .to_owned(),
20 ).await?;

3.  Action: Create the corresponding SeaORM entity for
    directory_paths.
    - File:
      src/infrastructure/database/entities/directory_paths.rs
      (new file).
    - Instruction: Create a new entity struct that maps to the
      table above. Remember to add it to
      src/infrastructure/database/entities/mod.rs.

Phase 2: Make Locations Virtual

This is the core change that decouples Locations from the
filesystem.

1.  Action: Modify the locations table schema.
    - File: Same new migration file.
    - Instruction: The locations table currently stores a
      path: String. This needs to be changed to entry_id: i32.


    1         -- This will require dropping the old
      column and adding a new one.
    2         -- NOTE: Since there are no v2 users, a
      destructive change is acceptable.
    3         manager.alter_table(
    4             Table::alter()
    5                 .table(Location::Table)
    6                 .drop_column(Alias::new("path"))
    7                 .to_owned(),
    8         ).await?;
    9

10 manager.alter_table(
11 Table::alter()
12 .table(Location::Table)
13 .add_column(
14 ColumnDef::new
(Location::EntryId).integer().not_null()
15 )
16 .to_owned(),
17 ).await?;

       * Reasoning: A Location is now just a named reference to a
         directory Entry.

2.  Action: Update the Location SeaORM entity to reflect this
    change.
    - File: src/infrastructure/database/entities/location.rs.

Phase 3: Update Indexing and Core Logic

This phase adapts the application logic to populate and use
the new structures.

1.  Action: Update EntryProcessor::create_entry.

    - File: src/operations/indexing/entry.rs.
    - Instruction: When a new Entry is created, if that entry is
      a directory, the logic must:
      1.  Determine its full path. This can be done by querying
          the directory_paths table for its parent_id and
          appending the new directory's name.
      2.  INSERT the new record into the directory_paths
          table.
      3.  This entire operation (creating the entry,
          populating the closure table, and populating the
          directory path) should be wrapped in a single
          database transaction.

2.  Action: Update EntryProcessor::move_entry.

    - File: src/operations/indexing/entry.rs.
    - Instruction: When a directory is moved:
      1.  The existing transactional logic for updating
          parent_id and the entry_closure table is still
          correct.
      2.  Add a step within the transaction to UPDATE the
          directory's own path in the directory_paths table.
      3.  Crucially, after the transaction commits, spawn a
          low-priority background job. This job's
          responsibility is to find all descendant directories
          of the one that was moved (using the closure table)
          and update their paths in the directory_paths table.
    - Reasoning: This makes the move operation feel instantaneous
      to the user, deferring the expensive task of updating all
      descendant paths.

3.  Action: Create a centralized Path Retrieval Service.

    - File: A new module, e.g.,
      src/operations/indexing/path_resolver.rs.
    - Instruction: This service will have a function like
      get_full_path(entry_id: i32) -> Result<PathBuf>.
      - If the entry is a directory, it will SELECT path
        FROM directory_paths WHERE entry_id = ?.
      - If the entry is a file, it will SELECT e.name,
        dp.path FROM entries e JOIN directory_paths dp ON
        e.parent_id = dp.entry_id WHERE e.id = ?.
    - Reasoning: This centralizes path reconstruction logic
      and ensures it's done consistently and efficiently
      everywhere.

4.  Action: Refactor all parts of the codebase that need a full
    path.
    - Files: This will be a broad change. Key areas will
      include:
      - Search result generation.
      - UI-facing API endpoints.
      - The Action System's preview generation.
      - Any logging that requires full paths.
    - Instruction: All these locations must now call the new
      PathRetrievalService instead of trying to concatenate
      relative_path and name.

---

This guide provides a clear, logical path to achieving a more
robust, scalable, and flexible architecture for Spacedrive. By
following these steps, the next agent can successfully
implement this significant and valuable upgrade.
