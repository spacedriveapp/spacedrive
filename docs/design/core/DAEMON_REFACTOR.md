<!--CREATED: 2025-07-11-->
# Daemon Refactoring Design Document

## Overview

The current `daemon.rs` file has grown to over 1,500 lines and handles all command processing in a single monolithic `handle_command` function. This document outlines a plan to refactor the daemon into a modular architecture that improves maintainability, testability, and extensibility.

## Current Problems

1. **Monolithic Structure**: All command handling logic is in one massive switch statement
2. **Mixed Concerns**: Business logic, presentation formatting, and transport concerns are intermingled
3. **Poor Testability**: Difficult to unit test individual command handlers
4. **Code Duplication**: Common patterns (like "get current library") are repeated throughout
5. **Hard to Navigate**: Finding specific command logic requires scrolling through 1,500+ lines

## Proposed Architecture

### Directory Structure

```
src/infrastructure/cli/daemon/
├── mod.rs                 # Core daemon server (socket handling, lifecycle)
├── client.rs              # DaemonClient implementation
├── config.rs              # DaemonConfig and instance management
├── types/
│   ├── mod.rs            # Re-exports all types
│   ├── commands.rs       # DaemonCommand enum and sub-commands
│   ├── responses.rs      # DaemonResponse enum and response types
│   └── common.rs         # Shared types (JobInfo, LibraryInfo, etc.)
├── handlers/
│   ├── mod.rs            # Handler trait and registry
│   ├── core.rs           # Core commands (ping, shutdown, status)
│   ├── library.rs        # Library command handling
│   ├── location.rs       # Location command handling
│   ├── job.rs            # Job command handling
│   ├── network.rs        # Network command handling
│   ├── file.rs           # File command handling
│   └── system.rs         # System command handling
└── services/
    ├── mod.rs            # Service traits
    ├── state.rs          # CLI state management service
    └── helpers.rs        # Common helpers (get_current_library, etc.)
```

### Core Components

#### 1. Command Handler Trait

```rust
// daemon/handlers/mod.rs
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn handle(&self, cmd: DaemonCommand) -> DaemonResponse;
}

pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}
```

#### 2. Individual Handlers

Each handler focuses on a specific domain:

```rust
// daemon/handlers/library.rs
pub struct LibraryHandler {
    core: Arc<Core>,
    state_service: Arc<StateService>,
}

#[async_trait]
impl CommandHandler for LibraryHandler {
    async fn handle(&self, cmd: DaemonCommand) -> DaemonResponse {
        match cmd {
            DaemonCommand::CreateLibrary { name, path } => {
                self.create_library(name, path).await
            }
            DaemonCommand::ListLibraries => {
                self.list_libraries().await
            }
            // ... other library commands
            _ => DaemonResponse::Error("Invalid command for library handler".into())
        }
    }
}
```

#### 3. Services Layer

Common functionality extracted into reusable services:

```rust
// daemon/services/state.rs
pub struct StateService {
    cli_state: Arc<RwLock<CliState>>,
    data_dir: PathBuf,
}

impl StateService {
    pub async fn get_current_library(&self, core: &Core) -> Option<Arc<Library>> {
        // Common logic for getting current library
    }
    
    pub async fn switch_library(&self, library_id: Uuid) -> Result<(), Error> {
        // Common logic for switching libraries
    }
}
```

#### 4. Simplified Daemon Core

The main daemon becomes a thin routing layer:

```rust
// daemon/mod.rs
pub struct Daemon {
    core: Arc<Core>,
    config: DaemonConfig,
    handlers: HandlerRegistry,
    services: Arc<Services>,
}

async fn handle_client(/* ... */) -> Result<(), Box<dyn Error>> {
    // ... read command ...
    
    let response = match cmd {
        DaemonCommand::Ping => self.handlers.core.handle(cmd).await,
        DaemonCommand::CreateLibrary { .. } |
        DaemonCommand::ListLibraries |
        DaemonCommand::SwitchLibrary { .. } => {
            self.handlers.library.handle(cmd).await
        }
        DaemonCommand::AddLocation { .. } |
        DaemonCommand::ListLocations |
        DaemonCommand::RemoveLocation { .. } => {
            self.handlers.location.handle(cmd).await
        }
        // ... etc
    };
    
    // ... send response ...
}
```

## Migration Plan

### Phase 1: Extract Types (Low Risk)
1. Create `types/` directory
2. Move all type definitions (commands, responses, common types)
3. Update imports throughout the codebase

### Phase 2: Extract Services (Medium Risk)
1. Create `services/` directory
2. Extract common patterns into services:
   - State management
   - Current library logic
   - Device registration
   - Error handling patterns

### Phase 3: Create Handlers (Medium Risk)
1. Create `handlers/` directory
2. Implement handler trait
3. Create individual handlers, starting with:
   - Core handler (ping, shutdown, status)
   - Library handler
   - One handler at a time for remaining domains

### Phase 4: Refactor Daemon Core (High Risk)
1. Update daemon to use handler registry
2. Replace monolithic switch with handler dispatch
3. Clean up remaining code

### Phase 5: Cleanup and Testing
1. Add unit tests for each handler
2. Add integration tests for daemon
3. Remove any dead code
4. Update documentation

## Benefits

1. **Modularity**: Each domain's logic is isolated in its own handler
2. **Testability**: Handlers can be unit tested without starting a daemon
3. **Maintainability**: Easy to find and modify specific functionality
4. **Extensibility**: Adding new commands only requires adding a handler
5. **Code Reuse**: Common patterns are extracted into services
6. **Type Safety**: Better type organization prevents errors

## Alternative Approaches Considered

### 1. Message Bus Pattern
- **Pros**: Fully decoupled, async message passing
- **Cons**: More complex, harder to debug, overkill for this use case

### 2. Plugin System
- **Pros**: Maximum extensibility
- **Cons**: Too complex for internal refactoring

### 3. Macro-based Code Generation
- **Pros**: Less boilerplate
- **Cons**: Harder to understand, debug, and maintain

## Implementation Timeline

- **Week 1**: Extract types and create directory structure
- **Week 2**: Implement services layer
- **Week 3-4**: Create handlers (2-3 handlers per week)
- **Week 5**: Refactor daemon core and testing
- **Week 6**: Documentation and cleanup

## Success Metrics

1. **Code Reduction**: daemon.rs reduced from 1,500+ lines to <300 lines
2. **Test Coverage**: Each handler has >80% unit test coverage
3. **Performance**: No regression in command processing time
4. **Developer Experience**: Easier to find and modify command logic

## Risks and Mitigations

1. **Breaking Changes**: Mitigate by keeping external API identical
2. **Regression Bugs**: Mitigate with comprehensive testing at each phase
3. **Performance Impact**: Mitigate by benchmarking before/after
4. **Merge Conflicts**: Mitigate by completing refactor quickly

## Additional Refactoring: CLI Domains to Commands

### Current Confusion

The current codebase uses "domains" for CLI modules that primarily:
- Define command structures (enums with clap attributes)
- Handle command-line argument parsing
- Format output for user presentation
- Send requests to the daemon

### Proposed Renaming

Rename `cli/domains/` to `cli/commands/` to better reflect their purpose:

```
src/infrastructure/cli/
├── commands/              # (renamed from domains/)
│   ├── daemon.rs         # Daemon lifecycle commands (start, stop, status)
│   ├── library.rs        # Library management commands
│   ├── location.rs       # Location management commands
│   ├── job.rs           # Job monitoring commands
│   ├── network.rs        # Network operation commands
│   ├── file.rs          # File operation commands
│   └── system.rs        # System monitoring commands
└── daemon/
    ├── handlers/         # Daemon-side handlers that
    │   ├── library.rs    # process commands and execute logic
    │   ├── location.rs
    │   └── ...
```

This creates a clearer separation:
- **CLI Commands** (`cli/commands/`): Define command structure, parse arguments, format output
- **Daemon Handlers** (`daemon/handlers/`): Execute business logic, interact with Core

### Example to Illustrate the Difference

```rust
// cli/commands/library.rs - Defines the command and presentation
#[derive(Subcommand)]
pub enum LibraryCommands {
    Create { name: String, path: Option<PathBuf> },
    List { detailed: bool },
}

pub async fn handle_library_command(cmd: LibraryCommands, output: CliOutput) {
    let response = daemon_client.send_command(cmd).await?;
    // Format and present the response to the user
    output.print_libraries(response);
}

// daemon/handlers/library.rs - Executes the actual logic
impl LibraryHandler {
    async fn create_library(&self, name: String, path: Option<PathBuf>) {
        // Actually create the library using Core
        self.core.libraries.create_library(name, path).await
    }
}
```

### Benefits of This Naming

1. **Clarity**: "Commands" clearly indicates these modules define CLI commands
2. **Separation of Concerns**: Commands (presentation) vs Handlers (logic) is clearer
3. **Intuitive**: Developers expect "commands" to contain CLI command definitions
4. **No Ambiguity**: Clear distinction between what defines commands and what handles them

## Next Steps

1. Review and approve this design
2. Rename `cli/domains/` to `cli/commands/`
3. Create tracking issues for each phase
4. Begin Phase 1 implementation
5. Set up testing infrastructure