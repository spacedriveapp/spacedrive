---
id: INDEX-002
title: Stale File Detection Algorithm
status: To Do
assignee: unassigned
parent: INDEX-000
priority: High
tags: [indexing, stale-detection, offline-recovery]
whitepaper: Section 4.3.4
---

## Description

Implement the algorithm for detecting stale files after the application has been offline. This is a critical part of the indexing process, ensuring that changes made while the application was not running are correctly detected and reconciled.

## Implementation Steps

1.  Design the algorithm for stale file detection, likely using a combination of inode numbers, modification times, and file sizes.
2.  Implement the algorithm as part of the `IndexerJob`'s startup process.
3.  The algorithm should be able to handle edge cases like file renames and moves.
4.  The algorithm should be efficient and not significantly slow down the application's startup time.

## Acceptance Criteria
-   [ ] The system can correctly detect files that were modified or deleted while the application was offline.
-   [ ] The system can correctly detect files that were moved or renamed while the application was offline.
-   [ ] The stale file detection process is efficient and does not block the application for an unreasonable amount of time.
