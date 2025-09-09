# CQRS Simplified: Modular Actions & Queries

**Authors:** Gemini, jamespine
**Date:** 2025-01-27
**Status:** **Active Implementation**

## Problem Statement

The current action system has excellent infrastructure (validation, audit logging, error handling) but suffers from a **modularity-breaking centralized `ActionOutput` enum**. Meanwhile, the job system already demonstrates the correct modular approach.

### Current Issues:

1. **ActionOutput Enum Breaks Modularity**: Every new action requires modifying central infrastructure
2. **Missing Query Infrastructure**: No formal system for read operations with validation/logging
3. **Inconsistent API Surface**: Actions vs Queries vs Jobs all have different interfaces
4. **Object Safety Limitations**: `ActionOutputTrait` can't be used as `dyn ActionOutputTrait`

### What Works Well:

- ✅ **Job System**: Already modular with native output types (`ThumbnailOutput`, `IndexerOutput`)
- ✅ **ActionManager**: Excellent validation, audit logging, error handling infrastructure
- ✅ **Individual Action Structs**: Well-designed, self-contained operations

## Solution: Copy Job System Pattern for Actions

Instead of complex trait systems, we **leverage existing infrastructure** and **copy the successful job pattern**.

### 1. Remove ActionOutput Enum (Copy Job Pattern)

**Current (Bad)**:

```rust
// Centralized enum that breaks modularity
pub enum ActionOutput {
    LibraryCreate { id: Uuid, name: String },
    VolumeTrack { fingerprint: VolumeFingerprint },
    // Every new action requires modifying this!
}
```

**Target (Good - Like Jobs)**:

```rust
// No centralized enum! Each action owns its output type
pub trait Action {
    type Output: Send + Sync + 'static;
    // ... existing methods remain the same
}

impl ActionManager {
    // Return native output types directly
    pub async fn dispatch<A: Action>(&self, action: A) -> Result<A::Output> {
        // Use existing validation, audit logging, error handling
        // But return A::Output instead of ActionOutput enum
    }
}
```

### 2. Add QueryManager (Mirror ActionManager)

Create a `QueryManager` that provides the same infrastructure benefits for read operations:

```rust
pub struct QueryManager {
    context: Arc<CoreContext>,
}

pub trait Query {
    type Output: Send + Sync + 'static;

    async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
}

impl QueryManager {
    pub async fn dispatch<Q: Query>(&self, query: Q) -> Result<Q::Output> {
        // Add validation, permissions, logging for queries
        // Same infrastructure pattern as ActionManager
        query.execute(self.context.clone()).await
    }
}
```

### 3. Unified Core Interface

```rust
impl Core {
    /// Execute any action with full infrastructure support
    pub async fn execute_action<A: Action>(&self, action: A) -> Result<A::Output> {
        self.action_manager.dispatch(action).await
    }

    /// Execute any query with full infrastructure support
    pub async fn execute_query<Q: Query>(&self, query: Q) -> Result<Q::Output> {
        self.query_manager.dispatch(query).await
    }
}
```

## Implementation Plan

### Phase 1: Remove ActionOutput Enum ✅ **HIGH IMPACT**

1. **Update ActionManager** to be generic over action output types
2. **Remove ActionOutput enum** entirely
3. **Update all ActionHandler implementations** to return native types
4. **Test that existing actions work** with native outputs

### Phase 2: Add QueryManager ✅ **NEW FUNCTIONALITY**

1. **Create QueryManager** with same infrastructure as ActionManager
2. **Move existing queries** (`ListLibrariesQuery`) to use QueryManager
3. **Add validation, permissions, logging** for query operations

### Phase 3: Unified Core API ✅ **API CONSISTENCY**

1. **Add Core::execute_action/execute_query** methods
2. **Update CLI** to use unified Core API instead of daemon
3. **Update GraphQL** to use unified Core API
4. **Deprecate old direct ActionManager usage**

## Benefits

- ✅ **True Modularity**: Each operation owns its output type (like jobs already do)
- ✅ **Zero Breaking Changes**: Existing ActionHandler logic remains unchanged
- ✅ **Leverages Existing Infrastructure**: Builds on proven ActionManager pattern
- ✅ **Consistent API Surface**: Single entry point for all clients
- ✅ **Performance**: Direct native types, no enum conversion overhead
- ✅ **Simple Implementation**: Copy proven job system pattern

## Key Insight

**CQRS isn't about complex trait systems** - it's about **consistent infrastructure for reads vs writes**. The job system already demonstrates the correct modular approach. We just need to apply the same pattern to actions and add equivalent infrastructure for queries.

This approach is **much simpler** than the previous trait-heavy design while solving the actual modularity problems.
