# AGENTS.md - Spacedrive Core v2

## Build/Test Commands

- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run specific test (e.g., `cargo test library_test`)
- `cargo clippy` - Lint code
- `cargo fmt` - Format code
- `cargo run --bin sd-cli -- <command>` - Run CLI (note: binary is `sd-cli`, not `spacedrive`)

## Code Style

- **Imports**: Group std, external crates, then local modules with blank lines between
- **Formatting**: Use `cargo fmt` - tabs for indentation, snake_case for variables/functions. DO NOT use emojis at all.
- **Types**: Explicit types preferred, use `Result<T, E>` for error handling with `thiserror`
- **Naming**: snake_case for functions/variables, PascalCase for types, SCREAMING_SNAKE_CASE for constants
- **Error Handling**: Use `Result` types, `thiserror` for custom errors, `anyhow` for application errors
- **Async**: Use `async/await`, prefer `tokio` primitives, avoid blocking operations
- **Resumable Jobs**: For long-running jobs that need to be resumable, store the job's state within the job's struct itself. Use `#[serde(skip)]` for fields that should not be persisted. For example, in a file copy job, the list of already copied files can be stored to allow the job to resume from where it left off.
- **Documentation**: Use `//!` for module docs, `///` for public items, include examples
- **Architecture**: Follow a Command Query Responsibility Segregation (CQRS) and Domain-Driven Design (DDD) pattern.
	- **Domain**: Core data structures and business logic are located in `src/domain/`. These are the "nouns" of your system.
	- **Operations**: State-changing commands (actions) and data-retrieving queries are located in `src/ops/`. These are the "verbs" of your system.
	- **Actions**: Operations that modify the state of the application. They should be self-contained and transactional.
	- **Queries**: Operations that retrieve data without modifying state. They should be efficient and optimized for reading.
- **Feature Modules**: Each new feature should be implemented in its own module within the `src/ops/` directory. For example, a new "share" feature would live in `src/ops/files/share`. Each feature module should contain the following files where applicable:
	- `action.rs`: The main logic for the state-changing operation.
	- `input.rs`: Data structures for the action's input.
	- `output.rs`: Data structures for the action's output.
	- `job.rs`: If the action is long-running, the job implementation.
- **Database**: Use SeaORM entities, async queries, proper error propagation
- **Comments**: Minimal inline comments, focus on why not what, no TODO comments in production code

## Daemon Architecture

Spacedrive uses a **daemon-client architecture** where a single daemon process manages the core functionality and multiple client applications (CLI, GraphQL server, desktop app) connect to it via Unix domain sockets.

> **📖 For detailed daemon architecture documentation, see [/docs/core/daemon.md](/docs/core/daemon.md)**

### **The `Wire` Trait**
All actions and queries must implement the `Wire` trait to enable type-safe client-daemon communication:

```rust
pub trait Wire {
    const METHOD: &'static str;
}
```

### **Registration Macros**
Instead of manually implementing `Wire`, use these registration macros that automatically:
1. Implement the `Wire` trait with the correct method string
2. Register the operation in the global registry using the `inventory` crate

**For Queries:**
```rust
crate::register_query!(NetworkStatusQuery, "network.status");
// Generates method: "query:network.status.v1"
```

**For Library Actions:**
```rust
crate::register_library_action!(FileCopyAction, "files.copy");
// Generates method: "action:files.copy.input.v1"
```

**For Core Actions:**
```rust
crate::register_core_action!(LibraryCreateAction, "libraries.create");
// Generates method: "action:libraries.create.input.v1"
```

### **Registry System**
- **Location**: `core/src/ops/registry.rs`
- **Mechanism**: Uses the `inventory` crate for compile-time registration
- **Global Maps**: `QUERIES` and `ACTIONS` hashmaps populated at startup
- **Handler Functions**: Generic handlers that decode payloads, execute operations, and encode results

## Logging Standards

- **Setup**: Use `tracing_subscriber::fmt()` with env filter for structured logging
- **Macros**: Use `info!`, `warn!`, `error!`, `debug!` from `tracing` crate, not `println!`
- **Job Context**: Use `ctx.log()` in jobs for job-specific logging with automatic job_id tagging
- **Structured**: Include relevant context fields: `debug!(job_id = %self.id, "message")`
- **Levels**: debug for detailed flow, info for user-relevant events, warn for recoverable issues, error for failures
- **Format**: `tracing_subscriber::fmt().with_env_filter(env_filter).init()` in main/examples
- **Environment**: Respect `RUST_LOG` env var, fallback to module-specific filters like `sd_core=info`

## Documentation

- **Core level docs**: Live in `/docs/core` - comprehensive architecture and implementation guides
- **Core design docs**: Live in `/docs/core/design` - planning documents, RFCs, and design decisions
- **Application level docs**: Live in `/docs`
- **Code docs**: Use `///` for public APIs, `//!` for module overviews, include examples


## Debug Instructions

- You can view the logs of a job in the job_logs directory in the root of the data folder
- When testing the CLI, after compiling you must first use the `restart` command to ensure the Spacedrive daemon is using the latest build.
