---
id: CORE-007
title: Implement Quantum State for On-Demand State Computation
status: To Do
assignee: unassigned
parent: CORE-000
priority: Low
tags: [core, vdfs, quantum-state, performance]
whitepaper: Section 4.1
---

## Description

Implement the "Quantum State" concept, where the state of an entry is computed on-demand rather than being stored directly in the database. This will improve performance and reduce storage overhead.

## Implementation Steps

1.  Identify the entry properties that can be computed on-demand (e.g., size of a directory, number of files).
2.  Implement the logic to compute these properties when they are requested.
3.  Integrate the on-demand computation into the VDFS API.
4.  Cache the computed values to avoid re-computation.

## Acceptance Criteria
-   [ ] The state of an entry is computed on-demand.
-   [ ] The on-demand computation is efficient and does not significantly impact performance.
-   [ ] The computed values are cached to improve performance.
