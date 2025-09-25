---
id: CORE-007
title: On-Demand State Computation
status: Completed
assignee: unassigned
parent: CORE-000
priority: Low
tags: [core, vdfs, performance]
whitepaper: Section 4.1
---

## Description

Implement on-demand state computation, where the state of an entry is computed on-demand rather than being stored directly in the database. This improves performance and reduces storage overhead by following a "hierarchy of truth" approach.

## Implementation Steps

1. ✅ Identify the entry properties that can be computed on-demand (e.g., size of a directory, number of files).
2. ✅ Implement the logic to compute these properties when they are requested.
3. ✅ Integrate the on-demand computation into the VDFS API.
4. ✅ Cache the computed values to avoid re-computation.

## Acceptance Criteria
- ✅ The state of an entry is computed on-demand.
- ✅ The on-demand computation is efficient and does not significantly impact performance.
- ✅ The computed values are cached to improve performance.

## Implementation Details

This functionality is implemented in `core/src/service/entry_state_service.rs`:

- **EntryStateService::get_states_for_entries()** queries the JobManager for active operations (highest priority)
- Falls back to database queries for physical state of entries not affected by running jobs
- Defaults to `Available` state for remaining entries
- Returns a clean `EntryState` enum with job IDs for real-time progress tracking

The implementation follows the "hierarchy of truth" described in the whitepaper Section 4.1.
