<!--CREATED: 2025-10-11-->
# API Infrastructure Reorganization

**Status**: RFC / Design Document
**Author**: AI Assistant with James Pine
**Date**: 2025-01-07
**Version**: 1.0

## Executive Summary

This document proposes a reorganization of Spacedrive's API infrastructure to improve code organization, discoverability, and maintainability. The core issue is that infrastructure concerns (queries, actions, registry, type extraction) are currently scattered across multiple directories with inconsistent naming and hierarchy.

## Current State Analysis

### Directory Structure

```
src/
├── cqrs.rs                      # Query traits (CoreQuery, LibraryQuery, QueryManager)
├── client/
│   └── mod.rs                   # Wire trait for client-daemon communication
├── ops/
│   ├── registry.rs              # Registration macros and inventory system
│   ├── type_extraction.rs       # Specta-based type generation
│   ├── api_types.rs            # API type wrappers
│   └── [feature modules]/       # Business logic (files/, libraries/, etc.)
└── infra/
    ├── action/                  # Action traits and infrastructure
    │   ├── mod.rs               # CoreAction, LibraryAction
    │   ├── manager.rs
    │   ├── builder.rs
    │   └── ...
    ├── api/                     # API dispatcher, sessions, permissions
    ├── daemon/                  # Daemon server and RPC
    ├── db/                      # Database layer
    ├── job/                     # Job system
    └── event/                   # Event bus
```

### Key Components

| Component | Location | Lines | Purpose |
|-----------|----------|-------|---------|
| Query Traits | `src/cqrs.rs` | 115 | `CoreQuery`, `LibraryQuery`, `QueryManager` |
| Action Traits | `src/infra/action/mod.rs` | 114 | `CoreAction`, `LibraryAction` |
| Registry System | `src/ops/registry.rs` | 484 | Registration macros, handler functions, inventory |
| Type Extraction | `src/ops/type_extraction.rs` | 698 | Specta type generation for Swift/TypeScript |
| API Dispatcher | `src/infra/api/dispatcher.rs` | 297 | Unified API entry point |
| Wire Trait | `src/client/mod.rs` | 83 | Type-safe client communication |

## Problems Identified

### 1. Misleading Name: "CQRS"

**Problem**: The file `cqrs.rs` contains only the Query side of CQRS (Command Query Responsibility Segregation), not both Command and Query. The "Command" side is in `infra/action/`.

**Impact**:
- Confusing for new contributors
- Suggests a complete CQRS implementation when it's only half
- Doesn't reflect actual contents

### 2. Separation of Counterparts

**Problem**: Actions and Queries are fundamental counterparts in our architecture, but they're separated:
- Actions: `src/infra/action/` (complete module with 8 files)
- Queries: `src/cqrs.rs` (single file at root level)

**Why This Matters**:
- Both are infrastructure traits that operations implement
- Both have parallel concepts (Core vs Library scope)
- Both are used together in the registry and type extraction systems
- They should be co-located for discoverability and maintainability

### 3. Registry/Type System in Wrong Layer

**Problem**: `registry.rs` and `type_extraction.rs` live in `src/ops/` but are infrastructure concerns:

- **Registry System**: Orchestrates the wire protocol, maps method strings to handlers, manages compile-time registration via `inventory` crate
- **Type Extraction**: Generates client types using Specta, builds API metadata for code generation
- **These are NOT business logic** - they're plumbing that connects clients to operations

**Current Confusion**:
```
src/ops/
├── registry.rs              # Infrastructure: wire protocol
├── type_extraction.rs       # Infrastructure: code generation
├── api_types.rs            # Infrastructure: type wrappers
└── files/
    └── copy/
        └── action.rs        # Business logic: copy operation
```

The registry and type extraction files are in `ops/` alongside business logic, but they're fundamentally different in nature.

## Architecture Overview

### The Wire Protocol System

Our system has a sophisticated wire protocol for client-daemon communication:

```
┌─────────────────────────────────────────────────────────────┐
│ Client Application (CLI, Swift, GraphQL)                    │
│   • Uses Wire trait with METHOD constant                   │
│   • Serializes input to JSON                               │
└─────────────────────────────────────────────────────────────┘
                           ↓ Unix Socket
┌─────────────────────────────────────────────────────────────┐
│ Daemon RPC Server (infra/daemon/rpc.rs)                     │
│   • Receives DaemonRequest { method, library_id, payload } │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│ Registry Lookup (ops/registry.rs)                           │
│   • LIBRARY_QUERIES map: method → handler function         │
│   • LIBRARY_ACTIONS map: method → handler function         │
│   • Uses inventory crate for compile-time registration     │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│ Handler Function (handle_library_query<Q>)                  │
│   • Deserializes payload to Q::Input                       │
│   • Creates ApiDispatcher                                  │
│   • Calls execute_library_query::<Q>                       │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│ API Dispatcher (infra/api/dispatcher.rs)                    │
│   • Session validation                                      │
│   • Permission checks                                       │
│   • Library lookup                                          │
│   • Calls Q::execute()                                     │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│ Business Logic (ops/files/query/directory_listing.rs)       │
│   • Actual query implementation                            │
│   • Returns Output                                          │
└─────────────────────────────────────────────────────────────┘
```

### Type Generation System

We use Specta to generate client types automatically:

```rust
// Registration macro implements Wire trait and submits to inventory
crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");

// This generates:
// 1. Wire::METHOD = "query:files.directory_listing.v1"
// 2. Registry entry for runtime dispatch
// 3. Type extractor for compile-time generation
```

The type extraction system (`type_extraction.rs`) collects all registered operations and generates:
- TypeScript types for web/desktop clients
- Swift types for iOS/macOS clients
- API structure metadata

## Proposed Solution: Option A

### New Directory Structure

```
src/infra/
├── action/                  # Command side (state-changing operations)
│   ├── mod.rs              # CoreAction, LibraryAction traits
│   ├── builder.rs
│   ├── manager.rs
│   ├── context.rs
│   ├── error.rs
│   ├── output.rs
│   └── receipt.rs
├── query/                   # Query side (read-only operations)  [NEW]
│   ├── mod.rs              # CoreQuery, LibraryQuery traits, Query trait
│   └── manager.rs          # QueryManager
├── wire/                    # Wire protocol & type system        [NEW]
│   ├── mod.rs              # Re-exports and module docs
│   ├── registry.rs         # Registration macros, inventory, handler functions
│   ├── type_extraction.rs  # Specta-based type generation
│   ├── api_types.rs        # API type wrappers (ApiJobHandle, etc.)
│   └── client.rs           # Wire trait (optional: could stay in src/client/)
├── api/                     # API layer (no changes)
│   ├── dispatcher.rs
│   ├── session.rs
│   ├── permissions.rs
│   └── ...
├── daemon/                  # Daemon server (no changes)
│   ├── rpc.rs
│   ├── dispatch.rs
│   └── ...
├── db/                      # Database layer (no changes)
├── job/                     # Job system (no changes)
├── event/                   # Event bus (no changes)
└── mod.rs
```

### Benefits

#### 1. Clear Semantic Grouping

- **`infra/action/`**: Everything about state-changing operations
- **`infra/query/`**: Everything about read operations
- **`infra/wire/`**: Everything about the wire protocol and type system

Each directory has a clear, single responsibility.

#### 2. Action/Query Symmetry

```
infra/
├── action/        # Commands - state changes
└── query/         # Queries - reads
```

Both are peers at the same level, making their relationship obvious. They're both infrastructure traits that operations implement.

#### 3. Infrastructure vs Business Logic Separation

```
src/
├── infra/         # Technical plumbing (HOW we execute operations)
│   ├── action/
│   ├── query/
│   ├── wire/
│   └── ...
└── ops/           # Business logic (WHAT operations we support)
    ├── files/
    ├── libraries/
    └── ...
```

Clear separation of concerns. If you're working on business logic, you're in `ops/`. If you're working on infrastructure, you're in `infra/`.

#### 4. Improved Discoverability

New contributors can easily understand:
- "Where do I find query-related infrastructure?" → `infra/query/`
- "Where do I find action-related infrastructure?" → `infra/action/`
- "Where's the wire protocol stuff?" → `infra/wire/`
- "Where do I add a new file copy feature?" → `ops/files/copy/`

#### 5. Better Naming

- `cqrs.rs` - Misleading, suggests complete CQRS implementation
- `infra/query/` - Clear, accurate, matches `action/`

### Design Principles Applied

1. **Co-location**: Related code should live together
2. **Symmetry**: Counterparts should be at the same level (action/query)
3. **Clear Boundaries**: Infrastructure vs business logic
4. **Single Responsibility**: Each directory has one clear purpose
5. **Discoverability**: Easy to find what you're looking for

## Migration Plan

### Phase 1: Create New Structure

1. Create new directories:
   ```bash
   mkdir -p src/infra/query
   mkdir -p src/infra/wire
   ```

2. Move query files:
   ```bash
   # Query system
   git mv src/cqrs.rs src/infra/query/mod.rs
   ```

3. Move wire protocol files:
   ```bash
   # Wire protocol and type system
   git mv src/ops/registry.rs src/infra/wire/registry.rs
   git mv src/ops/type_extraction.rs src/infra/wire/type_extraction.rs
   git mv src/ops/api_types.rs src/infra/wire/api_types.rs
   ```

### Phase 2: Update Module Declarations

1. **`src/infra/mod.rs`**
   ```rust
   pub mod action;
   pub mod api;
   pub mod daemon;
   pub mod db;
   pub mod event;
   pub mod job;
   pub mod query;  // NEW
   pub mod wire;   // NEW
   ```

2. **`src/infra/query/mod.rs`** (was `src/cqrs.rs`)
   - No changes to file contents
   - Just moved location

3. **`src/infra/wire/mod.rs`** (new file)
   ```rust
   //! Wire protocol and type system
   //!
   //! This module contains the infrastructure for client-daemon communication:
   //! - Registration system using the `inventory` crate
   //! - Type extraction using Specta for code generation
   //! - Handler functions that route requests to operations
   //! - API type wrappers for client compatibility

   pub mod api_types;
   pub mod registry;
   pub mod type_extraction;

   // Re-export commonly used items
   pub use api_types::{ApiJobHandle, ToApiType};
   pub use registry::{
       handle_core_action, handle_core_query,
       handle_library_action, handle_library_query,
       CoreActionEntry, CoreQueryEntry,
       LibraryActionEntry, LibraryQueryEntry,
       CORE_ACTIONS, CORE_QUERIES,
       LIBRARY_ACTIONS, LIBRARY_QUERIES,
   };
   pub use type_extraction::{
       generate_spacedrive_api, create_spacedrive_api_structure,
       OperationTypeInfo, QueryTypeInfo,
       OperationScope, QueryScope,
   };
   ```

### Phase 3: Update Import Paths

All files that import from moved modules need updates:

#### Files importing `cqrs`:
```rust
// Before
use crate::cqrs::{CoreQuery, LibraryQuery, QueryManager};

// After
use crate::infra::query::{CoreQuery, LibraryQuery, QueryManager};
```

**Files to update:**
- `src/lib.rs`
- `src/context.rs`
- `src/infra/api/dispatcher.rs`
- `src/ops/registry.rs` → `src/infra/wire/registry.rs`
- All query implementations in `src/ops/*/query.rs`

#### Files importing `ops::registry`:
```rust
// Before
use crate::ops::registry::{handle_library_query, LIBRARY_QUERIES};

// After
use crate::infra::wire::registry::{handle_library_query, LIBRARY_QUERIES};
```

**Files to update:**
- `src/infra/daemon/dispatch.rs`
- `src/lib.rs` (if using registry directly)

#### Files importing `ops::type_extraction`:
```rust
// Before
use crate::ops::type_extraction::{generate_spacedrive_api, OperationTypeInfo};

// After
use crate::infra::wire::type_extraction::{generate_spacedrive_api, OperationTypeInfo};
```

**Files to update:**
- `src/bin/generate_swift_types.rs`
- `src/bin/generate_typescript_types.rs`
- `src/ops/test_type_extraction.rs`

#### Files importing `ops::api_types`:
```rust
// Before
use crate::ops::api_types::{ApiJobHandle, ToApiType};

// After
use crate::infra::wire::api_types::{ApiJobHandle, ToApiType};
```

**Files to update:**
- Any action outputs that wrap JobHandle
- Files in `src/ops/*/output.rs` that use ApiJobHandle

#### Registration Macros

The registration macros themselves don't need changes - they're path-agnostic:
```rust
// Still works after move
crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
```

The macros generate references to:
- `$crate::infra::wire::registry::LibraryQueryEntry` (update in macro)
- `$crate::infra::query::LibraryQuery` (update in macro)
- `$crate::infra::wire::type_extraction::QueryTypeInfo` (update in macro)

### Phase 4: Update Registration Macros

In `src/infra/wire/registry.rs`, update the macro paths:

```rust
#[macro_export]
macro_rules! register_library_query {
    ($query:ty, $name:literal) => {
        impl $crate::client::Wire for <$query as $crate::infra::query::LibraryQuery>::Input {
            const METHOD: &'static str = $crate::query_method!($name);
        }
        inventory::submit! {
            $crate::infra::wire::registry::LibraryQueryEntry {
                method: <<$query as $crate::infra::query::LibraryQuery>::Input as $crate::client::Wire>::METHOD,
                handler: $crate::infra::wire::registry::handle_library_query::<$query>,
            }
        }

        impl $crate::infra::wire::type_extraction::QueryTypeInfo for $query {
            type Input = <$query as $crate::infra::query::LibraryQuery>::Input;
            type Output = <$query as $crate::infra::query::LibraryQuery>::Output;

            fn identifier() -> &'static str {
                $name
            }

            fn scope() -> $crate::infra::wire::type_extraction::QueryScope {
                $crate::infra::wire::type_extraction::QueryScope::Library
            }

            fn wire_method() -> String {
                $crate::query_method!($name).to_string()
            }
        }

        inventory::submit! {
            $crate::infra::wire::type_extraction::QueryExtractorEntry {
                extractor: <$query as $crate::infra::wire::type_extraction::QueryTypeInfo>::extract_types,
                identifier: $name,
            }
        }
    };
}
```

Similar updates for:
- `register_core_query!`
- `register_library_action!`
- `register_core_action!`

### Phase 5: Update Module Documentation

1. **`src/infra/query/mod.rs`**
   ```rust
   //! Query infrastructure for read-only operations
   //!
   //! This module provides the query side of our CQRS-inspired architecture:
   //! - Query traits (`CoreQuery`, `LibraryQuery`) that operations implement
   //! - `QueryManager` for consistent infrastructure (validation, logging)
   //!
   //! ## Relationship to Actions
   //!
   //! Queries are the read-only counterpart to actions (see `infra::action`):
   //! - **Queries**: Retrieve data without mutating state
   //! - **Actions**: Modify state (create, update, delete)
   //!
   //! Both use the same wire protocol system (see `infra::wire`) for
   //! client-daemon communication.
   ```

2. **`src/infra/wire/mod.rs`**
   ```rust
   //! Wire protocol and type system infrastructure
   //!
   //! This module contains the plumbing that connects client applications
   //! to core operations via Unix domain sockets:
   //!
   //! ## Components
   //!
   //! - **Registry**: Compile-time registration using `inventory` crate,
   //!   maps method strings to handler functions
   //! - **Type Extraction**: Generates client types (Swift, TypeScript) from
   //!   Rust types using Specta
   //! - **API Types**: Wrappers for client-compatible types (e.g., ApiJobHandle)
   //!
   //! ## How It Works
   //!
   //! 1. Operations register with macros: `register_library_query!`, etc.
   //! 2. At compile time, `inventory` collects all registrations
   //! 3. At runtime, daemon looks up handlers by method string
   //! 4. Handlers deserialize input, execute operation, serialize output
   //! 5. At build time, code generators use type extractors to create clients
   ```

### Phase 6: Update Documentation

Update these documentation files:
- `docs/core/daemon.md` - Update paths in code examples
- `core/AGENTS.md` - Update architecture section
- `docs/API_DESIGN.md` - Update if it references cqrs.rs

### Phase 7: Testing

1. Run tests to ensure all imports resolved:
   ```bash
   cargo test --workspace
   ```

2. Run clippy to catch any issues:
   ```bash
   cargo clippy --workspace
   ```

3. Verify type generation still works:
   ```bash
   cargo run --bin generate_swift_types
   cargo run --bin generate_typescript_types
   ```

4. Test daemon startup and client communication:
   ```bash
   cargo run --bin sd-cli restart
   cargo run --bin sd-cli libraries list
   ```

## File-by-File Changes

### Files to Move

| Old Path | New Path | Lines |
|----------|----------|-------|
| `src/cqrs.rs` | `src/infra/query/mod.rs` | 115 |
| `src/ops/registry.rs` | `src/infra/wire/registry.rs` | 484 |
| `src/ops/type_extraction.rs` | `src/infra/wire/type_extraction.rs` | 698 |
| `src/ops/api_types.rs` | `src/infra/wire/api_types.rs` | 42 |

### Files to Create

| Path | Purpose |
|------|---------|
| `src/infra/query/manager.rs` | Extract QueryManager from mod.rs if needed |
| `src/infra/wire/mod.rs` | Module re-exports and documentation |

### Files to Update (Import Changes)

**Critical files** (must be updated for compilation):
- `src/lib.rs` - Core module, uses cqrs and registry
- `src/infra/mod.rs` - Add new modules
- `src/ops/mod.rs` - Remove moved modules
- `src/infra/api/dispatcher.rs` - Uses query traits
- `src/infra/daemon/dispatch.rs` - Uses registry
- `src/bin/generate_swift_types.rs` - Uses type extraction
- `src/bin/generate_typescript_types.rs` - Uses type extraction

**Operation files** (50+ files):
- All `src/ops/*/query.rs` files - Import CoreQuery/LibraryQuery
- All `src/ops/*/action.rs` files - Import CoreAction/LibraryAction
- Files using registration macros

## Validation Checklist

Before considering the migration complete:

- [ ] All files compile without errors
- [ ] All tests pass (`cargo test --workspace`)
- [ ] Clippy has no new warnings (`cargo clippy --workspace`)
- [ ] Type generation works (Swift and TypeScript)
- [ ] Daemon starts successfully
- [ ] Client can communicate with daemon
- [ ] All registration macros work correctly
- [ ] Documentation updated
- [ ] AGENTS.md updated with new paths

## Alternatives Considered

### Alternative 1: Keep Query Separate

```
src/
├── query/                   # Move cqrs.rs to top level
└── infra/
    ├── action/
    └── registry/
```

**Pros**: Smaller change
**Cons**: Query and Action still not peers, inconsistent

### Alternative 2: Lighter Touch

```
src/infra/
├── action/
├── query/
└── registry/               # Registry and type extraction together
```

**Pros**: Less nesting
**Cons**: "registry" doesn't capture type extraction purpose

### Why Option A (with `wire/` directory) is Best

1. **Semantic Clarity**: "wire" clearly indicates wire protocol concerns
2. **Room to Grow**: Can add related concerns (serialization, versioning)
3. **Clear Boundaries**: Each directory has single, obvious purpose
4. **Industry Standard**: "wire" is common in RPC/protocol contexts

## Implementation Notes

### About `inventory` Crate

The registration system uses `inventory` for compile-time collection:
```rust
inventory::submit! {
    LibraryQueryEntry {
        method: "query:files.list.v1",
        handler: handle_library_query::<FileListQuery>,
    }
}
```

This means the registry system has **no runtime discovery** - everything is determined at compile time. This is why the registry and type extraction live together: they're both part of the compile-time type system.

### Path Updates in Macros

The registration macros use `$crate::` which resolves to the crate root, so they reference absolute paths. When updating macros, use full paths:

```rust
$crate::infra::wire::registry::LibraryQueryEntry
```

Not:
```rust
crate::infra::wire::registry::LibraryQueryEntry  // Wrong - missing $
```

### Client Wire Trait

The `Wire` trait in `src/client/mod.rs` could optionally move to `src/infra/wire/client.rs` for better organization, but it's fine to leave it where it is since "client" is a top-level concept.

## Future Considerations

### Potential Enhancements

1. **Versioning**: Add version negotiation to wire protocol
2. **Middleware**: Add query/action middleware system
3. **Caching**: Add query result caching layer
4. **Metrics**: Add wire protocol metrics collection

### Evolution Path

This reorganization sets up for future enhancements:
- `infra/wire/versioning.rs` - Protocol version negotiation
- `infra/wire/middleware.rs` - Request/response interceptors
- `infra/query/cache.rs` - Query result caching
- `infra/action/validation.rs` - Cross-action validation

## Conclusion

This reorganization improves code organization by:
1. Grouping related infrastructure together
2. Making action/query relationship obvious
3. Clarifying infrastructure vs business logic boundary
4. Improving discoverability for new contributors
5. Using more accurate names

The migration is mechanical (mostly moving files and updating imports) with minimal risk since we're not changing functionality - just organization.

## Appendix: Search and Replace Patterns

For migration assistance, here are regex patterns for common import updates:

### Query Imports
```bash
# Find
use crate::cqrs::(.*);

# Replace
use crate::infra::query::$1;
```

### Registry Imports
```bash
# Find
use crate::ops::registry::(.*);

# Replace
use crate::infra::wire::registry::$1;
```

### Type Extraction Imports
```bash
# Find
use crate::ops::type_extraction::(.*);

# Replace
use crate::infra::wire::type_extraction::$1;
```

### API Types Imports
```bash
# Find
use crate::ops::api_types::(.*);

# Replace
use crate::infra::wire::api_types::$1;
```
