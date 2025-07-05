# File Copy Builder Pattern Usage Examples

This document demonstrates how the builder pattern is implemented and used for the file copy action in Spacedrive.

## Overview

The builder pattern provides a fluent, type-safe API for constructing file copy actions. It handles validation, CLI integration, and seamless conversion to the action system.

## Architecture

```
CLI Arguments → FileCopyArgs (clap) → FileCopyActionBuilder → FileCopyAction → Action::FileCopy → Job Dispatch
```

## Builder API Examples

### 1. Basic Fluent API

```rust
use crate::operations::files::copy::action::FileCopyAction;

// Simple single file copy
let action = FileCopyAction::builder()
    .source("/path/to/source.txt")
    .destination("/path/to/destination.txt")
    .build()?;

// Multiple files with options
let action = FileCopyAction::builder()
    .sources(["/file1.txt", "/file2.txt", "/dir/"])
    .destination("/backup/")
    .overwrite(true)
    .verify_checksum(true)
    .preserve_timestamps(false)
    .move_files(true)  // Move instead of copy
    .build()?;
```

### 2. Convenience Methods

```rust
// Quick single file copy
let builder = FileCopyAction::copy_file("/source.txt", "/dest.txt");

// Quick multiple files copy
let sources = vec!["/file1.txt", "/file2.txt"];
let builder = FileCopyAction::copy_files(sources, "/backup/");
```

### 3. CLI Integration

The CLI automatically uses the builder pattern:

```bash
# Basic copy
spacedrive copy file1.txt file2.txt --destination /backup/

# With options
spacedrive copy /photos/* --destination /backup/photos/ --overwrite --verify

# Move operation
spacedrive copy /temp/* --destination /archive/ --move-files

# Preserve timestamps disabled
spacedrive copy /source/ --destination /dest/ --preserve-timestamps false
```

### 4. Programmatic Usage

```rust
use crate::infrastructure::actions::builder::{ActionBuilder, CliActionBuilder};
use crate::operations::files::copy::action::{FileCopyActionBuilder, FileCopyArgs};

// From CLI args
let args = FileCopyArgs {
    sources: vec!["/src/file.txt".into()],
    destination: "/dest/".into(),
    overwrite: true,
    verify: false,
    preserve_timestamps: true,
    move_files: false,
};

let action = FileCopyActionBuilder::from_cli_args(args).build()?;

// Direct builder usage
let action = FileCopyActionBuilder::new()
    .source("/important.doc")
    .destination("/backup/important.doc")
    .overwrite(false)
    .verify_checksum(true)
    .build()?;
```

## Validation

The builder provides comprehensive validation:

```rust
// This will fail - no sources
let result = FileCopyAction::builder()
    .destination("/dest/")
    .build();
assert!(result.is_err());

// This will fail - no destination
let result = FileCopyAction::builder()
    .source("/file.txt")
    .build();
assert!(result.is_err());

// This will fail - source doesn't exist (if validation runs)
let result = FileCopyAction::builder()
    .source("/nonexistent.txt")
    .destination("/dest/")
    .build();
// Error: "Source file does not exist: /nonexistent.txt"
```

## CLI Command Flow

When a user runs a copy command, here's what happens:

1. **CLI Parsing**: `clap` parses arguments into `FileCopyArgs`
2. **Builder Creation**: `FileCopyActionBuilder::from_cli_args(args)`
3. **Validation**: Builder validates sources exist, destination is valid
4. **Action Creation**: `builder.build()` creates `FileCopyAction`
5. **Action Wrapping**: Wrapped in `Action::FileCopy { library_id, action }`
6. **Dispatch**: `action_manager.dispatch(action)` sends to handler
7. **Job Creation**: Handler creates `FileCopyJob` directly (no JSON roundtrip)
8. **Job Dispatch**: Job dispatched to job system
9. **CLI Feedback**: User sees job ID and can monitor progress

## Error Handling

The builder provides detailed error messages:

```rust
match FileCopyActionBuilder::new().build() {
    Err(ActionBuildError::Validation(errors)) => {
        for error in errors {
            println!("Validation error: {}", error);
        }
        // Output:
        // Validation error: At least one source file must be specified
        // Validation error: Destination path must be specified
    }
    _ => {}
}
```

## Benefits

1. **Type Safety**: Compile-time validation of required fields
2. **Fluent API**: Easy to read and write
3. **Validation**: Build-time validation prevents invalid actions
4. **CLI Integration**: Seamless conversion from CLI args
5. **Performance**: Direct job creation eliminates JSON serialization
6. **Extensibility**: Easy to add new options without breaking existing code

## Testing

Comprehensive tests validate the builder pattern:

```rust
#[test]
fn test_builder_fluent_api() {
    let action = FileCopyAction::builder()
        .sources(["/src/file1.txt", "/src/file2.txt"])
        .destination("/dest/")
        .overwrite(true)
        .verify_checksum(true)
        .build();
    // Validation tests...
}

#[test]
fn test_cli_integration() {
    let args = FileCopyArgs { /* ... */ };
    let action = FileCopyActionBuilder::from_cli_args(args).build().unwrap();
    // Integration tests...
}
```

## Future Extensions

The builder pattern makes it easy to add new features:

```rust
impl FileCopyActionBuilder {
    // Future options
    pub fn compression(mut self, level: u8) -> Self { /* ... */ }
    pub fn encryption(mut self, enabled: bool) -> Self { /* ... */ }
    pub fn bandwidth_limit(mut self, mbps: u32) -> Self { /* ... */ }
}
```

This architecture provides a solid foundation for expanding file operations while maintaining type safety and ease of use.