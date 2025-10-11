# Query Architecture Refactor Plan

## Goal: Consistent Input/Output Pattern for Queries

Currently queries have inconsistent architecture compared to actions. This plan will make them consistent with the clean Input/Output separation pattern.

## Current State Analysis

### Actions (Good Architecture)
```rust
FileCopyInput → FileCopyAction → JobHandle/CustomOutput
```
- **Input**: Clean API contract
- **Action**: Internal execution logic
- **Output**: Clean result data

### Queries (Inconsistent Architecture)

#### Pattern 1: Query Struct Contains Fields
```rust
// Mixed concerns - API fields + execution logic
pub struct JobListQuery {
    pub status: Option<JobStatus>,  // ← API input mixed with query logic
}
```

#### Pattern 2: Query Struct Contains Input (Better)
```rust
// Better separation
pub struct FileSearchQuery {
    pub input: FileSearchInput,  // ← Cleaner!
}
```

## Refactor Plan

### Phase 1: Create Input Structs for All 12 Queries

| Current Query | New Input Struct | Type | Notes |
|--------------|------------------|------|-------|
| `CoreStatusQuery` | `CoreStatusInput` | Core | Empty struct for consistency |
| `JobListQuery` | `JobListInput` | Library | `{ status: Option<JobStatus> }` |
| `JobInfoQuery` | `JobInfoInput` | Library | `{ job_id: JobId }` |
| `LibraryInfoQuery` | `LibraryInfoInput` | Library | `{ library_id: Uuid }` |
| `ListLibrariesQuery` | `ListLibrariesInput` | Core | `{ include_stats: bool }` |
| `GetCurrentLibraryQuery` | `GetCurrentLibraryInput` | Core | Empty struct |
| `LocationsListQuery` | `LocationsListInput` | Library | `{ library_id: Uuid }` |
| `FileSearchQuery` | `FileSearchInput` | Library | Already exists |
| `SearchTagsQuery` | `SearchTagsInput` | Library | Already exists |
| `NetworkStatusQuery` | `NetworkStatusInput` | Core | Empty struct |
| `ListDevicesQuery` | `ListDevicesInput` | Core | Empty struct |
| `PairStatusQuery` | `PairStatusInput` | Core | Empty struct |

### Phase 2: Update Query Struct Implementations

#### Before (Mixed Concerns)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListQuery {
    pub status: Option<JobStatus>,  // ← API field mixed with logic
}

impl Query for JobListQuery {
    type Output = JobListOutput;

    async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
        // Use self.status directly
    }
}

crate::register_query!(JobListQuery, "jobs.list");
```

#### After (Clean Separation)
```rust
// Clean input struct
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListInput {
    pub status: Option<JobStatus>,
}

// Clean query struct
#[derive(Debug, Clone)]
pub struct JobListQuery {
    pub input: JobListInput,
    // Future: internal query state/context could go here
}

impl LibraryQuery for JobListQuery {
    type Input = JobListInput;
    type Output = JobListOutput;

    fn from_input(input: Self::Input) -> Result<Self> {
        Ok(Self { input })
    }

    async fn execute(self, context: Arc<CoreContext>, library_id: Uuid) -> Result<Self::Output> {
        // Use self.input.status
    }
}

crate::register_library_query!(JobListQuery, "jobs.list");
```

### Phase 3: Update QueryManager to Support New Traits

```rust
impl QueryManager {
    /// Dispatch a library query
    pub async fn dispatch_library<Q: LibraryQuery>(&self, query: Q, library_id: Uuid) -> Result<Q::Output> {
        query.execute(self.context.clone(), library_id).await
    }

    /// Dispatch a core query
    pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
        query.execute(self.context.clone()).await
    }
}
```

### Phase 4: Migration Strategy

#### Step 1: Core Queries (4 queries)
- `CoreStatusQuery` → Core (no library context needed)
- `ListLibrariesQuery` → Core (lists all libraries)
- `NetworkStatusQuery` → Core (daemon-level network status)
- `ListDevicesQuery` → Core (daemon-level device list)

#### Step 2: Library Queries (8 queries)
- `JobListQuery` → Library (library-specific jobs)
- `JobInfoQuery` → Library (library-specific job info)
- `LibraryInfoQuery` → Library (specific library info)
- `GetCurrentLibraryQuery` → Core (session state, not library-specific)
- `LocationsListQuery` → Library (library-specific locations)
- `FileSearchQuery` → Library (search within library)
- `SearchTagsQuery` → Library (library-specific tags)
- `PairStatusQuery` → Core (daemon-level pairing status)

## Benefits After Refactor

### **Architectural Consistency**
- Actions and queries follow same Input/Output pattern
- Clean separation of API contract vs execution logic
- Consistent wire protocol handling

### **Better Type Safety**
- Explicit Input types for Swift generation
- Clear distinction between library vs core operations
- Proper type extraction via enhanced registration macros

### **rspc Magic Compatibility**
- All queries will work with automatic type extraction
- Complete Swift API generation for all 12 queries
- Type-safe wire methods and identifiers

## Implementation Order

1. **Create Input structs** for each query
2. **Update query implementations** to use new traits
3. **Change registration macro calls** from `register_query!` to `register_library_query!`/`register_core_query!`
4. **Test complete system** with all 41 operations

This refactor will give us a **clean, consistent architecture** that works perfectly with the rspc-inspired type extraction system!
