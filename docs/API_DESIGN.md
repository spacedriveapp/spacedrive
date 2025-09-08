# Design Doc: Spacedrive Architecture v2

**Authors:** Gemini, jamespine
**Date:** 2025-09-08
**Status:** **Active**

## 1\. Abstract

This document proposes a significant refactoring of the Spacedrive `Core` engine's API. The goal is to establish a formal, scalable, and modular API boundary that enhances the existing strengths of the codebase.

The proposed architecture will:

1.  **Formalize the API using a CQRS (Command Query Responsibility Segregation) pattern**. We will introduce distinct `Action` (write) and `Query` (read) traits.
2.  **Define the `Core` API as a collection of self-contained, modular operations**, rather than a monolithic enum. Each operation will be its own discoverable and testable unit.
3.  **Provide a generic `Core::execute_action` and `Core::execute_query` method**, using Rust's trait system to create a type-safe and extensible entry point into the engine.

This design provides a robust foundation for all client applications (GUI, CLI, GraphQL), ensuring consistency, maintainability, and scalability.

---

## 2\. Motivation

After analyzing the current codebase, we've discovered that Spacedrive already has a sophisticated and well-designed action system:

**Existing Strengths:**

- **Modular Action System:** Individual action structs in dedicated `ops/` modules (e.g., `LibraryCreateAction`, `FileCopyAction`)
- **Robust Infrastructure:** `ActionManager` with audit logging, validation, and error handling
- **Type Safety:** Strong typing with proper validation and output types
- **Clean Separation:** Each operation is self-contained with its own handler

**Real Problems to Address:**

- **Missing Query Operations:** No formal system for read-only operations (browsing, searching, listing)
- **CLI-Daemon Coupling:** CLI tightly coupled to `DaemonCommand` enum instead of using Core API directly
- **Inconsistent API Surface:** Actions go through ActionManager, but other operations are ad-hoc
- **No Unified Entry Point:** Multiple ways to interact with Core instead of consistent interface

The new proposal builds upon the existing excellent action foundation while addressing these real gaps.

---

## 3\. Proposed Design: Enhanced CQRS API

The design enhances the existing action system by adding formal query operations and a unified API surface, following the **CQRS** pattern for absolute clarity between reads and writes.

### 3.1. Enhanced Action System (for Writes/Mutations)

The existing action system already provides excellent foundations. We'll enhance it with a formal `Action` trait that existing action structs can implement.

- **New Action Trait**:

  ```rust
  /// A command that mutates system state.
  /// This trait will be implemented by existing action structs.
  pub trait Action {
      /// The output after the action succeeds.
      type Output;

      /// Execute this action with the given context.
      async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
  }
  ```

- **Integration with Existing System**:

  ```rust
  // Existing action struct in: core/src/ops/libraries/create/action.rs
  impl Action for LibraryCreateAction {
      type Output = LibraryCreateOutput;

      async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
          // Delegate to existing ActionManager infrastructure
          let action = crate::infra::action::Action::LibraryCreate(self);
          let result = context.action_manager().dispatch(action).await?;
          // Convert ActionOutput to LibraryCreateOutput
          Ok(result.into())
      }
  }
  ```

### 3.2. New Query System (for Reads)

This is the major addition - a formal system for read-only operations that mirrors the design and benefits of the existing `ActionManager`. It will be the single entry point for all read operations, allowing us to implement cross-cutting concerns like validation, permissions, and logging for every query in the system.

- **Query Trait**:

  ```rust
  /// A request that retrieves data without mutating state.
  pub trait Query {
      /// The data structure returned by the query.
      type Output;
  }
  ```

- **QueryHandler Trait**:

  ```rust
  /// Any struct that knows how to resolve a query will implement this trait.
  pub trait QueryHandler<Q: Query> {
      /// Validates the query input and checks permissions.
      async fn validate(&self, core: &Core, query: &Q) -> Result<()>;

      /// Executes the query and returns the result.
      async fn execute(&self, core: &Core, query: Q) -> Result<Q::Output>;
  }
  ```

- **QueryManager**:

  The `QueryManager` will use a registry to look up the correct `QueryHandler` for any given `Query` struct. Its `dispatch` method will orchestrate the entire process.

  ```rust
  pub struct QueryManager {
      registry: QueryRegistry, // Maps Query types to their handlers
  }

  impl QueryManager {
      pub async fn dispatch<Q: Query>(&self, core: &Core, query: Q) -> Result<Q::Output> {
          // 1. Look up the handler for this specific query type.
          let handler = self.registry.get_handler_for::<Q>()?;

          // 2. Run validation and permission checks.
          handler.validate(core, &query).await?;

          // 3. (Optional) Add audit logging for the read operation.
          // log::info!("User X is querying Y...");

          // 4. Execute the query.
          handler.execute(core, query).await
      }
  }
  ```

### 3.3. Enhanced Core Interface

The `Core` engine will expose a unified API that delegates to the appropriate manager, keeping the `Core` itself clean.

```rust
// In: core/src/lib.rs
impl Core {
    /// Executes an action using the existing ActionManager infrastructure.
    pub async fn execute_action<A: Action>(&self, action: A) -> Result<A::Output> {
        self.action_manager.dispatch(action).await
    }

    /// Executes a query using the new QueryManager.
    pub async fn execute_query<Q: Query>(&self, query: Q) -> Result<Q::Output> {
        self.query_manager.dispatch(query).await
    }
}
```

---

## 4\. Client Integration Strategy

The strategy focuses on **decoupling the CLI from the daemon** while preserving the existing, working action infrastructure.

### 4.1. CLI Refactoring Strategy

The **CLI** should be refactored to use the Core API directly instead of going through the daemon for most operations. The daemon becomes optional infrastructure for background services.

**Current Architecture:**

```
CLI → DaemonCommand → Daemon → ActionManager → Action Handlers
```

**Target Architecture:**

```
CLI → Core API (execute_action/execute_query) → Action/Query Handlers
Daemon → Core API (same interface, used for background services)
```

- **Migration Approach:**

  ```rust
  // CURRENT: CLI sends commands to daemon
  let command = DaemonCommand::CreateLibrary { name: "Photos".to_string() };
  daemon_client.send_command(command).await?;

  // TARGET: CLI uses Core API directly
  let action = LibraryCreateAction { name: "Photos".to_string(), path: None };
  let result = core.execute_action(action).await?;
  println!("Library created with ID: {}", result.id);
  ```

### 4.2. Daemon Role Evolution

The **daemon** evolves from a command processor to a **background service coordinator**. Most CLI operations will bypass the daemon entirely.

**New Daemon Responsibilities:**

1. **Background Services:** Long-running operations (indexing, file watching, networking)
2. **Multi-Client Coordination:** When multiple clients need to share state
3. **Resource Management:** Managing expensive resources (database connections, file locks)
4. **Optional IPC:** For GUI clients that prefer daemon-mediated access

**Simplified Daemon Logic:**

```rust
// Daemon becomes a thin wrapper around Core
impl DaemonHandler {
    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        match request {
            DaemonRequest::Action(action) => {
                let result = self.core.execute_action(action).await;
                DaemonResponse::ActionResult(result)
            }
            DaemonRequest::Query(query) => {
                let result = self.core.execute_query(query).await;
                DaemonResponse::QueryResult(result)
            }
        }
    }
}
```

### 4.3. GraphQL Server Integration

The **GraphQL server** is a new, first-class client of the `Core` engine. The CQRS model maps perfectly to its structure.

- **GraphQL Queries**: Resolvers will construct and execute `Query` structs via `core.execute_query()`.
- **GraphQL Mutations**: Resolvers will construct and execute `Action` structs via `core.execute_action()`.

This allows the GraphQL layer to be a flexible composer of modular backend operations without needing any special logic or "god object" queries in the `Core`.

**Example GraphQL Resolver:**

```rust
// In: apps/graphql/src/resolvers.rs
async fn resolve_objects(core: &Core, parent_id: Uuid) -> Result<Vec<Entry>> {
    // 1. Build the strongly-typed Query struct from GraphQL arguments.
    let query = GetDirectoryContentsQuery {
        parent_id: Some(parent_id),
        // ... other options
    };

    // 2. Execute the query via the Core API.
    let result = core.execute_query(query).await?;

    // 3. Return the result.
    Ok(result)
}
```

---

## 5\. Benefits of this Enhanced Design

- **Preserves Existing Investment:** Builds upon the excellent existing action system rather than replacing it
- **Adds Missing Functionality:** Introduces formal query operations that were previously ad-hoc
- **Reduces CLI-Daemon Coupling:** CLI can work directly with Core API, making daemon optional
- **Maintains All Benefits:** Preserves audit logging, validation, error handling from existing ActionManager
- **Type-Safe Query System:** Brings the same type safety to read operations that actions already have
- **Unified API Surface:** Single entry point (`execute_action`/`execute_query`) for all clients
- **Backward Compatibility:** Existing code continues to work unchanged during migration

# Revised Implementation Plan

## **Phase 1: Add CQRS Traits (Zero Risk)**

Add the trait definitions that will work alongside the existing action system, without changing any existing code.

1.  **Define the Enhanced Traits:**

    ```rust
    // core/src/cqrs.rs
    use anyhow::Result;
    use std::sync::Arc;
    use crate::context::CoreContext;

    /// Enhanced action trait that works with existing ActionManager
    pub trait Action {
        type Output;

        /// Execute this action using existing infrastructure
        async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
    }

    /// New query trait for read operations
    pub trait Query {
        type Output;

        /// Execute this query
        async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
    }
    ```

2.  **Add Core API Methods:**

    ```rust
    // core/src/lib.rs - add to existing Core impl
    impl Core {
        /// Execute action using new trait (delegates to existing ActionManager)
        pub async fn execute_action<A: Action>(&self, action: A) -> Result<A::Output> {
            action.execute(self.context.clone()).await
        }

        /// Execute query using new system
        pub async fn execute_query<Q: Query>(&self, query: Q) -> Result<Q::Output> {
            query.execute(self.context.clone()).await
        }
    }
    ```

**Outcome:** New API exists alongside current system. Zero breaking changes.

---

## **Phase 2: Implement Action Trait (Low Risk)**

Implement the Action trait for existing LibraryCreateAction, delegating to the existing ActionManager.

1.  **Implement Action Trait:**

    ```rust
    // core/src/ops/libraries/create/action.rs - add to existing file
    use crate::cqrs::Action;
    use crate::infra::action::output::ActionOutput;

    impl Action for LibraryCreateAction {
        type Output = LibraryCreateOutput;

        async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
            // Use existing ActionManager infrastructure
            let action = crate::infra::action::Action::LibraryCreate(self);
            let result = context.action_manager().dispatch(action).await?;

            // Convert ActionOutput to specific output type
            match result {
                ActionOutput::LibraryCreate(output) => Ok(output),
                _ => Err(anyhow::anyhow!("Unexpected output type")),
            }
        }
    }
    ```

2.  **Test the Integration:**

    ```rust
    // Test that both paths work
    let action = LibraryCreateAction { name: "Test".to_string(), path: None };

    // Old way (still works)
    let old_result = core.dispatch_action(Action::LibraryCreate(action.clone())).await?;

    // New way (trait-based)
    let new_result = core.execute_action(action).await?;
    ```

**Outcome:** LibraryCreateAction works through both old and new APIs.

---

## **Phase 3: Create Query System (Medium Risk)**

Add the first query operations to demonstrate the read-only system.

1.  **Create First Query:**

    ```rust
    // core/src/ops/libraries/list/query.rs (new file)
    use crate::cqrs::Query;

    pub struct ListLibrariesQuery {
        pub include_stats: bool,
    }

    pub struct LibraryInfo {
        pub id: Uuid,
        pub name: String,
        pub path: PathBuf,
        pub stats: Option<LibraryStats>,
    }

    impl Query for ListLibrariesQuery {
        type Output = Vec<LibraryInfo>;

        async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
            let libraries = context.library_manager.list().await;
            let mut result = Vec::new();

            for lib in libraries {
                let stats = if self.include_stats {
                    Some(lib.get_stats().await?)
                } else {
                    None
                };

                result.push(LibraryInfo {
                    id: lib.id(),
                    name: lib.name().await,
                    path: lib.path().to_path_buf(),
                    stats,
                });
            }

            Ok(result)
        }
    }
    ```

**Outcome:** Query system exists and can be used alongside actions.

---

## **Phase 4: CLI Direct Integration (High Value)**

Refactor CLI to use Core API directly, reducing daemon dependency.

1.  **CLI Architecture Change:**

    ```rust
    // Current: CLI → Daemon → Core
    // Target:  CLI → Core (daemon optional)

    // apps/cli/src/main.rs (conceptual)
    pub async fn run_cli() -> Result<()> {
        // Initialize Core directly in CLI
        let core = Core::new_with_config(data_dir).await?;

        match cli_args.command {
            Command::CreateLibrary { name } => {
                let action = LibraryCreateAction { name, path: None };
                let result = core.execute_action(action).await?;
                println!("Created library: {}", result.id);
            }
            Command::ListLibraries => {
                let query = ListLibrariesQuery { include_stats: true };
                let libraries = core.execute_query(query).await?;
                display_libraries(libraries);
            }
        }
    }
    ```

2.  **Gradual Migration:**
    - Start with read-only commands (list, status, info)
    - Move to simple actions (create, rename)
    - Keep complex operations daemon-mediated initially

**Outcome:** CLI becomes independent, daemon becomes optional infrastructure.

---

## **Phase 5: Complete Query System & GraphQL**

Finish the query system and build GraphQL server as proof of unified API.

1.  **Complete Query Coverage:**

    - File browsing queries
    - Search queries
    - Status/info queries
    - Statistics queries

2.  **GraphQL Server:**
    - Uses same `execute_action`/`execute_query` interface
    - Demonstrates API consistency across clients
    - Provides web-friendly interface

**Outcome:** Full CQRS API with multiple client types proving the design.
