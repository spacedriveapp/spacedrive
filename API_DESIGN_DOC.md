# Design Doc: Spacedrive Architecture Refactoring

**Authors:** Gemini, jamespine
**Date:** 2025-09-08
**Status:** Proposed

## 1. Abstract

This document proposes a significant refactoring of the Spacedrive CLI and daemon architecture. The goal is to create a cleaner, more scalable, and more maintainable system by establishing clear boundaries and responsibilities for the core components of the application.

The proposed architecture will:

1.  **Promote the Command-Line Interface (CLI) to be a first-class application** within the `apps/` directory, consistent with other client applications.
2.  **Establish a formal, public API for the `Core` engine** by introducing `CoreCommand` and `CoreQuery` enums.
3.  **Refactor the daemon to be a pure, application-agnostic middleware layer** whose sole responsibilities are process management and request routing.

This refactoring will create a more logical and decoupled architecture, laying a robust foundation for future development, including the creation of a GraphQL API and other clients.

## 2. Motivation

The current architecture, while functional, has several design characteristics that will impede future development and maintenance:

*   **Tight Coupling:** The CLI and daemon are tightly coupled. The `DaemonCommand` enum is a direct mirror of the CLI's command structure, meaning any change to the CLI requires a corresponding change in the daemon. This makes it difficult to evolve the system or add new clients (like a GraphQL API) without significant effort.
*   **Blurred Responsibilities:** The daemon currently contains application-specific logic, including command handlers and client state management, that rightfully belongs in the `Core` or should be managed on behalf of a client. This makes the daemon a "fat" component that is difficult to maintain and test independently of the `Core`.
*   **Inconsistent Project Structure:** The CLI, a user-facing application, is currently located in `core/src/infra/cli`. This is not an intuitive location and is inconsistent with the project's convention of placing applications in the `apps/` directory.

This refactoring will address these issues by creating a clean and logical separation of concerns between the three main components of the system: the client applications, the daemon middleware, and the `Core` engine.

## 3. Proposed Architecture

The new architecture will be composed of three distinct, decoupled components:

1.  **Client Applications (`apps/`):** User-facing applications. The primary focus of this refactoring is the `apps/cli` crate.
2.  **The Daemon (`daemon/`):** A pure middleware layer that manages `Core` instances and routes RPC requests.
3.  **The `Core` (`core/`):** The heart of the application, containing all business logic and exposing a formal public API.

### 3.1. Project Structure

The new project structure will be as follows:

```
spacedrive/
├── apps/
│   ├── cli/                # << NEW: CLI App Crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs     # CLI entry point
│   │       ├── commands.rs # clap command definitions
│   │       ├── client.rs   # Daemon client
│   │       └── output.rs   # Output formatting logic
│   │
│   ├── desktop/
│   ├── mobile/
│   └── web/
│
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       │
│       ├── api/              # << NEW: Formal Core API
│       │   ├── mod.rs
│       │   ├── command.rs    # Defines the CoreCommand enum
│       │   ├── query.rs      # Defines the CoreQuery enum
│       │   └── response.rs   # Defines response types
│       │
│       ├── engine/           # << NEW: Command/Query Execution Logic
│       │   ├── mod.rs        # Implements `Core::execute` and `Core::query`
│       │   ├── library.rs    # Logic for library commands/queries
│       │   ├── location.rs   # Logic for location commands/queries
│       │   └── ...           # etc. for each domain
│       │
│       ├── infra/            # Infrastructure (DB, events, etc.)
│       │   └── ...           # (The old `cli` folder will be removed from here)
│       │
│       └── domain/           # Core domain models
│           └── ...
│
└── daemon/                   # << NEW: Daemon Crate
    ├── Cargo.toml
    └── src/
        ├── main.rs         # Daemon entry point
        ├── instance.rs     # Manages a single Core instance
        ├── rpc.rs          # RPC layer for communication
        └── state.rs        # ClientStateService
```

### 3.2. The `Core` API

The `Core` will expose a formal, public API based on the Command-Query Separation (CQS) principle. This API will be defined by two primary enums:

*   **`CoreCommand`:** Defines all state-changing (write) operations that can be performed on a Spacedrive instance (e.g., `CreateLibrary`, `AddLocation`).
*   **`CoreQuery`:** Defines all data-retrieval (read) operations (e.g., `ListLibraries`, `GetJobInfo`).

The `Core` will expose two methods for executing these operations:

*   `Core::execute(command: CoreCommand) -> CoreCommandResponse`
*   `Core::query(query: CoreQuery) -> CoreQueryResponse`

This API will be the single, comprehensive, and strongly-typed contract for all clients.

### 3.3. The Daemon

The daemon will be refactored into a pure middleware layer with two main responsibilities:

1.  **`Core` Instance Management:** The daemon will run and manage one or more `Core` instances, allowing for multiple, isolated Spacedrive environments on a single machine.
2.  **RPC Routing:** The daemon will listen for RPC requests from clients, deserialize them into `CoreCommand`s or `CoreQuery`s, and route them to the appropriate `Core` instance for execution.

The daemon will also provide a `ClientStateService` to manage small amounts of client-specific state (e.g., the "current library" for the CLI) as a pragmatic solution to improve user experience.

### 3.4. The CLI

The CLI will be promoted to a first-class application in the `apps/` directory. Its responsibilities will be:

*   Parsing user commands via `clap`.
*   Constructing the appropriate `CoreCommand` or `CoreQuery`.
*   Sending the command/query to the daemon.
*   Receiving the response from the daemon and formatting it for display to the user.

## 4. Implementation Plan

The refactoring will be executed in the following sequence to ensure a smooth transition:

1.  **Establish the `Core` API:** Create the `core/src/api` directory and define the `CoreCommand`, `CoreQuery`, and their corresponding response enums.
2.  **Implement the `Core` Engine:** Create the `core/src/engine` directory. Implement the `Core::execute()` and `Core::query()` methods, which will contain the logic currently found in the daemon's command handlers.
3.  **Create the `daemon` Crate:** Create a new top-level `daemon` crate. Move the daemon logic from `core/src/infra/cli/daemon` into this new crate and adapt it to use the new `Core` API.
4.  **Create the `apps/cli` Crate:** Create a new `apps/cli` crate. Move the CLI-specific code from `core/src/infra/cli` into this new crate and adapt it to communicate with the new daemon.
5.  **Update `Cargo.toml` Files:** Adjust dependencies and workspace members in the root `Cargo.toml` to reflect the new crate structure.
6.  **Cleanup:** Remove the now-empty `core/src/infra/cli` directory.

## 5. Future Considerations

This refactoring will provide a solid and scalable foundation for future development.

### 5.1. Query Performance and Optimization

A potential pitfall of this architecture is that a naive implementation of the `CoreQuery` API could lead to performance problems (e.g., the "N+1 query problem"). To avoid this, the query system will be designed with the following principles in mind:

*   **Expressive Query API:** The `CoreQuery` API will be designed as a rich, expressive query language using data structures, rather than a simple enum of fixed queries. This will allow clients to specify their exact data requirements, including filters, sorting, field selection, and relations.
*   **Smart Query Engine:** The `Core`'s query engine will be responsible for taking these expressive query objects and translating them into efficient database queries. It will be designed to be highly optimized, with features like the "Data Loader" pattern to batch and cache database requests.
*   **Client-Side Lookahead:** Clients like the GraphQL API will be encouraged to use "lookahead" features to inspect the client's query and construct the most efficient `CoreQuery` possible.

This approach will provide the flexibility and performance required by a data-intensive application like Spacedrive, while still maintaining a clean separation of concerns.

### 5.2. The Role of the GraphQL API

While the `Core` API provides a powerful and performant interface for internal, system-level clients like the CLI, the GraphQL API will serve as the primary interface for user-facing clients, especially the web frontend.

The GraphQL API is not a redundant abstraction, but rather a different tool for a different job. It provides several key advantages for frontend development:

*   **A Standard, Language-Agnostic Interface:** The GraphQL API is a language-agnostic standard, which allows our frontend developers (who will be writing JavaScript/TypeScript) to use a vast ecosystem of existing tools and libraries.
*   **A Rich Tooling Ecosystem:** The GraphQL ecosystem provides tools like GraphQL Code Generator and Apollo Client, which dramatically accelerate frontend development by providing type safety, caching, and state management.
*   **Client-Driven Flexibility:** GraphQL allows the frontend to request exactly the data it needs, in the shape it needs it, in a single request. This is a huge advantage for building complex user interfaces.

The GraphQL API will be implemented as a new application in the `apps/` directory. Its resolvers will be a thin translation layer that constructs `CoreCommand`s and `CoreQuery`s and sends them to the `Core` via the daemon.

### 5.3. Example GraphQL Queries

Here are some examples of the kinds of complex, file manager-style queries that the GraphQL API will support:

#### 1. Paginated and Sorted File Listing

```graphql
query GetDirectoryContents(
  $libraryId: ID!
  $parentId: ID
  $first: Int = 50
  $after: String
  $sortBy: FileSortField = NAME
  $sortDirection: SortDirection = ASC
  $filter: FileFilter
) {
  library(id: $libraryId) {
    objects(
      parentId: $parentId
      first: $first
      after: $after
      sortBy: $sortBy
      sortDirection: $sortDirection
      filter: $filter
    ) {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
          name
          size
          modifiedAt
          ... on File {
            extension
          }
          ... on Directory {
            childCount
          }
        }
      }
    }
  }
}
```

#### 2. File Details with Specific Metadata

```graphql
query GetFileDetails($objectId: ID!) {
  object(id: $objectId) {
    id
    name
    size
    createdAt
    modifiedAt
    ... on Image {
      format
      width
      height
      exif {
        cameraModel
        exposureTime
        iso
      }
    }
    ... on Video {
      format
      duration
      width
      height
      codec
    }
  }
}
```

#### 3. Job Monitoring (Subscription)

```graphql
subscription MonitorJob($jobId: ID!) {
  job(id: $jobId) {
    id
    status
    progress
    message
  }
}
```

### 5.4. GraphQL API Mapping

The GraphQL API resolvers will act as a thin translation layer between the GraphQL schema and the `Core` API. The resolvers' primary responsibility is to construct the appropriate `CoreCommand` or `CoreQuery` and send it to the `Core` for execution.

#### Mutation to `CoreCommand` Mapping

A GraphQL mutation will be mapped to a `CoreCommand`. The arguments of the mutation will be used to construct the corresponding `CoreCommand` variant.

**GraphQL Mutation:**

```graphql
mutation CreateNewLibrary($name: String!, $path: String!) {
  createLibrary(name: $name, path: $path) {
    id
    name
    path
  }
}
```

**Resolver Logic (Conceptual):**

```rust
// In apps/graphql/src/resolvers.rs

async fn create_library(&self, name: String, path: String) -> Result<Library> {
    // 1. Construct the CoreCommand
    let command = CoreCommand::CreateLibrary { name, path };

    // 2. Send the command to the Core via the daemon
    let response = self.daemon_client.execute(command).await?;

    // 3. Process the response and return the result
    match response {
        CoreCommandResponse::Library(library) => Ok(library),
        _ => Err("Unexpected response from Core".into()),
    }
}
```

#### Query to `CoreQuery` Mapping

A GraphQL query will be mapped to a `CoreQuery`. The resolver will use the query's arguments and the "lookahead" feature to construct the most efficient `CoreQuery` possible.

**GraphQL Query:**

```graphql
query GetLibraryDetails($libraryId: ID!) {
  library(id: $libraryId) {
    id
    name
    locations {
      id
      name
    }
  }
}
```

**Resolver Logic (Conceptual):**

```rust
// In apps/graphql/src/resolvers.rs

async fn library(&self, ctx: &Context<'_>, id: Uuid) -> Result<Library> {
    // 1. Use lookahead to determine which relations to include
    let include_locations = ctx.look_ahead().field("locations").exists();

    // 2. Construct the CoreQuery
    let query = CoreQuery::GetLibrary {
        id,
        options: GetLibraryOptions {
            include_locations,
            // ... other options based on lookahead
        },
    };

    // 3. Send the query to the Core via the daemon
    let response = self.daemon_client.query(query).await?;

    // 4. Process the response and return the result
    match response {
        CoreQueryResponse::Library(library) => Ok(library),
        _ => Err("Unexpected response from Core".into()),
    }
}
```
