---
id: CORE-009
title: User-Managed Collections
status: To Do
assignee: unassigned
parent: CORE-000
priority: Medium
tags: [core, vdfs, collections, organization]
whitepaper: Section 4.1
---

## Description

Implement the ability for users to save selections of files into persistent collections. This will provide a flexible way for users to organize their files without being constrained by the file system hierarchy.

## Implementation Steps

1.  Design the database schema for storing collections and their members.
2.  Implement the logic for creating, renaming, and deleting collections.
3.  Implement the logic for adding and removing entries from collections.
4.  Develop a UI/CLI for managing collections.

## Acceptance Criteria
-   [ ] A user can create a new collection.
-   [ ] A user can add files and directories to a collection.
-   [ ] A user can view the contents of a collection.
-   [ ] A user can remove items from a collection.
