# Spacedrive Operations System

The Spacedrive Operations System is a modular, type-safe architecture for handling all business logic through a unified Command-Query Separation (CQRS) pattern. This system enables clean separation of concerns, excellent testability, and consistent APIs across all client applications (CLI, GraphQL, Desktop, Mobile, Web).

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Client Apps   │    │     Daemon      │    │   Core Engine   │
│                 │    │                 │    │                 │
│ • CLI           │───▶│ • RPC Server    │───▶│ • Method        │
│ • GraphQL       │    │ • Request       │    │   Dispatcher    │
│ • Desktop       │    │   Routing       │    │ • Inventory     │
│ • Mobile        │    │ • Session Mgmt  │    │   Registry      │
│ • Web           │    │                 │    │ • Query/Action  │
└─────────────────┘    └─────────────────┘    │   Execution     │
                                              └─────────────────┘
```

## Core Concepts

### 1. Commands vs Queries (CQRS)

- **Queries**: Read-only operations that return data without side effects
- **Actions**: State-changing operations that modify data and may return results

### 2. Modular Operations

Each feature is organized as a self-contained module in `core/src/ops/` with a consistent structure:

```
ops/
├── feature_name/
│   ├── mod.rs          # Module exports and re-exports
│   ├── query.rs        # Query definitions (if applicable)
│   ├── action.rs       # Action definitions (if applicable)
│   ├── input.rs        # Input types for external APIs
│   ├── output.rs       # Output types for responses
│   └── job.rs          # Background job implementations
```

### 3. Type-Safe Routing

All operations are registered with the Core engine using method strings and the `Wire` trait:

```rust
impl Wire for CoreStatusQuery {
    const METHOD: &'static str = "query:core.status.v1";
}
```

## Directory Structure

### Domain Organization

Operations are organized by domain:

```
ops/
├── core/                    # Core system operations
│   └── status/
│       ├── query.rs        # Core status query
│       └── output.rs       # Core status output
├── files/                   # File operations
│   ├── copy/               # File copying
│   ├── delete/             # File deletion
│   └── validation/         # File validation
├── libraries/              # Library management
│   ├── create/             # Create library
│   ├── list/               # List libraries
│   └── delete/             # Delete library
├── locations/              # Location management
├── volumes/                # Volume operations
├── media/                  # Media processing
└── indexing/               # File indexing
```

### File Patterns

Each operation module follows consistent patterns:

#### `query.rs` - Query Definitions

```rust
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListLibrariesQuery;

impl Query for ListLibrariesQuery {
    type Output = Vec<LibraryInfo>;

    async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
        // Query implementation
    }
}

impl Wire for ListLibrariesQuery {
    const METHOD: &'static str = "query:libraries.list.v1";
}

register_query!(ListLibrariesQuery);
```

#### `action.rs` - Action Definitions

```rust
use crate::infra::action::{LibraryAction, ActionError};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLibraryAction {
    pub name: String,
    pub description: Option<String>,
}

impl LibraryAction for CreateLibraryAction {
    type Output = LibraryInfo;

    async fn execute(
        self,
        library: Arc<Library>,
        context: Arc<CoreContext>,
    ) -> Result<Self::Output, ActionError> {
        // Action implementation
    }
}
```

#### `input.rs` - External API Input Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyInput {
    pub library_id: Option<Uuid>,
    pub source_paths: Vec<String>,
    pub destination_path: String,
    pub copy_method: CopyMethod,
    pub overwrite: bool,
}

impl BuildLibraryActionInput for FileCopyInput {
    type Action = FileCopyAction;

    fn build(self, session: &SessionState) -> Result<Self::Action, String> {
        // Convert input to action
    }
}

impl Wire for FileCopyInput {
    const METHOD: &'static str = "action:files.copy.input.v1";
}

register_action_input!(FileCopyInput);
```

#### `output.rs` - Response Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyActionOutput {
    pub job_id: Uuid,
    pub sources_count: usize,
    pub destination: String,
}

impl ActionOutputTrait for FileCopyActionOutput {
    fn display_message(&self) -> String {
        format!("Dispatched file copy job {} for {} source(s)",
                self.job_id, self.sources_count)
    }
}
```

#### `job.rs` - Background Job Implementation

```rust
pub struct FileCopyJob {
    pub options: CopyOptions,
    pub sources: Vec<SdPath>,
    pub destination: SdPath,
}

impl Job for FileCopyJob {
    async fn execute(&mut self, progress: &mut Progress) -> Result<()> {
        // Job execution logic
    }
}
```

## Request Flow

### 1. Client Request

```rust
// CLI/GraphQL creates typed input
let input = FileCopyInput {
    source_paths: vec!["/path/to/file".to_string()],
    destination_path: "/path/to/dest".to_string(),
    // ... other fields
};

// Uses CoreClient SDK
let client = CoreClient::new(socket_path);
client.action(&input).await?;
```

### 2. Daemon Routing

```rust
// Daemon receives DaemonRequest::Action
match req {
    Ok(DaemonRequest::Action { method, payload }) => {
        let core = instances.get_default().await?;
        let session = session.get().await;

        // Route to Core's method dispatcher
        match core.execute_action_by_method(&method, payload, session).await {
            Ok(out) => DaemonResponse::Ok(out),
            Err(e) => DaemonResponse::Error(e),
        }
    }
}
```

### 3. Core Method Dispatch

```rust
// Core looks up handler in inventory registry
pub async fn execute_action_by_method(&self, method: &str, payload: Vec<u8>, session: SessionState) -> Result<Vec<u8>, String> {
    if let Some(handler) = crate::ops::registry::ACTIONS.get(method) {
        return handler(Arc::new((*self).clone()), session, payload).await;
    }
    Err("Unknown action method".into())
}
```

### 4. Inventory Registry Handler

```rust
// Generic handler deserializes input and executes action
pub fn handle_library_action_input<I>(core: Arc<Core>, session: SessionState, payload: Vec<u8>) -> LocalBoxFuture<'static, Result<Vec<u8>, String>> {
    (async move {
        let input: I = decode_from_slice(&payload, standard())?.0;
        let action = input.build(&session)?;
        core.execute_library_action(action).await?;
        Ok(Vec::new())
    }).boxed_local()
}
```

### 5. Action Execution

```rust
// Core executes the typed action
pub async fn execute_library_action<A: LibraryAction>(&self, action: A) -> anyhow::Result<A::Output> {
    let action_manager = ActionManager::new(self.context.clone());
    action_manager.dispatch_library(action).await
}
```

## Registration System

### Automatic Registration

Operations self-register using the `inventory` crate:

```rust
// For queries
register_query!(CoreStatusQuery);

// For action inputs
register_action_input!(FileCopyInput);
```

### Method Naming Convention

- **Queries**: `query:{domain}.{operation}.v{version}`

  - `query:core.status.v1`
  - `query:libraries.list.v1`

- **Actions**: `action:{domain}.{operation}.input.v{version}`
  - `action:files.copy.input.v1`
  - `action:libraries.create.input.v1`

## Benefits

### 1. Modularity

- Each feature is self-contained
- Easy to add new operations
- Clear separation of concerns

### 2. Type Safety

- Full type safety from client to execution
- Compile-time verification of API contracts
- No runtime type errors

### 3. Consistency

- Uniform API across all clients
- Consistent error handling
- Standardized input/output patterns

### 4. Testability

- Easy to unit test individual operations
- Mockable dependencies
- Clear interfaces

### 5. Scalability

- Easy to add new client types
- Simple to extend with new features
- Minimal boilerplate for new operations

## Adding New Operations

### 1. Create Module Structure

```bash
mkdir -p core/src/ops/my_feature
touch core/src/ops/my_feature/{mod.rs,query.rs,input.rs,output.rs}
```

### 2. Implement Query/Action

```rust
// query.rs
impl Query for MyFeatureQuery {
    type Output = MyFeatureOutput;
    async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
        // Implementation
    }
}

impl Wire for MyFeatureQuery {
    const METHOD: &'static str = "query:my_feature.get.v1";
}

register_query!(MyFeatureQuery);
```

### 3. Add to Module Exports

```rust
// mod.rs
pub mod query;
pub mod input;
pub mod output;

pub use query::MyFeatureQuery;
pub use input::MyFeatureInput;
pub use output::MyFeatureOutput;
```

### 4. Update Parent Module

```rust
// ops/mod.rs
pub mod my_feature;
```

## Error Handling

All operations use consistent error handling:

- **Query Errors**: Return `anyhow::Result<Output>`
- **Action Errors**: Return `ActionError` with specific error types
- **Input Validation**: Errors during input-to-action conversion
- **Network Errors**: Handled at daemon level

## Performance Considerations

- **Binary Serialization**: Uses `bincode` for efficient serialization
- **Async Execution**: All operations are async for non-blocking I/O
- **Background Jobs**: Long-running operations use job system
- **Caching**: Query results can be cached at appropriate levels

## Future Extensions

The system is designed to easily support:

- **Authentication**: Add auth checks to QueryManager/ActionManager
- **Rate Limiting**: Implement at daemon level
- **Audit Logging**: Add to execution pipeline
- **Metrics**: Collect operation metrics
- **Webhooks**: Add webhook support for actions
- **Batch Operations**: Support for batch queries/actions

This modular operations system provides a solid foundation for building scalable, maintainable file management applications with consistent APIs across all client types.
