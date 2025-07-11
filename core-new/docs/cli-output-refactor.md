# CLI Output Refactor Design Document

## Overview

This document outlines a proposed refactoring of the CLI output system to replace the current `println!` usage with a more structured and consistent approach.

## Current State

### Problems
1. **Inconsistent output patterns** - Each domain handler uses different formatting styles
2. **Mixed approaches** - Some functions return strings, others print directly
3. **No output format options** - Cannot output JSON for scripting/automation
4. **Difficult to test** - Direct `println!` calls are hard to capture in tests
5. **No verbosity control** - All output is shown regardless of user preference
6. **Scattered emoji/color logic** - Formatting decisions spread throughout codebase

### Current Dependencies
- `colored` - Terminal colors
- `indicatif` - Progress bars and spinners
- `console` - Terminal utilities
- `comfy-table` - Table formatting
- `tracing` - Structured logging (underutilized for CLI output)

## Proposed Solution

### Core Design Principles
1. **Separation of concerns** - Business logic should not know about output formatting
2. **Testability** - Output should be capturable and assertable in tests
3. **Flexibility** - Support multiple output formats (human, json, quiet)
4. **Consistency** - Unified visual language across all commands
5. **Context-aware** - Respect user preferences (color, verbosity, format)

### Architecture

```rust
// src/infrastructure/cli/output/mod.rs

/// Global output context passed through CLI operations
pub struct OutputContext {
    format: OutputFormat,
    verbosity: VerbosityLevel,
    color: ColorMode,
    writer: Box<dyn Write>,  // Allows testing with buffers
}

pub enum OutputFormat {
    Human,      // Default, pretty-printed with colors/emojis
    Json,       // Machine-readable JSON
    Quiet,      // Minimal output (errors only)
}

pub enum VerbosityLevel {
    Quiet = 0,      // Errors only
    Normal = 1,     // Default
    Verbose = 2,    // Additional info
    Debug = 3,      // Everything
}

pub enum ColorMode {
    Auto,       // Detect terminal support
    Always,     // Force colors
    Never,      // No colors
}

/// All possible output messages in the system
pub enum Message {
    // Success messages
    LibraryCreated { name: String, id: Uuid },
    LocationAdded { path: PathBuf },
    DaemonStarted { instance: String },
    
    // Error messages
    DaemonNotRunning { instance: String },
    LibraryNotFound { id: Uuid },
    
    // Progress messages
    IndexingProgress { current: u64, total: u64, location: String },
    
    // Status messages
    DaemonStatus { version: String, uptime: u64, libraries: Vec<LibraryInfo> },
    
    // ... etc
}

/// Core output trait - implemented for each format
pub trait OutputFormatter {
    fn format(&self, message: &Message, context: &OutputContext) -> String;
}

/// Main output handler
impl OutputContext {
    pub fn print(&mut self, message: Message) {
        if self.should_print(&message) {
            let formatted = self.format(&message);
            writeln!(self.writer, "{}", formatted).ok();
        }
    }
    
    pub fn error(&mut self, message: Message) {
        // Errors always print regardless of verbosity
        let formatted = self.format_error(&message);
        writeln!(self.writer, "{}", formatted).ok();
    }
}
```

### Human-Readable Formatter

```rust
pub struct HumanFormatter;

impl OutputFormatter for HumanFormatter {
    fn format(&self, message: &Message, context: &OutputContext) -> String {
        match message {
            Message::LibraryCreated { name, id } => {
                format!("{} Library '{}' created successfully", 
                    if context.use_emoji() { "✓" } else { "[OK]" }.green(),
                    name.bright_cyan()
                )
            }
            Message::DaemonNotRunning { instance } => {
                format!("{} Spacedrive daemon instance '{}' is not running\n   Start it with: spacedrive start",
                    "❌".red(),
                    instance
                )
            }
            // ... etc
        }
    }
}
```

### JSON Formatter

```rust
pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn format(&self, message: &Message, _: &OutputContext) -> String {
        // Convert messages to structured JSON
        match message {
            Message::LibraryCreated { name, id } => {
                json!({
                    "type": "library_created",
                    "success": true,
                    "data": {
                        "name": name,
                        "id": id.to_string()
                    }
                }).to_string()
            }
            // ... etc
        }
    }
}
```

### Integration Points

#### 1. CLI Entry Point
```rust
// In main CLI parser
let output = OutputContext::new(
    matches.value_of("format").unwrap_or("human"),
    matches.occurrences_of("verbose"),
    matches.is_present("no-color"),
);

// Pass through command handlers
handle_library_command(cmd, output).await?;
```

#### 2. Command Handlers
```rust
pub async fn handle_library_command(
    cmd: LibraryCommands,
    mut output: OutputContext,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        LibraryCommands::Create { name } => {
            let library = create_library(name).await?;
            output.print(Message::LibraryCreated {
                name: library.name,
                id: library.id,
            });
        }
    }
}
```

#### 3. Testing
```rust
#[test]
fn test_library_create_output() {
    let mut buffer = Vec::new();
    let mut output = OutputContext::test(buffer);
    
    output.print(Message::LibraryCreated {
        name: "Test".into(),
        id: Uuid::new_v4(),
    });
    
    let result = String::from_utf8(output.into_inner()).unwrap();
    assert!(result.contains("Library 'Test' created"));
}
```

### Progress Handling

For long-running operations, integrate with existing `indicatif`:

```rust
pub struct ProgressContext {
    output: OutputContext,
    progress: Option<ProgressBar>,
}

impl ProgressContext {
    pub fn update(&mut self, message: Message) {
        match &message {
            Message::IndexingProgress { current, total, .. } => {
                if let Some(pb) = &self.progress {
                    pb.set_position(*current);
                    pb.set_message(format!("{}/{}", current, total));
                }
            }
            _ => self.output.print(message),
        }
    }
}
```

### Migration Strategy

1. **Phase 1**: Implement core output module without changing existing code
2. **Phase 2**: Gradually migrate each domain handler to use new system
3. **Phase 3**: Add JSON output support once all handlers migrated
4. **Phase 4**: Add advanced features (output filtering, custom formats)

### Benefits

1. **Testability** - Can capture and assert output in tests
2. **Consistency** - Single source of truth for all messages
3. **Localization-ready** - Messages defined in one place
4. **Machine-readable** - JSON output for automation
5. **Better UX** - Respects user preferences (quiet mode, no color, etc.)
6. **Maintainability** - Easy to update output style globally

### Backwards Compatibility

- Default behavior remains unchanged (human-readable with colors)
- Existing CLI commands work identically
- New flags are additive: `--format json`, `--quiet`, `--no-color`

### Future Extensions

1. **Structured logging integration** - Connect with tracing for debug output
2. **Template support** - User-defined output templates
3. **Localization** - Message translations
4. **Output plugins** - Custom formatters for specific tools
5. **Streaming JSON** - For real-time event monitoring

## Implementation Checklist

- [ ] Create output module structure
- [ ] Define core Message enum with all CLI messages
- [ ] Implement HumanFormatter
- [ ] Implement JsonFormatter  
- [ ] Add OutputContext to CLI args
- [ ] Migrate domain handlers one by one
- [ ] Add tests for output formatting
- [ ] Update CLI documentation
- [ ] Add examples of JSON usage for scripting