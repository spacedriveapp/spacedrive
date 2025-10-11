# Infrastructure Layer Separation of Concerns

**Status**: RFC / Design Document
**Author**: AI Assistant with James Pine
**Date**: 2025-01-07
**Version**: 1.0
**Related**: API_INFRASTRUCTURE_REORGANIZATION.md

## Executive Summary

This document proposes a fundamental architectural fix to the Spacedrive infrastructure layer. Currently, `ApiDispatcher` bypasses both `ActionManager` and `QueryManager`, directly calling operation `.execute()` methods. This causes:

1. **Duplication** - ApiDispatcher reimplements validation, logging, and error handling
2. **Broken abstractions** - ActionManager and QueryManager exist but are unused by the API
3. **Inconsistency** - Internal code paths differ from API code paths
4. **Missing features** - Query infrastructure lags far behind Action infrastructure

This RFC proposes clear separation of concerns across four infrastructure layers, with each layer having a single, well-defined responsibility.

## Current State Analysis

### The Problem: Bypassed Managers

```rust
// Current ApiDispatcher - BYPASSES ActionManager!
impl ApiDispatcher {
    pub async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
        -> ApiResult<A::Output>
    {
        // 1. Permission check (good)
        self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;

        // 2. Validate library exists (redundant with ActionManager)
        let library = self.core_context.get_library(library_id).await
            .ok_or(ApiError::LibraryNotFound { ... })?;

        // 3. Create action from input (redundant with ActionManager)
        let action = A::from_input(input).map_err(|e| ApiError::invalid_input(e))?;

        // 4. DIRECTLY EXECUTE - bypasses ActionManager entirely!
        let result = action.execute(library, self.core_context.clone()).await
            .map_err(ApiError::from)?;

        Ok(result)
    }
}
```

**What's bypassed:**
- `action.validate()` - Never called through API!
- Audit logging in ActionManager - Never happens!
- ActionManager's error handling and logging
- Any future middleware in ActionManager

**Consequence:** ActionManager exists but is only used by internal code, not the API layer!

### Query Infrastructure Gap

```rust
// Current QueryManager - MINIMAL implementation
pub struct QueryManager {
    context: Arc<CoreContext>,
}

impl QueryManager {
    pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
        // Just creates a session and executes - no validation, caching, etc.
        query.execute(self.context.clone(), session).await
    }
}
```

**Missing compared to ActionManager:**
- No validation step
- No query-specific error type (uses `anyhow`)
- No logging/metrics
- No caching layer
- No middleware support
- Not used by ApiDispatcher anyway!

## Architecture Overview

### Current Flow (Broken)

```
┌──────────────────────────────────────────────────────────────┐
│ Client Application (CLI, Swift, GraphQL)                     │
└────────────────────────────┬─────────────────────────────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ Wire Protocol (infra/wire)                                   │
│   • Registry lookup by method string                         │
│   • Deserialize JSON payload to Input                        │
│   • Route to handler function                                │
└────────────────────────────┬─────────────────────────────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ ApiDispatcher (infra/api)                                    │
│   ✓ Check permissions                                        │
│   ✓ Validate session                                         │
│   ✓ Request/response logging                                 │
│   Validates library exists (REDUNDANT)                    │
│   Calls action.execute() DIRECTLY (BYPASSES MANAGER)      │
│   Reimplements error handling                             │
└────────────────────────────┬─────────────────────────────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ ActionManager (BYPASSED)                                  │
│   • action.validate() - NEVER CALLED                         │
│   • Audit logging - NEVER HAPPENS                            │
│   • Result tracking - SKIPPED                                │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ QueryManager (ALSO BYPASSED)                              │
│   • Would validate - NEVER CALLED                            │
│   • Would cache - DOESN'T EXIST                              │
└──────────────────────────────────────────────────────────────┘
```

### Proposed Flow (Fixed)

```
┌──────────────────────────────────────────────────────────────┐
│ Client Application (CLI, Swift, GraphQL)                     │
└────────────────────────────┬─────────────────────────────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ Layer 1: Wire Protocol (infra/wire)                          │
│   RESPONSIBILITY: RPC infrastructure                         │
│   • Registry & method routing                                │
│   • Serialization/deserialization                            │
│   • Type generation for clients                              │
└────────────────────────────┬─────────────────────────────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ Layer 2: API Orchestration (infra/api)                       │
│   RESPONSIBILITY: Cross-cutting concerns for ALL operations  │
│   • Session management & authentication                      │
│   • Permission checks & authorization                        │
│   • Middleware pipeline (logging, metrics, rate limiting)    │
│   • Error transformation (internal → API errors)             │
│   • DELEGATES to operation-specific managers              │
└────────────────────────────┬─────────────────────────────────┘
                             │
                 ┌───────────┴───────────┐
                 │                       │
                 ↓                       ↓
┌─────────────────────────────┐ ┌──────────────────────────────┐
│ Layer 3A: Action Manager    │ │ Layer 3B: Query Manager      │
│ (infra/action)               │ │ (infra/query)                │
│ RESPONSIBILITY:              │ │ RESPONSIBILITY:              │
│   Action-specific infra      │ │   Query-specific infra       │
│   • Validation               │ │   • Validation               │
│   • Audit logging            │ │   • Result caching           │
│   • Result tracking          │ │   • Query optimization       │
│   • Action-specific errors   │ │   • Query-specific errors    │
└─────────────────────────────┘ └──────────────────────────────┘
                 │                       │
                 └───────────┬───────────┘
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│ Layer 4: Business Logic (ops/)                               │
│   RESPONSIBILITY: Actual operation implementation            │
│   • Domain logic                                             │
│   • Database queries                                         │
│   • File system operations                                   │
│   • Business rules                                           │
└──────────────────────────────────────────────────────────────┘
```

## Layer Responsibilities (Detailed)

### Layer 1: Wire Protocol (`infra/wire/`)

**Purpose**: RPC infrastructure - how operations are exposed over the wire

**Responsibilities:**
- Method string registration and routing (`"action:files.copy.v1"`)
- JSON/Bincode serialization/deserialization
- Type generation for clients (Swift, TypeScript)
- Wire method → handler function mapping

**NOT Responsible For:**
- Business logic
- Permissions/authentication
- Validation
- Logging (beyond basic RPC logging)

**Key Files:**
- `registry.rs` - Method registration and routing
- `type_extraction.rs` - Client type generation
- `api_types.rs` - Wire-compatible type wrappers

**Example:**
```rust
// Registry handler - thin wrapper that routes to API layer
pub fn handle_library_action<A>(
    context: Arc<CoreContext>,
    session: SessionContext,
    payload: serde_json::Value,
) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
{
    Box::pin(async move {
        // Deserialize input
        let input: A::Input = serde_json::from_value(payload)?;

        // Delegate to API layer (NOT directly to manager)
        let dispatcher = ApiDispatcher::new(context);
        let output = dispatcher.execute_library_action::<A>(input, session).await?;

        // Serialize output
        serde_json::to_value(output)
    })
}
```

### Layer 2: API Orchestration (`infra/api/`)

**Purpose**: Cross-cutting concerns that apply to ALL operations (both actions and queries)

**Responsibilities:**
- Session management and validation
- Authentication (who is making the request?)
- Authorization/permissions (are they allowed?)
- Middleware pipeline (logging, metrics, rate limiting)
- Error transformation (internal errors → API errors)
- Request/response metadata (request IDs, timestamps)
- **Delegates to operation-specific managers**

**NOT Responsible For:**
- Operation-specific validation (that's in managers)
- Audit logging (that's action-specific)
- Query caching (that's query-specific)
- Business logic

**Key Files:**
- `dispatcher.rs` - Main API entry point (delegates to managers)
- `session.rs` - Session context and authentication
- `permissions.rs` - Permission checking
- `middleware.rs` - Middleware pipeline
- `error.rs` - API error types

**Current (Wrong):**
```rust
impl ApiDispatcher {
    pub async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
        -> ApiResult<A::Output>
    {
        // Cross-cutting concerns (CORRECT - API layer's job)
        self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;

        // Get library (WRONG - duplicates ActionManager logic)
        let library = self.core_context.get_library(library_id).await?;

        // Create action (WRONG - duplicates ActionManager logic)
        let action = A::from_input(input)?;

        // Execute directly (WRONG - bypasses ActionManager)
        action.execute(library, self.core_context.clone()).await?
    }
}
```

**Proposed (Correct):**
```rust
impl ApiDispatcher {
    pub async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
        -> ApiResult<A::Output>
    {
        // 1. Cross-cutting concerns (API layer's responsibility)
        self.middleware_pipeline.process(&session, "action", async {
            // 2. Permission check (API layer's responsibility)
            self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;

            // 3. Create action from input
            let action = A::from_input(input)
                .map_err(|e| ApiError::InvalidInput { details: e })?;

            // 4. DELEGATE to ActionManager (action-specific infrastructure)
            let action_manager = ActionManager::new(self.core_context.clone());
            let result = action_manager
                .dispatch_library(session.current_library_id, action)
                .await
                .map_err(ApiError::from)?;

            Ok(result)
        }).await
    }
}
```

### Layer 3A: Action Manager (`infra/action/`)

**Purpose**: Action-specific infrastructure concerns

**Responsibilities:**
- Library/resource validation and lookup
- Action validation (`action.validate()`)
- Audit logging (track mutations)
- Action-specific error handling
- Result tracking and receipts
- Action context tracking (who/what/when)

**NOT Responsible For:**
- Permissions (that's cross-cutting - API layer)
- Wire protocol (that's Layer 1)
- Business logic (that's Layer 4)

**Key Files:**
- `manager.rs` - ActionManager orchestration
- `context.rs` - Action context tracking
- `error.rs` - Action-specific errors
- `receipt.rs` - Action execution receipts
- `mod.rs` - Action trait definitions

**Current (Bypassed):**
```rust
impl ActionManager {
    pub async fn dispatch_library<A: LibraryAction>(
        &self,
        library_id: Option<Uuid>,
        action: A,
    ) -> Result<A::Output, ActionError> {
        // Get library (action-specific validation)
        let library = self.context.get_library(library_id.unwrap())
            .ok_or(ActionError::LibraryNotFound(library_id))?;

        // Create audit log entry (action-specific)
        let audit_entry = self.create_action_audit_log(library_id, action.action_kind()).await?;

        // Validate the action (action-specific)
        action.validate(library.clone(), self.context.clone()).await?;

        // Execute action
        let result = action.execute(library, self.context.clone()).await;

        // Finalize audit log (action-specific)
        self.finalize_audit_log(audit_entry, &result, library_id).await?;

        result
    }
}
```

**This is CORRECT - but it's bypassed by ApiDispatcher!**

### Layer 3B: Query Manager (`infra/query/`)

**Purpose**: Query-specific infrastructure concerns (CURRENTLY MINIMAL)

**Responsibilities (Proposed):**
- Library/resource validation and lookup
- Query validation (`query.validate()`)
- Result caching (for expensive queries)
- Query-specific error handling
- Query optimization hints
- Query context tracking

**NOT Responsible For:**
- Permissions (that's cross-cutting - API layer)
- Wire protocol (that's Layer 1)
- Business logic (that's Layer 4)

**Key Files:**
- `mod.rs` - Query traits and QueryManager
- `error.rs` - Query-specific errors (TO BE CREATED)
- `cache.rs` - Query result caching (TO BE CREATED)
- `context.rs` - Query context tracking (TO BE CREATED)

**Current (Minimal):**
```rust
impl QueryManager {
    pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
        // Just create session and execute - no validation, caching, etc.
        let device_id = self.context.device_manager.device_id()?;
        let session = SessionContext::device_session(device_id, "Core Device".to_string());
        query.execute(self.context.clone(), session).await
    }
}
```

**Proposed (Enhanced):**
```rust
impl QueryManager {
    pub async fn dispatch_core<Q: CoreQuery>(
        &self,
        query: Q,
        session: SessionContext,
    ) -> Result<Q::Output, QueryError> {
        let query_type = std::any::type_name::<Q>();

        // 1. Check cache first (query-specific)
        if let Some(cached) = self.cache.get::<Q>(&query).await {
            tracing::debug!("Cache hit for query: {}", query_type);
            return Ok(cached);
        }

        // 2. Validate the query (query-specific)
        query.validate(self.context.clone()).await?;

        // 3. Execute query
        tracing::info!("Executing query: {}", query_type);
        let start = Instant::now();

        let result = query.execute(self.context.clone(), session).await?;

        let duration = start.elapsed();
        tracing::info!("Query {} completed in {:?}", query_type, duration);

        // 4. Cache result if appropriate (query-specific)
        if Q::is_cacheable() {
            self.cache.set(&query, &result).await;
        }

        Ok(result)
    }
}
```

### Layer 4: Business Logic (`ops/`)

**Purpose**: Actual operation implementations

**Responsibilities:**
- Domain logic and business rules
- Database queries and updates
- File system operations
- External service calls
- Data transformations

**NOT Responsible For:**
- Permissions (that's API layer)
- Audit logging (that's ActionManager)
- Caching (that's QueryManager)
- Wire protocol (that's Layer 1)

**Key Files:**
- `ops/files/copy/action.rs` - File copy implementation
- `ops/files/query/directory_listing.rs` - Directory listing implementation
- etc.

## Design Principles

### 1. Single Responsibility

Each layer has ONE clear responsibility:
- **Wire**: RPC mechanics
- **API**: Cross-cutting orchestration
- **Manager**: Operation-specific infrastructure
- **Ops**: Business logic

### 2. Delegation Not Duplication

Higher layers delegate to lower layers - they don't reimplement:
- ApiDispatcher should NOT validate library exists (ActionManager does that)
- ApiDispatcher should NOT create audit logs (ActionManager does that)
- ApiDispatcher SHOULD check permissions (that's cross-cutting)
- ApiDispatcher SHOULD delegate to managers

### 3. Layered Architecture

```
┌─────────────────────────────────────────────┐
│  Each layer only knows about the layer      │
│  immediately below it                        │
└─────────────────────────────────────────────┘

Wire → API → Manager → Ops

Wire CAN'T call Ops directly
API CAN'T call Ops directly
Everyone goes through their immediate neighbor
```

### 4. Symmetry

Actions and Queries should have symmetric infrastructure:
- Both have managers
- Both have validation
- Both have error types
- Both have context tracking
- Different concerns (audit vs cache) but same structure

## Implementation Plan

### Phase 1: Enhance Query Infrastructure

**Goal**: Bring QueryManager to parity with ActionManager

1. Create `infra/query/error.rs` - Query-specific errors
2. Add validation to Query traits
3. Enhance QueryManager with:
   - Validation step
   - Logging/metrics
   - Error handling
4. Create `infra/query/context.rs` - Query context tracking

**Files to Create:**
- `core/src/infra/query/error.rs`
- `core/src/infra/query/context.rs`
- `core/src/infra/query/cache.rs` (optional Phase 2)

**Files to Modify:**
- `core/src/infra/query/mod.rs` - Add validation to traits

### Phase 2: Fix ApiDispatcher Delegation

**Goal**: Make ApiDispatcher delegate to managers instead of bypassing them

**Changes to `infra/api/dispatcher.rs`:**

```rust
// BEFORE: ApiDispatcher bypasses managers
pub async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
    -> ApiResult<A::Output>
{
    self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;
    let library = self.core_context.get_library(library_id).await?;
    let action = A::from_input(input)?;
    action.execute(library, self.core_context.clone()).await? // BYPASS
}

// AFTER: ApiDispatcher delegates to ActionManager
pub async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
    -> ApiResult<A::Output>
{
    // 1. Cross-cutting: permissions
    self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;

    // 2. Cross-cutting: middleware
    self.middleware.process(&session, "action", async {
        // 3. Create action
        let action = A::from_input(input)
            .map_err(|e| ApiError::InvalidInput { details: e })?;

        // 4. DELEGATE to ActionManager
        let action_manager = ActionManager::new(self.core_context.clone());
        action_manager
            .dispatch_library(session.current_library_id, action)
            .await
            .map_err(ApiError::from)
    }).await
}
```

**Similar changes for:**
- `execute_core_action()`
- `execute_library_query()`
- `execute_core_query()`

### Phase 3: Update Wire Registry Handlers

**Goal**: Ensure wire handlers use ApiDispatcher correctly

**No changes needed** - wire handlers already delegate to ApiDispatcher.

### Phase 4: Add Query Validation to All Queries

**Goal**: Implement `validate()` method on all query implementations

**Example:**
```rust
impl LibraryQuery for DirectoryListingQuery {
    type Input = DirectoryListingInput;
    type Output = DirectoryListingOutput;

    fn from_input(input: Self::Input) -> Result<Self> {
        Ok(Self { path: input.path })
    }

    // NEW: Validation step
    async fn validate(
        &self,
        library: Arc<Library>,
        context: Arc<CoreContext>,
    ) -> Result<(), QueryError> {
        // Validate path exists, is within library bounds, etc.
        if !self.path.is_within_library_bounds() {
            return Err(QueryError::InvalidPath { path: self.path.clone() });
        }
        Ok(())
    }

    async fn execute(
        self,
        context: Arc<CoreContext>,
        session: SessionContext,
    ) -> Result<Self::Output> {
        // Business logic here
    }
}
```

### Phase 5: Testing and Validation

1. **Unit tests** for each manager's delegation logic
2. **Integration tests** ensuring full flow works
3. **Audit tests** verifying ActionManager audit logs are created
4. **Cache tests** for QueryManager caching (if implemented)

## Error Handling Strategy

### Error Type Hierarchy

```
┌──────────────────────────────────────────────┐
│ ApiError (infra/api/error.rs)               │
│ - User-facing errors                         │
│ - Serializable over wire                     │
│ - Maps from internal errors                  │
└──────────────────┬───────────────────────────┘
                   │
        ┌──────────┴──────────┐
        │                     │
        ↓                     ↓
┌───────────────┐    ┌────────────────┐
│ ActionError   │    │ QueryError     │
│ (infra/action)│    │ (infra/query)  │
│ - Action-     │    │ - Query-       │
│   specific    │    │   specific     │
└───────────────┘    └────────────────┘
```

### Error Conversion

```rust
// ActionError → ApiError
impl From<ActionError> for ApiError {
    fn from(err: ActionError) -> Self {
        match err {
            ActionError::LibraryNotFound(id) =>
                ApiError::LibraryNotFound { library_id: id.to_string() },
            ActionError::PermissionDenied { action, reason } =>
                ApiError::InsufficientPermissions { reason },
            ActionError::Internal(msg) =>
                ApiError::InternalError { message: msg },
            // etc.
        }
    }
}

// QueryError → ApiError
impl From<QueryError> for ApiError {
    fn from(err: QueryError) -> Self {
        match err {
            QueryError::LibraryNotFound(id) =>
                ApiError::LibraryNotFound { library_id: id.to_string() },
            QueryError::InvalidInput(msg) =>
                ApiError::InvalidInput { details: msg },
            QueryError::CacheMiss =>
                ApiError::InternalError { message: "Cache miss".into() },
            // etc.
        }
    }
}
```

## Benefits of This Architecture

### 1. Clear Separation of Concerns

Each layer has a single, well-defined purpose:
- Wire = RPC mechanics
- API = Cross-cutting orchestration
- Manager = Operation-specific infrastructure
- Ops = Business logic

### 2. No Duplication

- ApiDispatcher doesn't reimplement validation (delegates to managers)
- ApiDispatcher doesn't reimplement audit logging (ActionManager does it)
- Wire registry doesn't know about permissions (API layer does it)

### 3. Consistent Behavior

- All code paths (API, internal, CLI, Swift) go through the same managers
- Validation always runs
- Audit logging always happens
- No "backdoors" that bypass infrastructure

### 4. Testability

- Each layer can be tested independently
- Managers can be tested without wire protocol
- Business logic can be tested without API layer
- Mock layers easily

### 5. Extensibility

- Add middleware to API layer → affects all operations
- Add caching to QueryManager → affects all queries
- Add validation rules → happens in one place
- Add audit requirements → happens in ActionManager

### 6. Symmetry

- Actions and Queries have parallel infrastructure
- Easy to understand: "Just like actions, but for reads"
- Consistent patterns across codebase

## Migration Checklist

### Pre-Migration
- [ ] Read and approve this design document
- [ ] Review current code paths and identify all bypass points
- [ ] Create feature branch for migration

### Phase 1: Query Infrastructure
- [ ] Create `infra/query/error.rs` with `QueryError` type
- [ ] Create `infra/query/context.rs` with `QueryContext` type
- [ ] Add `validate()` method to `CoreQuery` trait
- [ ] Add `validate()` method to `LibraryQuery` trait
- [ ] Enhance `QueryManager` with validation, logging, error handling
- [ ] Write unit tests for QueryManager

### Phase 2: ApiDispatcher Delegation
- [ ] Update `execute_library_action()` to delegate to ActionManager
- [ ] Update `execute_core_action()` to delegate to ActionManager
- [ ] Update `execute_library_query()` to delegate to QueryManager
- [ ] Update `execute_core_query()` to delegate to QueryManager
- [ ] Remove duplicate validation logic from ApiDispatcher
- [ ] Write integration tests for full flow

### Phase 3: Query Implementations
- [ ] Add `validate()` implementation to all LibraryQuery implementations
- [ ] Add `validate()` implementation to all CoreQuery implementations
- [ ] Update query error handling to use `QueryError`

### Phase 4: Testing
- [ ] Run full test suite
- [ ] Verify audit logs are created through API
- [ ] Verify validation runs through API
- [ ] Test error propagation end-to-end
- [ ] Performance testing

### Phase 5: Documentation
- [ ] Update `AGENTS.md` with new architecture
- [ ] Update `/docs/core/daemon.md` with flow diagrams
- [ ] Add code examples to documentation
- [ ] Update inline code comments

## Future Enhancements

### Query Caching (Phase 2+)

```rust
// infra/query/cache.rs
pub struct QueryCache {
    cache: Arc<RwLock<HashMap<QueryKey, CachedResult>>>,
}

impl QueryCache {
    pub async fn get<Q: CoreQuery>(&self, query: &Q) -> Option<Q::Output> {
        let key = QueryKey::from_query(query);
        self.cache.read().await.get(&key).cloned()
    }

    pub async fn set<Q: CoreQuery>(&self, query: &Q, result: &Q::Output) {
        let key = QueryKey::from_query(query);
        self.cache.write().await.insert(key, result.clone());
    }
}
```

### Middleware Pipeline Enhancement

```rust
// infra/api/middleware.rs
pub struct MiddlewarePipeline {
    middlewares: Vec<Box<dyn ApiMiddleware>>,
}

impl MiddlewarePipeline {
    pub fn new() -> Self {
        Self {
            middlewares: vec![
                Box::new(LoggingMiddleware),
                Box::new(MetricsMiddleware),
                Box::new(RateLimitMiddleware),
            ],
        }
    }

    pub async fn process<F, T>(&self, session: &SessionContext, op_name: &str, next: F)
        -> ApiResult<T>
    where
        F: FnOnce() -> Future<Output = ApiResult<T>>,
    {
        // Chain middlewares recursively
        // ...
    }
}
```

### Query Optimization Hints

```rust
pub trait LibraryQuery {
    // ...

    fn optimization_hints(&self) -> QueryOptimization {
        QueryOptimization::default()
    }
}

pub struct QueryOptimization {
    pub cacheable: bool,
    pub cache_duration: Option<Duration>,
    pub eager_load: Vec<String>, // Relations to eager load
    pub index_hints: Vec<String>, // Database index hints
}
```

## Known Technical Debt: Error Type Duplication

### The Problem

Currently, `ApiError`, `ActionError`, and `QueryError` share many identical variants:

```rust
// Duplicated across all three:
- LibraryNotFound(Uuid)
- InvalidInput(String)
- Validation { field, message }
- Timeout
- Database(String)
- FileSystem { path, error }
- Internal(String)
```

This violates DRY (Don't Repeat Yourself) and creates maintenance burden when adding new error types.

### Why We Accept It (For Now)

During this refactoring, we prioritize:
1. **Clear layer boundaries** - Each error type is specific to its layer
2. **Type safety** - Can't accidentally mix layer-specific errors
3. **Independent evolution** - Layers can change without affecting others
4. **Getting the architecture right first** - Error consolidation can come later

### Future Improvement: Shared CoreErrorKind

**Proposed Solution** (post-refactoring):

```rust
// New: core/src/common/error_kinds.rs
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreErrorKind {
    #[error("Library {0} not found")]
    LibraryNotFound(Uuid),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Validation error for field '{field}': {message}")]
    Validation { field: String, message: String },

    #[error("Operation timed out")]
    Timeout,

    #[error("Database error: {0}")]
    Database(String),

    #[error("File system error at '{path}': {error}")]
    FileSystem { path: String, error: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

// Then each layer wraps it:
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    Core(#[from] CoreErrorKind),

    // Action-specific only:
    #[error("Job error: {0}")]
    Job(#[from] JobError),

    #[error("Permission denied for action '{action}': {reason}")]
    PermissionDenied { action: String, reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Core(#[from] CoreErrorKind),

    // Query-specific only:
    #[error("Cache error: {0}")]
    Cache(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Core(#[from] CoreErrorKind),

    // API-specific only:
    #[error("Authentication required")]
    Unauthenticated,

    #[error("Rate limit exceeded: {retry_after_seconds}s")]
    RateLimitExceeded { retry_after_seconds: u64 },
}
```

**Benefits:**
- Single source of truth for common errors
- Easy to add new common error types
- Layer-specific errors remain separate
- Clear distinction between shared and layer-specific concerns

**Implementation Plan:**
1. Complete current refactoring with duplicated errors
2. Create `CoreErrorKind` in `core/src/common/error_kinds.rs`
3. Migrate `ActionError`, `QueryError`, `ApiError` to wrap `CoreErrorKind`
4. Update error conversions to handle the wrapper
5. Update all error construction sites

**Estimated Effort:** 2-4 hours for full migration after current refactoring is stable.

## Conclusion

This architecture provides clear separation of concerns across the infrastructure layer:

1. **Wire Layer**: RPC mechanics only
2. **API Layer**: Cross-cutting orchestration (sessions, permissions, middleware)
3. **Manager Layer**: Operation-specific infrastructure (validation, audit, cache)
4. **Business Layer**: Actual operation implementation

By fixing the bypass problem and bringing Query infrastructure to parity with Action infrastructure, we create a consistent, maintainable, and extensible foundation for all Spacedrive operations.

The key insight: **Managers should orchestrate operations, not be bypassed by the API layer**.

## Appendix: Code Examples

### Complete Flow Example

```rust
// 1. CLIENT MAKES REQUEST
let response = client.call("action:files.copy.v1", {
    source: "/path/to/source",
    destination: "/path/to/dest"
});

// 2. WIRE LAYER - Registry handler
pub fn handle_library_action<FileCopyAction>(
    context: Arc<CoreContext>,
    session: SessionContext,
    payload: Value,
) -> Result<Value> {
    let input = serde_json::from_value(payload)?;
    let dispatcher = ApiDispatcher::new(context);
    let output = dispatcher.execute_library_action::<FileCopyAction>(input, session).await?;
    Ok(serde_json::to_value(output)?)
}

// 3. API LAYER - Cross-cutting concerns
impl ApiDispatcher {
    async fn execute_library_action<A>(&self, input: A::Input, session: SessionContext)
        -> ApiResult<A::Output>
    {
        // Cross-cutting: permissions
        self.permission_layer.check_library_action::<A>(&session, PhantomData).await?;

        // Cross-cutting: middleware
        self.middleware.process(&session, "action", async {
            // Create action
            let action = A::from_input(input)?;

            // DELEGATE to manager
            let manager = ActionManager::new(self.core_context.clone());
            manager.dispatch_library(session.current_library_id, action).await
        }).await
    }
}

// 4. MANAGER LAYER - Action-specific infrastructure
impl ActionManager {
    async fn dispatch_library<A>(&self, library_id: Uuid, action: A)
        -> Result<A::Output, ActionError>
    {
        // Action-specific: Get library
        let library = self.context.get_library(library_id)
            .ok_or(ActionError::LibraryNotFound(library_id))?;

        // Action-specific: Create audit log
        let audit = self.create_audit_log(library_id, action.action_kind()).await?;

        // Action-specific: Validate
        action.validate(library.clone(), self.context.clone()).await?;

        // Execute business logic
        let result = action.execute(library, self.context.clone()).await;

        // Action-specific: Finalize audit
        self.finalize_audit_log(audit, &result).await?;

        result
    }
}

// 5. BUSINESS LAYER - Actual implementation
impl LibraryAction for FileCopyAction {
    async fn validate(&self, library: Arc<Library>, ctx: Arc<CoreContext>)
        -> Result<(), ActionError>
    {
        // Business validation: source exists, destination valid, etc.
        if !self.source.exists() {
            return Err(ActionError::FileSystem {
                path: self.source.to_string(),
                error: "Source file not found".into()
            });
        }
        Ok(())
    }

    async fn execute(self, library: Arc<Library>, ctx: Arc<CoreContext>)
        -> Result<JobHandle, ActionError>
    {
        // Actual business logic: create job, copy files, etc.
        let job = FileCopyJob::new(self.source, self.destination);
        let handle = ctx.job_manager.spawn(job).await?;
        Ok(handle)
    }
}
```

This shows the complete flow with proper delegation at each layer!
