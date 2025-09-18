# AGENTS.md - Spacedrive Core v2

## Build/Test Commands

- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run specific test (e.g., `cargo test library_test`)
- `cargo clippy` - Lint code
- `cargo fmt` - Format code
- `cargo run --bin spacedrive -- <command>` - Run CLI

## Code Style

- **Imports**: Group std, external crates, then local modules with blank lines between
- **Formatting**: Use `cargo fmt` - tabs for indentation, snake_case for variables/functions
- **Types**: Explicit types preferred, use `Result<T, E>` for error handling with `thiserror`
- **Naming**: snake_case for functions/variables, PascalCase for types, SCREAMING_SNAKE_CASE for constants
- **Error Handling**: Use `Result` types, `thiserror` for custom errors, `anyhow` for application errors
- **Async**: Use `async/await`, prefer `tokio` primitives, avoid blocking operations
- **Documentation**: Use `//!` for module docs, `///` for public items, include examples
- **Architecture**: Follow domain-driven design - domain models in `src/domain/`, operations in `src/operations/`
- **Database**: Use SeaORM entities, async queries, proper error propagation
- **Comments**: Minimal inline comments, focus on why not what, no TODO comments in production code

## Logging Standards

- **Setup**: Use `tracing_subscriber::fmt()` with env filter for structured logging
- **Macros**: Use `info!`, `warn!`, `error!`, `debug!` from `tracing` crate, not `println!`
- **Job Context**: Use `ctx.log()` in jobs for job-specific logging with automatic job_id tagging
- **Structured**: Include relevant context fields: `debug!(job_id = %self.id, "message")`
- **Levels**: debug for detailed flow, info for user-relevant events, warn for recoverable issues, error for failures
- **Format**: `tracing_subscriber::fmt().with_env_filter(env_filter).init()` in main/examples
- **Environment**: Respect `RUST_LOG` env var, fallback to module-specific filters like `sd_core=info`

## Documentation

- **Finalized docs**: Live in `/docs` - comprehensive architecture and implementation guides
- **Design docs**: Live in `/docs/design` - planning documents, RFCs, and design decisions
- **Code docs**: Use `///` for public APIs, `//!` for module overviews, include examples
