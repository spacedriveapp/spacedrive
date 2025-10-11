# CLI Output Refactor Design Document

## Overview

This document outlines a proposed refactoring of the CLI output system to replace the current `println!` usage with a more structured and consistent approach using existing Rust libraries.

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

## Library Options

### Recommended Libraries

After evaluating various options, here are the recommended libraries for different aspects:

1. **Terminal UI Framework: `ratatui`** (for TUI mode)
   - Modern terminal UI framework
   - Great for the planned TUI mode
   - Handles layout, widgets, and rendering

2. **CLI Output: `dialoguer` + `console`**
   - `dialoguer`: High-level constructs (prompts, selections, progress)
   - `console`: Low-level terminal control
   - Both work well together

3. **Structured Output: `owo-colors` + `supports-color`**
   - More modern than `colored` crate
   - Better performance
   - Automatic color detection

4. **Progress Bars: Keep `indicatif`**
   - Already in use
   - Best-in-class for progress indication

5. **Table Formatting: Keep `comfy-table`**
   - Already in use
   - Good API and customization

### Alternative: All-in-One Solution with `dialoguer`

```rust
use dialoguer::{theme::ColorfulTheme, console::style};
use console::{Term, Emoji};

// Emojis with fallback
static SUCCESS: Emoji = Emoji("", "[OK] ");
static ERROR: Emoji = Emoji("", "[ERROR] ");
static INFO: Emoji = Emoji("️  ", "[INFO] ");

// Structured output
let term = Term::stdout();
term.clear_line()?;
term.write_line(&format!("{}{}", SUCCESS, style("Library created").green()))?;

// Progress bars
let pb = indicatif::ProgressBar::new(100);
pb.set_style(
    indicatif::ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .progress_chars("#>-")
);

// Tables (keep comfy-table)
let mut table = comfy_table::Table::new();
table.set_header(vec!["ID", "Name", "Status"]);
```

## Proposed Solution

### Core Design Principles
1. **Separation of concerns** - Business logic should not know about output formatting
2. **Testability** - Output should be capturable and assertable in tests
3. **Flexibility** - Support multiple output formats (human, json, quiet)
4. **Consistency** - Unified visual language across all commands
5. **Context-aware** - Respect user preferences (color, verbosity, format)

### Lightweight Wrapper Approach

Instead of building a complex abstraction, we'll create a thin wrapper around these libraries:

```rust
// src/infrastructure/cli/output.rs
use console::{style, Emoji, Term};
use dialoguer::theme::ColorfulTheme;
use serde::Serialize;
use std::io::Write;

pub struct CliOutput {
    term: Term,
    format: OutputFormat,
    theme: ColorfulTheme,
}

// Simple emoji constants with fallbacks
const SUCCESS: Emoji = Emoji("", "[OK] ");
const ERROR: Emoji = Emoji("", "[ERROR] ");
const WARNING: Emoji = Emoji("️  ", "[WARN] ");
const INFO: Emoji = Emoji("️  ", "[INFO] ");

impl CliOutput {
    pub fn success(&self, msg: &str) -> std::io::Result<()> {
        match self.format {
            OutputFormat::Human => {
                self.term.write_line(&format!("{}{}", SUCCESS, style(msg).green()))
            }
            OutputFormat::Json => {
                let output = json!({"type": "success", "message": msg});
                self.term.write_line(&output.to_string())
            }
            OutputFormat::Quiet => Ok(()),
        }
    }
    
    pub fn section(&self) -> OutputSection {
        OutputSection::new(self)
    }
}

// Fluent builder for sections
pub struct OutputSection<'a> {
    output: &'a CliOutput,
    lines: Vec<String>,
}

impl<'a> OutputSection<'a> {
    pub fn title(mut self, text: &str) -> Self {
        self.lines.push(format!("\n{}", style(text).bold().cyan()));
        self
    }
    
    pub fn item(mut self, label: &str, value: &str) -> Self {
        self.lines.push(format!("  {}: {}", label, style(value).bright()));
        self
    }
    
    pub fn render(self) -> std::io::Result<()> {
        for line in self.lines {
            self.output.term.write_line(&line)?;
        }
        Ok(())
    }
}
```

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

### Output Grouping and Spacing

One of the major improvements is eliminating the "println! soup" pattern where multiple `println!()` calls are used for spacing:

#### Current (Ugly) Pattern
```rust
println!("Checking pairing status...");
println!();
println!("Current Pairing Status: {}", status);
println!();
println!("No pending pairing requests");
println!();
println!("To start pairing:");
println!("   • Generate a code: spacedrive network pair generate");
```

#### New Pattern
```rust
// Using output groups
output.print(Message::PairingStatus {
    status: status.clone(),
    pending_requests: vec![],
    help_text: true,
});

// Or using a builder pattern for complex outputs
output.section("Checking pairing status")
    .status("Current Pairing Status", &status)
    .empty_line()
    .info("No pending pairing requests")
    .empty_line()
    .help()
        .item("Generate a code: spacedrive network pair generate")
        .item("Join with a code: spacedrive network pair join <code>")
    .render();
```

The formatter handles appropriate spacing based on context, eliminating manual spacing management.

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

### Section Builder API

For complex multi-line outputs, a fluent builder API makes the code much cleaner:

```rust
pub struct OutputSection<'a> {
    output: &'a mut OutputContext,
    lines: Vec<Line>,
}

impl<'a> OutputSection<'a> {
    pub fn title(mut self, text: &str) -> Self {
        self.lines.push(Line::Title(text.to_string()));
        self
    }
    
    pub fn status(mut self, label: &str, value: &str) -> Self {
        self.lines.push(Line::Status(label.to_string(), value.to_string()));
        self
    }
    
    pub fn table(mut self, table: Table) -> Self {
        self.lines.push(Line::Table(table));
        self
    }
    
    pub fn empty_line(mut self) -> Self {
        self.lines.push(Line::Empty);
        self
    }
    
    pub fn render(self) {
        // Smart spacing: removes duplicate empty lines, adds appropriate spacing
        let formatted = self.output.formatter.format_section(&self.lines);
        self.output.write(formatted);
    }
}

// Usage example - much cleaner than multiple println!s
output.section()
    .title("System Status")
    .status("Version", &status.version)
    .status("Uptime", &format_duration(status.uptime))
    .empty_line()
    .title("Libraries")
    .table(library_table)
    .empty_line()
    .help()
        .item("Create a library: spacedrive library create <name>")
        .item("Switch library: spacedrive library switch <name>")
    .render();
```

## Implementation Checklist

### Phase 1: Add Dependencies
- [ ] Add `dialoguer` to Cargo.toml
- [ ] Add `owo-colors` to Cargo.toml (or stick with `colored`)
- [ ] Keep existing `console`, `indicatif`, `comfy-table`

### Phase 2: Create Simple Wrapper
- [ ] Create `src/infrastructure/cli/output.rs`
- [ ] Implement basic `CliOutput` struct with library wrappers
- [ ] Add output format enum (Human, Json, Quiet)
- [ ] Create section builder using `console` styling

### Phase 3: Gradual Migration
- [ ] Start with one domain (e.g., library commands)
- [ ] Replace `println!` calls with output methods
- [ ] Test both human and JSON output
- [ ] Migrate remaining domains one by one

### Phase 4: Advanced Features
- [ ] Add interactive prompts with `dialoguer`
- [ ] Implement TUI mode with `ratatui`
- [ ] Add output templates for customization
- [ ] Integrate with tracing for debug output

## Example Migration

```rust
// Before:
println!("Starting Spacedrive daemon...");
println!();
println!("Daemon started successfully");
println!("   PID: {}", pid);
println!("   Socket: {}", socket_path);

// After:
let output = CliOutput::new(format);
output.info("Starting Spacedrive daemon...")?;
output.success("Daemon started successfully")?;
output.section()
    .item("PID", &pid.to_string())
    .item("Socket", &socket_path.display().to_string())
    .render()?;
```