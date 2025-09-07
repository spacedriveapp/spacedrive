# CLI Daemon Actions Refactoring Design Document

## Overview

This document outlines the plan to refactor CLI daemon handlers to properly use the action system for all state-mutating operations, while keeping read-only operations as direct queries.

## Principles

1. **Actions for State Mutations**: Any operation that modifies state (database, filesystem, job state) should go through the action system
2. **Direct Queries for Reads**: Read-only operations should remain as direct database queries or service calls
3. **Consistency**: Similar operations should follow similar patterns
4. **Audit Trail**: Actions provide built-in audit logging for all mutations

## Current State Analysis

### Operations Currently Using Actions ✅

- `LocationAdd` - Uses `LocationAddAction`
- `LocationRemove` - Uses `LocationRemoveAction`
- `Copy` - Uses `FileCopyAction`

### Operations That Should Use Actions 🔄

#### Indexing Operations

- `IndexLocation` - Re-indexes an existing location
- `IndexAll` - Indexes all locations in a library

**Can use existing actions:**

- `IndexLocation` → Use existing `LocationIndexAction`
- `IndexAll` → Could create a new `LibraryIndexAllAction` or dispatch multiple `LocationIndexAction`s

### Operations That Should NOT Use Actions ✅

These are read-only operations or ephemeral operations:

- `Browse` - Just reads filesystem without persisting
- `QuickScan` with `ephemeral: true` - Temporary scan, no persistence
- All List operations (`ListLibraries`, `ListLocations`, `ListJobs`)
- All Get operations (`GetJobInfo`, `GetCurrentLibrary`, `GetStatus`)
- `Ping` - Simple health check

### Operations to Remove 🗑️

- `IndexPath` - Redundant with location-based indexing
- `QuickScan` with `ephemeral: false` - Should just use location add + index

## Implementation Plan

### Phase 1: Update File Handler

1. Remove `IndexPath` command entirely
2. Implement `IndexLocation` using `LocationIndexAction`
3. Implement `IndexAll` as either:
   - New `LibraryIndexAllAction`, or
   - Loop dispatching multiple `LocationIndexAction`s <- this one is my fav
4. Keep `Browse` as direct filesystem operation (no action)
5. Remove `QuickScan` command

### Phase 2: Cleanup

1. Remove unused imports and dead code
2. Update documentation
3. Add tests for new actions

## Benefits

1. **Consistency**: All state mutations go through the same system
2. **Auditability**: Every state change is logged
3. **Validation**: Actions validate inputs before execution
4. **Extensibility**: Easy to add pre/post processing to actions
5. **Testability**: Actions can be tested in isolation

## Migration Strategy

1. Implement one handler at a time
2. Keep existing functionality working during migration
3. Test each migrated handler thoroughly
4. Remove old code only after new code is verified

## Future Considerations

### Potential New Actions

> Yes lets make these!

- `LibraryRename` - Rename a library
- `LibraryExport` - Export library metadata
- `LocationRescan` - Currently using direct job dispatch, could be an action
- `DeviceRevoke` - Remove device from network (currently direct)

### Read-Only Operation Patterns

> We can handle this another time

Consider creating a consistent pattern for read operations:

- Standardized query builders
- Consistent error handling
- Pagination support where appropriate

## Success Metrics

1. All state-mutating operations use actions
2. No direct database modifications in handlers
3. Consistent error handling across all handlers
4. Clear separation between read and write operations
5. Improved testability of handlers
