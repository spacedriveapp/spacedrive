# Input Abstraction Architecture

This document describes the refactored architecture that uses input abstraction to support multiple interfaces (CLI, GraphQL, REST, etc.) without code duplication.

## Overview

The input abstraction pattern separates **interface-specific argument parsing** from **core business logic** by introducing a canonical input type that all interfaces convert to.

## Architecture Flow

```
┌─────────────┬─────────────────┬─────────────────┐
│  CLI Args   │  GraphQL Input  │   REST Request  │
│ (clap)      │ (async-graphql) │    (serde)      │
└─────────────┴─────────────────┴─────────────────┘
      │                 │                 │
      ▼                 ▼                 ▼
┌─────────────┬─────────────────┬─────────────────┐
│FileCopyCmd  │FileCopyGQLInput │FileCopyReq      │
│Args         │                 │                 │
└─────────────┴─────────────────┴─────────────────┘
      │                 │                 │
      └─────────────────┼─────────────────┘
                        ▼
               ┌─────────────────┐
               │ FileCopyInput   │ ◄── Core canonical type
               │   (domain)      │
               └─────────────────┘
                        │
                        ▼
               ┌─────────────────┐
               │FileCopyAction   │
               │   Builder       │
               └─────────────────┘
                        │
                        ▼
               ┌─────────────────┐
               │ FileCopyAction  │
               └─────────────────┘
                        │
                        ▼
               ┌─────────────────┐
               │   Job System    │
               └─────────────────┘
```

## File Structure

```
src/
├── operations/files/copy/
│   ├── input.rs          # Core FileCopyInput type
│   ├── action.rs         # FileCopyAction + Builder  
│   ├── job.rs           # FileCopyJob
│   └── mod.rs
├── infrastructure/
│   ├── cli/adapters/
│   │   ├── copy.rs      # FileCopyCliArgs -> FileCopyInput
│   │   └── mod.rs
│   ├── graphql/inputs/  # Future: GraphQL inputs
│   └── rest/requests/   # Future: REST requests
```

## Core Components

### 1. FileCopyInput (Domain)

**Location**: `src/operations/files/copy/input.rs`

The canonical input type that defines the complete interface for file copy operations:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopyInput {
    pub sources: Vec<PathBuf>,
    pub destination: PathBuf,
    pub overwrite: bool,
    pub verify_checksum: bool,
    pub preserve_timestamps: bool,
    pub move_files: bool,
}

impl FileCopyInput {
    // Fluent API for programmatic construction
    pub fn new(sources: Vec<PathBuf>, destination: PathBuf) -> Self
    pub fn with_overwrite(self, overwrite: bool) -> Self
    pub fn with_verification(self, verify: bool) -> Self
    // ... other builder methods
    
    // Validation and conversion
    pub fn validate(&self) -> Result<(), Vec<String>>
    pub fn to_copy_options(&self) -> CopyOptions
    pub fn summary(&self) -> String
}
```

### 2. CLI Adapter

**Location**: `src/infrastructure/cli/adapters/copy.rs`

Handles CLI-specific argument parsing and converts to the core input type:

```rust
#[derive(Debug, Clone, Parser)]
pub struct FileCopyCliArgs {
    pub sources: Vec<PathBuf>,
    #[arg(short, long)]
    pub destination: PathBuf,
    #[arg(long)]
    pub overwrite: bool,
    // ... other CLI-specific options
}

impl From<FileCopyCliArgs> for FileCopyInput {
    fn from(args: FileCopyCliArgs) -> Self {
        // Convert CLI args to canonical input
    }
}

impl FileCopyCliArgs {
    pub fn validate_and_convert(self) -> Result<FileCopyInput, String>
}
```

### 3. Updated Action Builder

**Location**: `src/operations/files/copy/action.rs`

The builder now uses the input abstraction as its primary interface:

```rust
#[derive(Debug, Clone)]
pub struct FileCopyActionBuilder {
    input: FileCopyInput,  // Core input type
    errors: Vec<String>,
}

impl FileCopyActionBuilder {
    // Primary interface - all others convert to this
    pub fn from_input(input: FileCopyInput) -> Self
    
    // Interface-specific convenience methods
    pub fn from_cli_args(args: FileCopyCliArgs) -> Self {
        Self::from_input(args.into())
    }
    
    // Future interfaces
    pub fn from_graphql_input(input: FileCopyGraphQLInput) -> Self {
        Self::from_input(input.into())
    }
}
```

### 4. CLI Handler

**Location**: `src/infrastructure/cli/commands.rs`

Simplified to use the input abstraction:

```rust
pub async fn handle_copy_command(
    args: FileCopyCliArgs,
    core: &Core,
    state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Convert and validate CLI args
    let input = args.validate_and_convert()?;
    
    // Create action using core input
    let action = FileCopyActionBuilder::from_input(input.clone()).build()?;
    
    // Dispatch action
    let output = action_manager.dispatch(action).await?;
    
    // Handle output...
}
```

## Benefits

### ✅ **Single Source of Truth**
- `FileCopyInput` defines the canonical interface
- All validation logic centralized in one place
- Consistent behavior across all interfaces

### ✅ **Interface Independence**
- CLI can have CLI-specific features (help text, value parsing)
- GraphQL can use GraphQL scalars and nullable types  
- REST can use different field names and structures
- Each interface optimized for its use case

### ✅ **No Code Duplication**
- Core business logic written once
- Interface adapters are lightweight conversions
- Builder logic shared across all interfaces

### ✅ **Easy Testing**
- Test core input types independently
- Test interface adapters separately
- Mock different interfaces easily

### ✅ **Future Extensibility**
Adding new interfaces is straightforward:

```rust
// Future: GraphQL support
#[derive(InputObject)]
pub struct FileCopyGraphQLInput {
    pub sources: Vec<String>,
    pub destination: String,
    pub options: Option<CopyOptionsInput>,
}

impl From<FileCopyGraphQLInput> for FileCopyInput {
    fn from(input: FileCopyGraphQLInput) -> Self {
        // Convert GraphQL input to core input
    }
}

// Update builder
impl FileCopyActionBuilder {
    pub fn from_graphql_input(input: FileCopyGraphQLInput) -> Self {
        Self::from_input(input.into())
    }
}
```

## Current CLI Usage

The CLI interface remains exactly the same for users:

```bash
# Basic copy
spacedrive copy file1.txt file2.txt --destination /backup/

# Advanced copy with options
spacedrive copy /photos/* --destination /backup/photos/ \
  --overwrite --verify --move-files

# All existing functionality preserved
```

## Testing

Comprehensive test coverage at multiple levels:

### Core Input Tests (8 tests)
- Input validation
- Fluent API construction
- Conversion to CopyOptions
- Summary generation

### CLI Adapter Tests (4 tests)  
- CLI args → Input conversion
- Validation integration
- Default value handling

### Action Builder Tests (7 tests)
- Builder fluent API
- Input abstraction flow
- CLI integration
- Validation scenarios

**Total: 19 tests covering the full stack**

## Migration Benefits

This refactor provides:

1. **Immediate Value**: Clean separation of concerns
2. **Future Readiness**: Easy to add GraphQL, REST APIs
3. **Maintainability**: Centralized business logic
4. **Type Safety**: Compile-time validation preserved
5. **Performance**: No runtime overhead from abstraction

The architecture scales seamlessly as new interfaces are added, ensuring consistent behavior and preventing code duplication across the entire system.